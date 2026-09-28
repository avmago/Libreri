/**
 * PDFs, with PDF.js and its viewer: continuous pages, a selectable text
 * layer, links, find and zoom. Large files stream through HTTP range
 * requests on `book://`.
 *
 * Highlights are stored as rectangles in fractions of the page
 * (`{type:"pdf-highlight", page, rects}`) plus the quoted text, and drawn in
 * a layer on top of each rendered page.
 */
import { clipFrom } from "../clip";
import type { PDFDocumentProxy } from "pdfjs-dist";
import { commands, unwrap, type Annotation, type WordDto } from "@/lib/ipc";
import type { MarkupLayer } from "../markup/MarkupLayer";
import { findHits, matchRects, wordLayer } from "../ocrText";
import { makeQuote } from "../quote";
import { runWords, WordSpeech, type Box as SpeechBox, type PageWord } from "../speech/words";
import type { SpeechSource } from "../speech/types";
import type { PageTheme } from "../themes";
import {
  highlightFill,
  type FindResult,
  type Locator,
  type PdfDarkMode,
  type Rect,
  type Renderer,
  type RendererEvents,
  type TocItem,
  type ZoomValue,
  drawnClass,
  isDrawn,
} from "../types";
import { PDF_ASSETS } from "./assets";
import { loadPdfJs } from "./load";

type ViewerModule = typeof import("pdfjs-dist/legacy/web/pdf_viewer.mjs");
type PdfViewer = InstanceType<ViewerModule["PDFViewer"]>;
type EventBus = InstanceType<ViewerModule["EventBus"]>;
type FindController = InstanceType<ViewerModule["PDFFindController"]>;
type LinkService = InstanceType<ViewerModule["PDFLinkService"]>;

export class PdfRenderer implements Renderer {
  readonly paged = true;
  private root!: HTMLDivElement;
  private container!: HTMLDivElement;
  private viewer!: PdfViewer;
  private bus!: EventBus;
  private finder!: FindController;
  private links!: LinkService;
  private doc!: PDFDocumentProxy;
  private destroyed = false;
  private tocItems: TocItem[] = [];
  private dests = new Map<string, unknown>();
  private labels: string[] | null = null;
  private annotations: Annotation[] = [];
  private theme: PageTheme | null = null;
  private mode: PdfDarkMode = "recolour";
  private findResolve: ((r: FindResult) => void) | null = null;
  private lastQuery = "";
  private zoomValue: ZoomValue = "auto";
  private cleanup: (() => void)[] = [];
  private markup: MarkupLayer | null = null;
  /** Pages read with OCR (scans): they get a hidden text layer. */
  private ocrPages = new Set<number>();
  private ocrWords = new Map<number, WordDto[]>();
  private ocrTexts: string[] | null = null;
  /** The sentence read aloud: its page and line boxes. */
  private spoken: { page: number; rects: SpeechBox[] } | null = null;
  /** Finding in OCR text, when PDF.js finds nothing in the PDF's own text. */
  private ocrFind: { query: string; hits: { page: number; index: number }[]; at: number } | null =
    null;

  constructor(
    private readonly events: RendererEvents,
    private readonly bookId?: string,
  ) {}

