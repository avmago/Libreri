/**
 * Books made of page images: comics (CBZ, CBR, CB7, CBT, CBA) and DjVu.
 *
 * Pages come from `book://localhost/.pages/<id>/<page>?w=<width>`; Rust
 * reads them from the archive or renders them with DjVuLibre and caches
 * them on this computer. Layouts: continuous scrolling (DjVu, and comics'
 * "webtoon" mode), one page at a time, or two pages side by side (the cover
 * alone), left to right or right to left (manga).
 *
 * DjVu pages with a text layer get invisible words over the image, so text
 * can be selected, highlighted and found. Highlights use the same locator
 * as PDFs (`{type:"pdf-highlight", page, rects}`), so notes, links and
 * exports treat both alike.
 */
import { clipFrom, imageOf } from "../clip";
import { bookUrl, commands, unwrap, type Annotation, type WordDto } from "@/lib/ipc";
import type { MarkupLayer } from "../markup/MarkupLayer";
import { makeQuote } from "../quote";
import type { PageTheme } from "../themes";
import {
  highlightFill,
  type FindResult,
  type Locator,
  type PageLayout,
  type PdfDarkMode,
  type Rect,
  type Renderer,
  type RendererEvents,
  type TocItem,
  type ZoomValue,
  drawnClass,
  isDrawn,
} from "../types";
import { WordSpeech, type Box as SpeechBox } from "../speech/words";
import type { SpeechSource } from "../speech/types";

const LAYOUT_KEY = (id: string) => `libreri.pages.${id}`;
const WIDTHS = [480, 800, 1200, 1600, 2000, 2600, 3200];

function savedLayout(id: string): Partial<PageLayout> {
  try {
    return JSON.parse(localStorage.getItem(LAYOUT_KEY(id)) ?? "{}") as Partial<PageLayout>;
  } catch {
    return {};
  }
}

function norm(s: string): string {
  return s.normalize("NFKD").replace(/\p{M}/gu, "").toLowerCase();
}

interface PageSlot {
  n: number;
  div: HTMLDivElement;
  img: HTMLImageElement | null;
  /** Width / height, measured or known. */
  ratio: number;
  loadedWidth: number;
  textDone: boolean;
}

export class PageRenderer implements Renderer {
  readonly paged = true;
  private markup: MarkupLayer | null = null;
  private pixelSizes: [number, number][] = [];
  private root!: HTMLDivElement;
  private scroller!: HTMLDivElement;
  private stage!: HTMLDivElement;
  private slots: PageSlot[] = [];
  private kind: "comic" | "djvu" = "comic";
  private hasText = false;
  private tocItems: TocItem[] = [];
  private layoutValue: PageLayout = { mode: "single", rightToLeft: false };
  private zoomValue: ZoomValue = "page-fit";
  /** Current page (single/spread modes). */
  private current = 1;
  private annotations: Annotation[] = [];
  private theme: PageTheme | null = null;
  private mode: PdfDarkMode = "recolour";
  private words = new Map<number, WordDto[]>();
  private texts: string[] | null = null;
  private findState: { query: string; hits: { page: number; index: number }[]; at: number } | null =
    null;
  private observer: IntersectionObserver | null = null;
  /** The sentence read aloud: its page and line boxes. */
  private spoken: { page: number; rects: SpeechBox[] } | null = null;
  private cleanup: (() => void)[] = [];
  private destroyed = false;

  constructor(
    private readonly events: RendererEvents,
    private readonly bookId: string,
  ) {}

  get rightToLeft(): boolean {
    return this.layoutValue.rightToLeft && this.layoutValue.mode !== "scroll";
  }