  async open(host: HTMLElement, url: string, initial: Locator | null) {
    const { pdfjs, viewer: mod } = await loadPdfJs();
    this.root = document.createElement("div");
    this.root.className = "lb-pdf";
    this.container = document.createElement("div");
    this.container.className = "lb-pdf-scroller";
    const inner = document.createElement("div");
    inner.className = "pdfViewer";
    this.container.append(inner);
    this.root.append(this.container);
    host.append(this.root);

    this.bus = new mod.EventBus();
    this.links = new mod.PDFLinkService({ eventBus: this.bus, externalLinkTarget: 2 });
    this.finder = new mod.PDFFindController({ eventBus: this.bus, linkService: this.links });
    this.viewer = new mod.PDFViewer({
      container: this.container,
      viewer: inner,
      eventBus: this.bus,
      linkService: this.links,
      findController: this.finder,
      textLayerMode: 1,
      removePageBorders: false,
    });
    this.links.setViewer(this.viewer);

    const task = pdfjs.getDocument({
      url,
      disableAutoFetch: true,
      disableStream: true,
      rangeChunkSize: 1 << 20,
      ...PDF_ASSETS,
    });
    this.cleanup.push(() => void task.destroy());
    this.doc = await task.promise;
    if (this.destroyed) return;
    // Typing into a form field marks the book as changed (Save offers a new version).
    const storage = this.doc.annotationStorage as unknown as {
      onSetModified: (() => void) | null;
      onResetModified: (() => void) | null;
    };
    storage.onSetModified = () => this.events.formChanged?.(true);
    storage.onResetModified = () => this.events.formChanged?.(false);

    const pagesReady = new Promise<void>((resolve) =>
      this.bus.on("pagesinit", () => resolve(), { once: true }),
    );
    this.viewer.setDocument(this.doc);
    this.links.setDocument(this.doc);
    this.finder.setDocument(this.doc);
    this.labels = await this.doc.getPageLabels().catch(() => null);
    await this.loadOutline();
    await pagesReady;
    if (this.destroyed) return;
    this.viewer.currentScaleValue = "auto";

    this.bus.on("pagechanging", () => this.emitLocation());
    // Refit when the window or panels change size (or a hidden tab is shown).
    let lastWidth = this.container.clientWidth;
    const resize = new ResizeObserver(() => {
      const w = this.container.clientWidth;
      if (!w || w === lastWidth) return;
      lastWidth = w;
      if (typeof this.zoomValue === "string") {
        const page = this.viewer.currentPageNumber;
        this.viewer.currentScaleValue = this.zoomValue;
        this.viewer.currentPageNumber = page;
      }
    });
    resize.observe(this.container);
    this.cleanup.push(() => resize.disconnect());
    this.container.addEventListener("scroll", this.onScroll, { passive: true });
    this.bus.on("textlayerrendered", ({ pageNumber }: { pageNumber: number }) =>
      this.drawPage(pageNumber),
    );
    this.bus.on("pagerendered", ({ pageNumber }: { pageNumber: number }) =>
      this.drawPage(pageNumber),
    );
    this.bus.on("updatefindmatchescount", ({ matchesCount }: { matchesCount: FindResult }) =>
      this.findResolve?.(matchesCount),
    );
    this.bus.on(
      "updatefindcontrolstate",
      ({ matchesCount, state }: { matchesCount: FindResult; state: number }) => {
        // state 1 = not found
        if (state === 1) this.findResolve?.({ current: 0, total: 0 });
        else if (matchesCount.total) this.findResolve?.(matchesCount);
      },
    );

    const onUp = () => setTimeout(() => this.onSelection(), 0);
    this.container.addEventListener("pointerup", onUp);
    this.container.addEventListener("keyup", onUp);
    const onClick = (e: MouseEvent) => {
      const hit = document
        .elementsFromPoint(e.clientX, e.clientY)
        .find((el): el is HTMLElement => el.classList.contains("lb-pdf-hl"));
      if (hit && document.getSelection()?.isCollapsed) {
        this.events.annotationClick(hit.dataset.annotation!, hit.getBoundingClientRect());
      }
    };
    this.container.addEventListener("click", onClick);
    // Highlights sit above the text layer but must not block selecting text.
    const onMove = (e: PointerEvent) => this.hoverHighlights(e);
    this.container.addEventListener("pointermove", onMove);
    this.cleanup.push(() => {
      this.container.removeEventListener("scroll", this.onScroll);
      this.container.removeEventListener("pointerup", onUp);
      this.container.removeEventListener("keyup", onUp);
      this.container.removeEventListener("click", onClick);
      this.container.removeEventListener("pointermove", onMove);
    });

    if (initial) await this.goTo(initial);
    this.emitLocation();
    void this.loadOcr();
  }

  destroy() {
    this.destroyed = true;
    for (const f of this.cleanup) f();
    this.viewer?.cleanup();
    this.root?.remove();
  }