  async open(host: HTMLElement, _url: string, initial: Locator | null) {
    const info = await unwrap(commands.openPages(this.bookId));
    if (this.destroyed) return;
    this.kind = info.kind === "djvu" ? "djvu" : "comic";
    this.hasText = info.hasText;
    let n = 0;
    const convert = (items: typeof info.outline): TocItem[] =>
      items.map((i) => ({
        label: i.title,
        target: i.page ? String(i.page) : `toc:${n++}`,
        children: convert(i.children),
      }));
    this.tocItems = convert(info.outline);

    const saved = savedLayout(this.bookId);
    this.layoutValue =
      this.kind === "djvu"
        ? { mode: "scroll", rightToLeft: false }
        : {
            mode: saved.mode ?? "single",
            rightToLeft: saved.rightToLeft ?? info.rightToLeft,
          };
    this.zoomValue = this.layoutValue.mode === "scroll" ? "page-width" : "page-fit";

    this.root = document.createElement("div");
    this.root.className = `lb-pages lb-pages-${this.kind}`;
    this.scroller = document.createElement("div");
    this.scroller.className = "lb-pages-scroller";
    this.stage = document.createElement("div");
    this.stage.className = "lb-pages-stage";
    this.scroller.append(this.stage);
    this.root.append(this.scroller);
    host.append(this.root);

    this.pixelSizes = info.sizes.map(([w, h]) => [w ?? 0, h ?? 0] as [number, number]);
    for (let i = 1; i <= info.pages; i++) {
      const size = info.sizes[i - 1];
      const div = document.createElement("div");
      div.className = "lb-page";
      div.dataset.page = String(i);
      this.slots.push({
        n: i,
        div,
        img: null,
        ratio: size && size[1] ? size[0] / size[1] : 2 / 3,
        loadedWidth: 0,
        textDone: false,
      });
    }

    this.observer = new IntersectionObserver((entries) => this.onVisible(entries), {
      root: this.scroller,
      rootMargin: "150% 0px",
    });
    this.cleanup.push(() => this.observer?.disconnect());

    const resize = new ResizeObserver(() => this.layout(true));
    resize.observe(this.scroller);
    this.cleanup.push(() => resize.disconnect());
    this.scroller.addEventListener("scroll", this.onScroll, { passive: true });
    const onUp = () => setTimeout(() => this.onSelection(), 0);
    this.scroller.addEventListener("pointerup", onUp);
    const onClick = (e: MouseEvent) => this.onClick(e);
    this.scroller.addEventListener("click", onClick);
    this.cleanup.push(() => {
      this.scroller.removeEventListener("scroll", this.onScroll);
      this.scroller.removeEventListener("pointerup", onUp);
      this.scroller.removeEventListener("click", onClick);
    });

    this.layout(false);
    if (initial) await this.goTo(initial);
    this.emitLocation();
  }

  destroy() {
    this.destroyed = true;
    for (const f of this.cleanup) f();
    this.root?.remove();
  }

  // ---------- layout ----------

  /** The layout of comics (DjVu always scrolls). */
  layoutOptions(): PageLayout | null {
    return this.kind === "comic" ? { ...this.layoutValue } : null;
  }

  setLayout(layout: Partial<PageLayout>) {
    if (this.kind !== "comic") return;
    const page = this.visiblePage();
    const modeChanged = layout.mode && layout.mode !== this.layoutValue.mode;
    this.layoutValue = { ...this.layoutValue, ...layout };
    try {
      localStorage.setItem(LAYOUT_KEY(this.bookId), JSON.stringify(this.layoutValue));
    } catch {
      // Not remembered; the layout still changes.
    }
    if (modeChanged)
      this.zoomValue = this.layoutValue.mode === "scroll" ? "page-width" : "page-fit";
    this.current = page;
    this.layout(false);
    void this.showPage(page);
  }

  /** Pages shown together in single/spread modes: [left, right] order on screen. */
  private group(page: number): number[] {
    const total = this.slots.length;
    if (this.layoutValue.mode !== "spread" || page === 1) return [page];
    const first = page % 2 === 0 ? page : page - 1;
    const pair = first + 1 <= total ? [first, first + 1] : [first];
    return this.layoutValue.rightToLeft ? pair.reverse() : pair;
  }

  private pageWidth(slot: PageSlot, perRow: number): number {
    const avail = Math.max(100, this.scroller.clientWidth - (this.kind === "djvu" ? 48 : 0));
    const availH = Math.max(
      100,
      this.scroller.clientHeight - (this.layoutValue.mode === "scroll" ? 0 : 16),
    );
    const z = this.zoomValue;
    const fitWidth = avail / perRow;
    const fitPage = Math.min(fitWidth, availH * slot.ratio);
    if (z === "page-fit") return fitPage;
    if (z === "page-width") return fitWidth;
    if (z === "auto") return Math.min(fitWidth, 1000);
    return Math.max(80, fitWidth * z);
  }

  private layout(keepPlace: boolean) {
    if (!this.stage) return;
    const place = keepPlace ? this.visiblePage() : this.current;
    const mode = this.layoutValue.mode;
    this.stage.className = `lb-pages-stage lb-mode-${mode}${this.layoutValue.rightToLeft ? " lb-rtl" : ""}`;
    if (mode === "scroll") {
      if (this.stage.childElementCount !== this.slots.length) {
        this.stage.replaceChildren(...this.slots.map((s) => s.div));
      }
      for (const s of this.slots) {
        const w = this.pageWidth(s, 1);
        s.div.style.width = `${w}px`;
        s.div.style.height = `${w / s.ratio}px`;
        this.observer?.observe(s.div);
      }
    } else {
      this.observer?.disconnect();
      const pages = this.group(place);
      this.stage.replaceChildren(...pages.map((p) => this.slots[p - 1]!.div));
      for (const p of pages) {
        const s = this.slots[p - 1]!;
        const w = this.pageWidth(s, pages.length);
        s.div.style.width = `${w}px`;
        s.div.style.height = `${w / s.ratio}px`;
        this.load(s);
      }
      // Load the next pages early so turning is instant.
      for (const p of [place + 1, place + 2, place + 3]) {
        const s = this.slots[p - 1];
        if (s) this.prefetch(s);
      }
    }
    this.drawAll();
    if (keepPlace && mode === "scroll") this.scrollToPage(place, 0);
  }

  private urlFor(s: PageSlot): { url: string; width: number } {
    const cssWidth = parseFloat(s.div.style.width) || 800;
    const want = cssWidth * (window.devicePixelRatio || 1);
    const width = WIDTHS.find((w) => w >= want) ?? WIDTHS[WIDTHS.length - 1]!;
    const url = `${bookUrl(`.pages/${this.bookId}/${s.n}`)}${this.kind === "djvu" ? `?w=${width}` : ""}`;
    return { url, width: this.kind === "djvu" ? width : Number.MAX_SAFE_INTEGER };
  }

  private prefetch(s: PageSlot) {
    if (s.img) return;
    const img = new Image();
    img.src = this.urlFor(s).url;
  }

  private load(s: PageSlot) {
    const { url, width } = this.urlFor(s);
    if (s.img && s.loadedWidth >= width) return;
    const img = s.img ?? document.createElement("img");
    img.className = "lb-page-img";
    img.decoding = "async";
    img.draggable = false;
    img.alt = `Page ${s.n}`;
    img.onload = () => {
      if (this.kind === "comic" && img.naturalWidth && img.naturalHeight) {
        const ratio = img.naturalWidth / img.naturalHeight;
        if (Math.abs(ratio - s.ratio) > 0.01) {
          const before = s.div.getBoundingClientRect();
          const above = before.bottom < this.scroller.getBoundingClientRect().top;
          const oldH = before.height;
          s.ratio = ratio;
          const w = parseFloat(s.div.style.width);
          s.div.style.height = `${w / ratio}px`;
          if (this.layoutValue.mode !== "scroll") this.layout(false);
          else if (above) this.scroller.scrollTop += w / ratio - oldH;
        }
      }
      void this.loadText(s);
    };
    img.onerror = () => {
      s.div.classList.add("lb-page-error");
      void fetch(url)
        .then((r) => (r.ok ? "" : r.text()))
        .then((msg) => {
          if (msg) s.div.dataset.error = msg;
        })
        .catch(() => {});
    };
    img.src = url;
    s.loadedWidth = width;
    if (!s.img) {
      s.img = img;
      s.div.prepend(img);
    }
  }

  private onVisible(entries: IntersectionObserverEntry[]) {
    for (const e of entries) {
      if (!e.isIntersecting) continue;
      const n = Number((e.target as HTMLElement).dataset.page);
      const s = this.slots[n - 1];
      if (s) this.load(s);
    }
  }

  // ---------- text (DjVu) ----------

  private async wordsOf(page: number): Promise<WordDto[]> {
    const cached = this.words.get(page);
    if (cached) return cached;
    const list = this.hasText
      ? await unwrap(commands.pageWords(this.bookId, page)).catch(() => [])
      : [];
    this.words.set(page, list);
    return list;
  }