  private frame = 0;
  private onScroll = () => {
    cancelAnimationFrame(this.frame);
    this.frame = requestAnimationFrame(() => this.emitLocation());
  };

  private async loadOutline() {
    const outline = await this.doc.getOutline().catch(() => null);
    let n = 0;
    const convert = (items: NonNullable<typeof outline>): TocItem[] =>
      items.map((item) => {
        const key = `dest:${n++}`;
        this.dests.set(key, item.dest);
        return { label: item.title, target: key, children: convert(item.items ?? []) };
      });
    this.tocItems = outline ? convert(outline) : [];
  }

  toc() {
    return this.tocItems;
  }

  private pageLabel(page: number): string {
    const l = this.labels?.[page - 1];
    return l && l !== String(page) ? l : String(page);
  }

  private emitLocation() {
    if (!this.viewer?.pagesCount) return;
    const page = this.viewer.currentPageNumber;
    const pages = this.viewer.pagesCount;
    const view = this.viewer.getPageView(page - 1);
    const div = view?.div as HTMLElement | undefined;
    let top = 0;
    if (div) {
      const r = div.getBoundingClientRect();
      const c = this.container.getBoundingClientRect();
      top = Math.min(1, Math.max(0, (c.top - r.top) / r.height));
    }
    const label = this.pageLabel(page);
    this.events.relocate({
      locator: { type: "pdf", page, top },
      progress: pages > 1 ? (page - 1 + top) / pages : top,
      label: `Page ${label} of ${pages}`,
      shortLabel: `p. ${label}`,
      page,
      pages,
    });
  }

  async goTo(target: string | Locator) {
    if (typeof target === "string") {
      const dest = this.dests.get(target);
      if (dest) await this.links.goToDestination(dest as string);
      else if (/^\d+$/.test(target)) this.viewer.currentPageNumber = Number(target);
      else {
        // A page label such as "xii".
        const i = this.labels?.indexOf(target) ?? -1;
        if (i >= 0) this.viewer.currentPageNumber = i + 1;
      }
      return;
    }
    if (target.type === "pdf" || target.type === "pdf-highlight") {
      const page = Math.min(Math.max(1, target.page), this.viewer.pagesCount);
      const topFraction =
        target.type === "pdf" ? (target.top ?? 0) : Math.max(0, (target.rects[0]?.[1] ?? 0) - 0.1);
      this.viewer.scrollPageIntoView({ pageNumber: page });
      const div = this.viewer.getPageView(page - 1)?.div as HTMLElement | undefined;
      if (div && topFraction > 0) this.container.scrollTop += div.clientHeight * topFraction;
    }
  }

  next() {
    this.container.scrollBy({ top: this.container.clientHeight * 0.9, behavior: "smooth" });
  }

  prev() {
    this.container.scrollBy({ top: -this.container.clientHeight * 0.9, behavior: "smooth" });
  }

  scrollBy(direction: 1 | -1) {
    this.container.scrollBy({ top: direction * 60, behavior: "smooth" });
  }

  async start() {
    this.viewer.currentPageNumber = 1;
    this.container.scrollTop = 0;
  }

  async end() {
    this.viewer.currentPageNumber = this.viewer.pagesCount;
    this.container.scrollTop = this.container.scrollHeight;
  }

  setLineHeight() {
    /* PDF pages have their own layout. */
  }