  private async loadText(s: PageSlot) {
    if (!this.hasText || s.textDone) return;
    s.textDone = true;
    const words = await this.wordsOf(s.n);
    if (this.destroyed || !words.length) return;
    const layer = document.createElement("div");
    layer.className = "lb-page-text";
    const h = s.div.clientHeight || 1000;
    for (const w of words) {
      const span = document.createElement("span");
      const [x, y, ww, wh] = w.rect.map((v) => v ?? 0) as Rect;
      span.textContent = `${w.text} `;
      span.style.cssText = `left:${x * 100}%;top:${y * 100}%;height:${wh * 100}%;font-size:${wh * h * 0.85}px;--w:${ww}`;
      layer.append(span);
    }
    s.div.append(layer);
    // Stretch each word to its box.
    requestAnimationFrame(() => {
      const pw = s.div.clientWidth;
      for (const span of Array.from(layer.children) as HTMLElement[]) {
        const target = parseFloat(span.style.getPropertyValue("--w")) * pw;
        const natural = span.offsetWidth;
        if (natural > 0) span.style.transform = `scaleX(${target / natural})`;
      }
    });
    this.drawPage(s.n);
  }

  private pageText(page: number): string {
    return (this.words.get(page) ?? []).map((w) => w.text).join(" ");
  }

  private onSelection() {
    const sel = document.getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount) return this.events.selection(null);
    const range = sel.getRangeAt(0);
    if (!this.scroller.contains(range.commonAncestorContainer)) return;
    const pageDiv = range.startContainer.parentElement?.closest<HTMLElement>(".lb-page");
    if (!pageDiv) return;
    const page = Number(pageDiv.dataset.page);
    const box = pageDiv.getBoundingClientRect();
    const rects: Rect[] = mergeRects(
      Array.from(range.getClientRects())
        .filter((r) => r.width > 1 && r.height > 1)
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
    const text = this.pageText(page);
    const at = text.indexOf(exact);
    const quote =
      at >= 0 ? makeQuote(text, at, at + exact.length) : { exact, prefix: "", suffix: "" };
    this.events.selection({
      quote,
      locator: { type: "pdf-highlight", page, rects },
      label: `p. ${page}`,
      position: (page - 1 + (rects[0]?.[1] ?? 0)) / this.slots.length,
      rect: range.getBoundingClientRect(),
    });
  }

  private onClick(e: MouseEvent) {
    // Drawing with a markup tool is not a page turn.
    if ((e.target as Element | null)?.closest?.("svg.lb-markup-active")) return;
    const hit = document
      .elementsFromPoint(e.clientX, e.clientY)
      .find((el): el is HTMLElement => el.classList.contains("lb-pdf-hl"));
    if (hit && document.getSelection()?.isCollapsed) {
      this.events.annotationClick(hit.dataset.annotation!, hit.getBoundingClientRect());
      return;
    }
    // Comics: click the left or right third of the page to turn.
    if (this.kind !== "comic" || this.layoutValue.mode === "scroll") return;
    const r = this.scroller.getBoundingClientRect();
    const x = (e.clientX - r.left) / r.width;
    const forward = this.layoutValue.rightToLeft ? x < 0.33 : x > 0.67;
    const back = this.layoutValue.rightToLeft ? x > 0.67 : x < 0.33;
    if (forward) this.next();
    else if (back) this.prev();
  }

  // ---------- highlights ----------

  private drawPage(page: number) {
    const s = this.slots[page - 1];
    if (!s) return;
    let layer = s.div.querySelector<HTMLElement>(".lb-pdf-hl-layer");
    if (!layer) {
      layer = document.createElement("div");
      layer.className = "lb-pdf-hl-layer";
      s.div.append(layer);
    }
    layer.replaceChildren();
    const dark = !!(this.theme?.dark && this.mode !== "off");
    for (const a of this.annotations) {
      if (!isDrawn(a)) continue;
      let loc: Locator;
      try {
        loc = JSON.parse(a.locator) as Locator;
      } catch {
        continue;
      }
      if (loc.type !== "pdf-highlight" || loc.page !== page) continue;
      for (const [x, y, w, h] of loc.rects) {
        const r = document.createElement("div");
        r.className = drawnClass("lb-pdf-hl", a);
        r.dataset.annotation = a.id;
        r.style.cssText = `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${h * 100}%;background:${highlightFill(a.color, dark)}`;
        layer.append(r);
      }
    }
    const f = this.findState;
    const hit = f?.hits[f.at];
    if (hit && hit.page === page) {
      for (const rect of this.matchRects(page, f!.query, hit.index)) {
        const r = document.createElement("div");
        r.className = "lb-page-find";
        r.style.cssText = `left:${rect[0] * 100}%;top:${rect[1] * 100}%;width:${rect[2] * 100}%;height:${rect[3] * 100}%`;
        layer.append(r);
      }
    }
    if (this.spoken?.page === page) {
      for (const [x, y, w, h] of this.spoken.rects) {
        const r = document.createElement("div");
        r.className = "lb-tts-mark";
        r.style.cssText = `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;height:${h * 100}%`;
        layer.append(r);
      }
    }
    if (s.div.clientWidth > 0) this.markup?.mount(page, s.div);
  }

  async readAloud(): Promise<SpeechSource | null> {
    if (this.kind !== "djvu" && !this.hasText) return null;
    const page = this.visiblePage();
    let top = 0;
    if (this.layoutValue.mode === "scroll") {
      const r = this.slots[page - 1]!.div.getBoundingClientRect();
      const c = this.scroller.getBoundingClientRect();
      top = Math.min(1, Math.max(0, (c.top - r.top) / r.height));
    }
    return new WordSpeech(
      page,
      top,
      this.slots.length,
      async (p) =>
        (await this.wordsOf(p)).map((w) => ({ text: w.text, rect: w.rect as SpeechBox })),
      (p, rects) => {
        const before = this.spoken?.page;
        this.spoken = p ? { page: p, rects } : null;
        if (before && before !== p) this.drawPage(before);
        if (p) this.drawPage(p);
      },
      (p, y) => {
        const s = this.slots[p - 1];
        if (!s) return;
        if (this.layoutValue.mode !== "scroll") {
          if (!this.group(this.current).includes(p)) void this.showPage(p);
          return;
        }
        const r = s.div.getBoundingClientRect();
        const c = this.scroller.getBoundingClientRect();
        const at = r.top + r.height * y;
        if (at < c.top + 40 || at > c.bottom - 80) void this.showPage(p, Math.max(0, y - 0.15));
      },
    );
  }

  async goToFraction(fraction: number) {
    const pages = this.slots.length;
    const f = Math.max(0, Math.min(1, fraction)) * pages;
    const page = Math.min(pages, Math.floor(f) + 1);
    await this.showPage(page, f - (page - 1));
  }

  clipPicture(rect: DOMRect) {
    return clipFrom(
      rect,
      this.slots
        .filter((s) => s.div.isConnected)
        .map((s) => ({ page: s.n, div: s.div, image: () => imageOf(s.img) })),
    );
  }

  attachMarkup(layer: MarkupLayer | null) {
    this.markup = layer;
    this.drawAll();
  }

  pageSize(page: number): { size: [number, number]; points: boolean } | null {
    const known = this.pixelSizes[page - 1];
    if (known) return { size: known, points: false };
    const img = this.slots[page - 1]?.img;
    if (img?.naturalWidth) return { size: [img.naturalWidth, img.naturalHeight], points: false };
    return null;
  }

  private drawAll() {
    for (const s of this.slots) if (s.div.isConnected) this.drawPage(s.n);
  }

  setAnnotations(list: Annotation[]) {
    this.annotations = list;
    this.drawAll();
  }

  async showAnnotation(a: Annotation) {
    try {
      await this.goTo(JSON.parse(a.locator) as Locator);
    } catch {
      /* ignore */
    }
  }

  // ---------- moving around ----------

  private visiblePage(): number {
    if (this.layoutValue.mode !== "scroll") return this.current;
    const top = this.scroller.getBoundingClientRect().top + 8;
    for (const s of this.slots) {
      const r = s.div.getBoundingClientRect();
      if (r.bottom > top) return s.n;
    }
    return this.slots.length;
  }