  /** Turns the current text selection into a highlight candidate. */
  private onSelection() {
    const sel = document.getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount) return this.events.selection(null);
    const range = sel.getRangeAt(0);
    if (!this.container.contains(range.commonAncestorContainer)) return;
    const pageDiv = (range.startContainer.parentElement ?? null)?.closest<HTMLElement>(".page");
    // Scanned pages: the OCR words stand in for the PDF's text layer.
    const layer =
      pageDiv?.querySelector<HTMLElement>(".lb-ocr-text") ??
      pageDiv?.querySelector<HTMLElement>(".textLayer");
    if (!pageDiv || !layer) return;
    const page = Number(pageDiv.dataset.pageNumber);
    const box = pageDiv.getBoundingClientRect();
    const rects: Rect[] = mergeRects(
      Array.from(range.getClientRects())
        .filter((r) => r.width > 1 && r.height > 1)
        // Only this page (a selection can run into the next one).
        .filter((r) => r.top >= box.top - 2 && r.bottom <= box.bottom + 2)
        .map((r) => [
          (r.left - box.left) / box.width,
          (r.top - box.top) / box.height,
          r.width / box.width,
          r.height / box.height,
        ]),
    );
    const exact = sel.toString().replace(/\s+/g, " ").trim();
    if (!exact || !rects.length) return this.events.selection(null);
    const pageText = (layer.textContent ?? "").replace(/\s+/g, " ");
    const at = pageText.indexOf(exact);
    const quote =
      at >= 0 ? makeQuote(pageText, at, at + exact.length) : { exact, prefix: "", suffix: "" };
    const pages = this.viewer.pagesCount;
    this.events.selection({
      quote,
      locator: { type: "pdf-highlight", page, rects },
      label: `p. ${this.pageLabel(page)}`,
      position: (page - 1 + (rects[0]?.[1] ?? 0)) / pages,
      rect: range.getBoundingClientRect(),
    });
  }

  private hoverHighlights(e: PointerEvent) {
    // Highlights are pointer-events:none so text stays selectable; show a
    // pointer when hovering one so it is clearly clickable.
    const under = document
      .elementsFromPoint(e.clientX, e.clientY)
      .some((el) => el.classList.contains("lb-pdf-hl"));
    this.container.classList.toggle("lb-over-hl", under);
  }

  /** Loads which pages have OCR text (after the book opens). */
  private async loadOcr() {
    if (!this.bookId) return;
    const pages = await unwrap(commands.ocrPages(this.bookId)).catch(() => [] as number[]);
    if (this.destroyed || !pages.length) return;
    this.ocrPages = new Set(pages);
    for (const p of pages) this.drawOcr(p);
  }

  private async wordsOf(page: number): Promise<WordDto[]> {
    const cached = this.ocrWords.get(page);
    if (cached) return cached;
    const list = await unwrap(commands.pageWords(this.bookId!, page)).catch(() => []);
    this.ocrWords.set(page, list);
    return list;
  }

  /** A hidden layer of OCR words over a scanned page, so it can be selected. */
  private async drawOcr(pageNumber: number) {
    if (!this.ocrPages.has(pageNumber)) return;
    const div = this.viewer.getPageView(pageNumber - 1)?.div as HTMLElement | undefined;
    if (!div || !div.querySelector("canvas")) return;
    const words = await this.wordsOf(pageNumber);
    if (this.destroyed || !words.length) return;
    // PDF.js may have drawn its own text layer since; use it if it has words.
    const own = div.querySelector(".textLayer")?.textContent?.trim() ?? "";
    if (own.length > 20) return;
    div.querySelector(".lb-ocr-text")?.remove();
    wordLayer(div, words, "lb-page-text lb-ocr-text");
  }

  private drawPage(pageNumber: number) {
    const view = this.viewer.getPageView(pageNumber - 1);
    const div = view?.div as HTMLElement | undefined;
    if (!div) return;
    void this.drawOcr(pageNumber);
    this.markup?.mount(pageNumber, div);
    this.drawSpoken(pageNumber);
    let layer = div.querySelector<HTMLElement>(".lb-pdf-hl-layer");
    if (!layer) {
      layer = document.createElement("div");
      layer.className = "lb-pdf-hl-layer";
      div.append(layer);
    }
    layer.replaceChildren();
    const dark = this.theme?.dark && this.mode !== "off";
    for (const a of this.annotations) {
      if (!isDrawn(a)) continue;
      let loc: Locator;
      try {
        loc = JSON.parse(a.locator) as Locator;
      } catch {
        continue;
      }
      if (loc.type !== "pdf-highlight" || loc.page !== pageNumber) continue;
      for (const [x, y, w, h] of loc.rects) {
        const r = document.createElement("div");
        r.className = drawnClass("lb-pdf-hl", a);
        r.dataset.annotation = a.id;
        r.style.cssText = `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${h * 100}%;background:${highlightFill(a.color, !!dark)}`;
        layer.append(r);
      }
    }
    const f = this.ocrFind;
    const hit = f?.hits[f.at];
    if (f && hit?.page === pageNumber) {
      for (const [x, y, w, h] of matchRects(
        this.ocrWords.get(pageNumber) ?? [],
        f.query,
        hit.index,
      )) {
        const r = document.createElement("div");
        r.className = "lb-page-find";
        r.style.cssText = `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${h * 100}%`;
        layer.append(r);
      }
    }
  }

  /** Draws the sentence being read aloud on its page. */
  private drawSpoken(pageNumber: number) {
    const div = this.viewer.getPageView(pageNumber - 1)?.div as HTMLElement | undefined;
    if (!div) return;
    div.querySelector(".lb-tts-layer")?.remove();
    if (this.spoken?.page !== pageNumber) return;
    const layer = document.createElement("div");
    layer.className = "lb-tts-layer";
    for (const [x, y, w, h] of this.spoken.rects) {
      const r = document.createElement("div");
      r.className = "lb-tts-mark";
      r.style.cssText = `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${h * 100}%`;
      layer.append(r);
    }
    div.append(layer);
  }

  /** The words of a page: the PDF's text, or OCR words for scans. */
  private async speechWords(page: number): Promise<PageWord[]> {
    const p = await this.doc.getPage(page);
    const vp = p.getViewport({ scale: 1 });
    const content = await p.getTextContent();
    const words: PageWord[] = [];
    for (const it of content.items) {
      if (!("str" in it) || !it.str.trim()) continue;
      const [a, b, , , e, f] = it.transform as number[];
      const h = Math.hypot(a!, b!) || it.height;
      const [x1, y1] = vp.convertToViewportPoint(e!, f! - h * 0.22) as number[];
      const [x2, y2] = vp.convertToViewportPoint(e! + it.width, f! + h * 0.9) as number[];
      const box: SpeechBox = [
        Math.min(x1!, x2!) / vp.width,
        Math.min(y1!, y2!) / vp.height,
        Math.abs(x2! - x1!) / vp.width,
        Math.abs(y2! - y1!) / vp.height,
      ];
      words.push(...runWords(it.str, box));
    }
    if (words.length || !this.ocrPages.has(page)) return words;
    return (await this.wordsOf(page)).map((w) => ({ text: w.text, rect: w.rect as SpeechBox }));
  }

  async readAloud(): Promise<SpeechSource | null> {
    const loc = this.viewer.currentPageNumber;
    const div = this.viewer.getPageView(loc - 1)?.div as HTMLElement | undefined;
    let top = 0;
    if (div) {
      const r = div.getBoundingClientRect();
      const c = this.container.getBoundingClientRect();
      top = Math.min(1, Math.max(0, (c.top - r.top) / r.height));
    }
    return new WordSpeech(
      loc,
      top,
      this.viewer.pagesCount,
      (page) => this.speechWords(page),
      (page, rects) => {
        const before = this.spoken?.page;
        this.spoken = page ? { page, rects } : null;
        if (before && before !== page) this.drawSpoken(before);
        if (page) this.drawSpoken(page);
      },
      (page, y) => {
        const div = this.viewer.getPageView(page - 1)?.div as HTMLElement | undefined;
        const c = this.container.getBoundingClientRect();
        if (div) {
          const r = div.getBoundingClientRect();
          const at = r.top + r.height * y;
          if (at > c.top + 40 && at < c.bottom - 80) return;
        }
        void this.goTo({ type: "pdf", page, top: Math.max(0, y - 0.15) });
      },
    );
  }

  clipPicture(rect: DOMRect) {
    const sources = [];
    for (let i = 0; i < (this.viewer?.pagesCount ?? 0); i++) {
      const div = this.viewer.getPageView(i)?.div as HTMLElement | undefined;
      const canvas = div?.querySelector<HTMLCanvasElement>(".canvasWrapper canvas");
      if (!div || !canvas) continue;
      sources.push({
        page: i + 1,
        div,
        image: () =>
          Promise.resolve({ source: canvas, width: canvas.width, height: canvas.height }),
      });
    }
    return clipFrom(rect, sources);
  }

  async goToFraction(fraction: number) {
    const pages = this.viewer.pagesCount;
    const f = Math.max(0, Math.min(1, fraction)) * pages;
    const page = Math.min(pages, Math.floor(f) + 1);
    await this.goTo({ type: "pdf", page, top: f - (page - 1) });
  }

  attachMarkup(layer: MarkupLayer | null) {
    this.markup = layer;
    this.root?.classList.toggle("lb-has-markup", !!layer);
    for (let p = 1; p <= (this.viewer?.pagesCount ?? 0); p++) {
      const div = this.viewer.getPageView(p - 1)?.div as HTMLElement | undefined;
      if (div?.querySelector("canvas")) layer?.mount(p, div);
    }
  }

  pageSize(page: number): { size: [number, number]; points: boolean } | null {
    const vp = this.viewer?.getPageView(page - 1)?.viewport as
      { width: number; height: number; scale: number } | undefined;
    if (!vp?.scale) return null;
    // PDF.js scales CSS pixels by 96/72; the viewport scale includes it.
    const k = vp.scale / (96 / 72);
    return { size: [vp.width / (96 / 72) / k, vp.height / (96 / 72) / k], points: true };
  }

  setAnnotations(list: Annotation[]) {
    this.annotations = list;
    for (let p = 1; p <= (this.viewer?.pagesCount ?? 0); p++) this.drawPage(p);
  }

  async showAnnotation(a: Annotation) {
    try {
      await this.goTo(JSON.parse(a.locator) as Locator);
    } catch {
      /* ignore */
    }
  }

  async find(query: string, backwards = false): Promise<FindResult> {
    if (this.ocrFind?.query === query) return this.findInOcr(query, backwards);
    const found = await this.findInPdf(query, backwards);
    if (found.total || !this.ocrPages.size) return found;
    return this.findInOcr(query, backwards);
  }

  /** Finds in the OCR text of scanned pages, from the page shown. */
  private async findInOcr(query: string, backwards: boolean): Promise<FindResult> {
    const prevPage = this.ocrFind?.hits[this.ocrFind.at]?.page;
    let f = this.ocrFind;
    if (!f || f.query !== query) {
      this.ocrTexts ??= await unwrap(commands.pageTexts(this.bookId!)).catch(() => []);
      const hits = findHits(this.ocrTexts, query);
      const here = this.viewer.currentPageNumber;
      let at = hits.findIndex((h) => h.page >= here);
      if (at < 0) at = 0;
      if (backwards) at = Math.max(0, at - 1);
      f = this.ocrFind = { query, hits, at };
    } else if (f.hits.length) {
      f.at = (f.at + (backwards ? -1 : 1) + f.hits.length) % f.hits.length;
    }
    const hit = f.hits[f.at];
    if (!hit) return { current: 0, total: 0 };
    await this.wordsOf(hit.page);
    const rects = matchRects(this.ocrWords.get(hit.page) ?? [], query, hit.index);
    await this.goTo({ type: "pdf", page: hit.page, top: Math.max(0, (rects[0]?.[1] ?? 0) - 0.15) });
    if (prevPage && prevPage !== hit.page) this.drawPage(prevPage);
    this.drawPage(hit.page);
    return { current: f.at + 1, total: f.hits.length };
  }

  private findInPdf(query: string, backwards = false): Promise<FindResult> {
    const again = query === this.lastQuery;
    this.lastQuery = query;
    return new Promise((resolve) => {
      const timer = setTimeout(() => resolve({ current: 0, total: 0 }), 4000);
      this.findResolve = (r) => {
        clearTimeout(timer);
        this.findResolve = null;
        resolve(r);
      };
      this.bus.dispatch("find", {
        source: this,
        type: again ? "again" : "",
        query,
        caseSensitive: false,
        entireWord: false,
        highlightAll: true,
        findPrevious: backwards,
        matchDiacritics: false,
      });
    });
  }

  async prepareFind(hint: { page?: number | null; section?: number | null }) {
    if (hint.page) await this.goTo({ type: "pdf", page: hint.page, top: 0 });
  }

  clearFind() {
    const page = this.ocrFind?.hits[this.ocrFind.at]?.page;
    this.ocrFind = null;
    if (page) this.drawPage(page);
    this.lastQuery = "";
    this.bus?.dispatch("findbarclose", { source: this });
  }

  /**
   * Recolours pages to the theme with an SVG filter on the page canvases:
   * white paper becomes the theme's background and black ink its text colour
   * (PDF.js's own recolouring needs canvas filters, which WebKit lacks).
   */
  private recolourFilter(theme: PageTheme): string {
    const hex = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16) / 255);
    const fg = hex(theme.fg);
    const bg = hex(theme.bg);
    const rows = [0, 1, 2]
      .map((c) => {
        const k = bg[c]! - fg[c]!;
        return `${(k * 0.2126).toFixed(4)} ${(k * 0.7152).toFixed(4)} ${(k * 0.0722).toFixed(4)} 0 ${fg[c]!.toFixed(4)}`;
      })
      .join(" ");
    let svg = this.root.querySelector("svg.lb-pdf-filters");
    if (!svg) {
      svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
      svg.setAttribute("class", "lb-pdf-filters");
      svg.setAttribute("width", "0");
      svg.setAttribute("height", "0");
      svg.setAttribute("aria-hidden", "true");
      this.root.prepend(svg);
    }
    const id = `lb-recolour-${Math.random().toString(36).slice(2, 8)}`;
    svg.innerHTML = `<filter id="${id}" color-interpolation-filters="sRGB"><feColorMatrix type="matrix" values="${rows} 0 0 0 1 0"/></filter>`;
    return `url(#${id})`;
  }

  setTheme(theme: PageTheme, mode: PdfDarkMode) {
    this.theme = theme;
    this.mode = mode;
    this.root.style.setProperty("--page-surround", theme.surround);
    const filter =
      theme.id === "original" || mode === "off"
        ? ""
        : mode === "recolour" || !theme.dark
          ? this.recolourFilter(theme)
          : mode === "invert"
            ? "invert(0.9) hue-rotate(180deg)"
            : "brightness(0.72)";
    this.root.style.setProperty("--pdf-filter", filter || "none");
    this.root.classList.toggle("lb-pdf-dark", theme.dark && mode !== "off");
    this.setAnnotations(this.annotations);
  }

  setZoom(zoom: ZoomValue) {
    this.zoomValue = zoom;
    this.viewer.currentScaleValue =
      typeof zoom === "number" ? String(Math.min(5, Math.max(0.25, zoom))) : zoom;
  }

  zoom(): ZoomValue {
    const v = this.viewer?.currentScale;
    return typeof this.zoomValue === "number" ? this.zoomValue : (v ?? 1);
  }

  clearSelection() {
    document.getSelection()?.removeAllRanges();
  }

  async saveForm(): Promise<Uint8Array> {
    return this.doc.saveDocument();
  }
}

/** Joins the many small rectangles of a text selection into one per line. */
function mergeRects(rects: Rect[]): Rect[] {
  const sorted = [...rects].sort((a, b) => a[1] - b[1] || a[0] - b[0]);
  const out: Rect[] = [];
  for (const r of sorted) {
    const last = out[out.length - 1];
    if (last && Math.abs(last[1] - r[1]) < last[3] * 0.5 && r[0] <= last[0] + last[2] + 0.01) {
      const right = Math.max(last[0] + last[2], r[0] + r[2]);
      const bottom = Math.max(last[1] + last[3], r[1] + r[3]);
      last[0] = Math.min(last[0], r[0]);
      last[1] = Math.min(last[1], r[1]);
      last[2] = right - last[0];
      last[3] = bottom - last[1];
    } else {
      out.push([...r] as Rect);
    }
  }
  return out;
}