  private frame = 0;
  private onScroll = () => {
    cancelAnimationFrame(this.frame);
    this.frame = requestAnimationFrame(() => this.emitLocation());
  };

  private emitLocation() {
    const pages = this.slots.length;
    if (!pages) return;
    const page = this.visiblePage();
    let top = 0;
    if (this.layoutValue.mode === "scroll") {
      const r = this.slots[page - 1]!.div.getBoundingClientRect();
      const c = this.scroller.getBoundingClientRect();
      top = Math.min(1, Math.max(0, (c.top - r.top) / r.height));
    }
    const shown =
      this.layoutValue.mode === "spread" ? this.group(page).sort((a, b) => a - b) : [page];
    const label = shown.length > 1 ? `${shown[0]}–${shown[1]}` : String(page);
    this.events.relocate({
      locator: { type: "pdf", page, top },
      progress: pages > 1 ? (page - 1 + top) / pages : top,
      label: `Page ${label} of ${pages}`,
      shortLabel: `p. ${page}`,
      page,
      pages,
      section: this.sectionOf(page),
    });
  }

  private sectionOf(page: number): string | undefined {
    let found: string | undefined;
    const walk = (items: TocItem[]) => {
      for (const i of items) {
        const p = Number(i.target);
        if (p && p <= page) found = i.label;
        walk(i.children);
      }
    };
    walk(this.tocItems);
    return found;
  }

  private scrollToPage(page: number, top: number) {
    const s = this.slots[page - 1];
    if (!s) return;
    const r = s.div.getBoundingClientRect();
    const c = this.scroller.getBoundingClientRect();
    this.scroller.scrollTop += r.top - c.top + r.height * top - (top > 0 ? 40 : 0);
  }

  private async showPage(page: number, top = 0) {
    const p = Math.min(Math.max(1, page), this.slots.length);
    if (this.layoutValue.mode === "scroll") {
      this.scrollToPage(p, top);
    } else {
      this.current = p;
      this.layout(false);
      this.scroller.scrollTop = 0;
    }
    this.emitLocation();
  }

  toc() {
    return this.tocItems;
  }

  async goTo(target: string | Locator) {
    if (typeof target === "string") {
      if (/^\d+$/.test(target)) await this.showPage(Number(target));
      return;
    }
    if (target.type === "pdf") await this.showPage(target.page, target.top ?? 0);
    else if (target.type === "pdf-highlight")
      await this.showPage(target.page, Math.max(0, (target.rects[0]?.[1] ?? 0) - 0.1));
  }

  /** Pages to move by in single/spread modes. */
  private step(dir: 1 | -1): number {
    const cur = this.current;
    if (this.layoutValue.mode !== "spread") return cur + dir;
    if (dir > 0) return cur === 1 ? 2 : (cur % 2 === 0 ? cur : cur - 1) + 2;
    if (cur <= 3) return 1;
    return (cur % 2 === 0 ? cur : cur - 1) - 2;
  }

  next() {
    if (this.layoutValue.mode === "scroll") {
      this.scroller.scrollBy({ top: this.scroller.clientHeight * 0.9, behavior: "smooth" });
      return;
    }
    // A zoomed page scrolls first, then turns.
    const s = this.scroller;
    if (s.scrollTop + s.clientHeight < s.scrollHeight - 4) {
      s.scrollBy({ top: s.clientHeight * 0.9, behavior: "smooth" });
      return;
    }
    const n = this.step(1);
    if (n <= this.slots.length) void this.showPage(n);
  }

  prev() {
    if (this.layoutValue.mode === "scroll") {
      this.scroller.scrollBy({ top: -this.scroller.clientHeight * 0.9, behavior: "smooth" });
      return;
    }
    if (this.scroller.scrollTop > 4) {
      this.scroller.scrollBy({ top: -this.scroller.clientHeight * 0.9, behavior: "smooth" });
      return;
    }
    const n = this.step(-1);
    if (n >= 1) void this.showPage(n);
  }

  scrollBy(direction: 1 | -1) {
    this.scroller.scrollBy({ top: direction * 60, behavior: "smooth" });
  }

  async start() {
    await this.showPage(1);
  }

  async end() {
    await this.showPage(this.slots.length);
  }

  setLineHeight() {
    /* Pages are images. */
  }

  // ---------- find (DjVu text) ----------

  /** Where the `index`th match of `query` on `page` is, as rectangles. */
  private matchRects(page: number, query: string, index: number): Rect[] {
    const words = this.words.get(page);
    if (!words?.length) return [];
    const q = norm(query).replace(/\s+/g, " ").trim();
    let text = "";
    const starts: number[] = [];
    for (const w of words) {
      starts.push(text.length);
      text += `${norm(w.text)} `;
    }
    let at = -1;
    for (let i = 0; i <= index; i++) {
      at = text.indexOf(q, at + 1);
      if (at < 0) return [];
    }
    const end = at + q.length;
    return words
      .filter((_, i) => starts[i]! < end && starts[i]! + norm(words[i]!.text).length > at)
      .map((w) => w.rect.map((v) => v ?? 0) as Rect);
  }

  async find(query: string, backwards = false): Promise<FindResult> {
    if (!this.hasText || !query.trim()) return { current: 0, total: 0 };
    const f = this.findState;
    if (!f || f.query !== query) {
      this.texts ??= await unwrap(commands.pageTexts(this.bookId)).catch(() => []);
      const q = norm(query).replace(/\s+/g, " ").trim();
      const hits: { page: number; index: number }[] = [];
      this.texts.forEach((t, i) => {
        const text = norm(t).replace(/\s+/g, " ");
        let at = text.indexOf(q);
        let k = 0;
        while (at >= 0) {
          hits.push({ page: i + 1, index: k++ });
          at = text.indexOf(q, at + 1);
        }
      });
      const here = this.visiblePage();
      let at = hits.findIndex((h) => h.page >= here);
      if (at < 0) at = 0;
      if (backwards) at = Math.max(0, at - 1);
      this.findState = { query, hits, at };
    } else if (f.hits.length) {
      f.at = (f.at + (backwards ? -1 : 1) + f.hits.length) % f.hits.length;
    }
    const state = this.findState!;
    const hit = state.hits[state.at];
    if (!hit) {
      this.drawAll();
      return { current: 0, total: 0 };
    }
    await this.wordsOf(hit.page);
    const rects = this.matchRects(hit.page, query, hit.index);
    await this.showPage(hit.page, Math.max(0, (rects[0]?.[1] ?? 0) - 0.15));
    this.drawAll();
    return { current: state.at + 1, total: state.hits.length };
  }

  async prepareFind(hint: { page?: number | null; section?: number | null }) {
    if (hint.page) await this.goTo({ type: "pdf", page: hint.page, top: 0 });
  }

  clearFind() {
    this.findState = null;
    this.drawAll();
  }

  // ---------- look ----------

  setTheme(theme: PageTheme, mode: PdfDarkMode) {
    this.theme = theme;
    this.mode = mode;
    this.root.style.setProperty("--page-surround", theme.surround);
    const dark = theme.dark && mode !== "off";
    // Scans read well inverted; comic art is only ever dimmed unless asked.
    const filter = !dark
      ? "none"
      : mode === "dim" || (this.kind === "comic" && mode === "recolour")
        ? "brightness(0.78)"
        : "invert(0.9) hue-rotate(180deg)";
    this.root.style.setProperty("--page-filter", filter);
    this.root.classList.toggle("lb-pdf-dark", dark);
    this.drawAll();
  }

  setZoom(zoom: ZoomValue) {
    this.zoomValue = typeof zoom === "number" ? Math.min(5, Math.max(0.25, zoom)) : zoom;
    this.layout(true);
    this.emitLocation();
  }

  zoom(): ZoomValue {
    return this.zoomValue;
  }

  clearSelection() {
    document.getSelection()?.removeAllRanges();
  }
}

/** Joins the small rectangles of a selection into one per line. */
function mergeRects(rects: Rect[]): Rect[] {
  const sorted = [...rects].sort((a, b) => a[1] - b[1] || a[0] - b[0]);
  const out: Rect[] = [];
  for (const r of sorted) {
    const last = out[out.length - 1];
    if (last && Math.abs(last[1] - r[1]) < last[3] * 0.5 && r[0] <= last[0] + last[2] + 0.02) {
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
