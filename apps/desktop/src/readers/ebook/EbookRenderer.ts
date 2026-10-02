/**
 * EPUB, MOBI, AZW3, FB2 and CBZ with foliate-js (MIT): paginated reflowable
 * text, the book's own table of contents, search and an overlay for
 * highlights.
 *
 * Positions and highlights are EPUB CFIs (`{type:"cfi", cfi}`) plus the
 * quoted text. Scripts inside books never run: the app's content security
 * policy blocks them.
 */
import type { Annotation, HighlightColor } from "@/lib/ipc";
import { softLink, type PageTheme } from "../themes";
import { DomSpeech } from "../speech/dom";
import type { SpeechSource } from "../speech/types";
import type {
  FindResult,
  Locator,
  PdfDarkMode,
  Renderer,
  RendererEvents,
  TocItem,
  ZoomValue,
} from "../types";
import { isDrawn } from "../types";
import { mathmlToLatex } from "../math/latex";
import {
  applyBionic,
  BIONIC_CSS,
  bionicFilter,
  lineAt,
  liftBoundary,
  removeBionic,
  type BionicOptions,
  type LineBox,
} from "../focus";

interface FoliateTocItem {
  label: string;
  href: string;
  subitems?: FoliateTocItem[] | null;
}

interface FoliateLocation {
  fraction?: number;
  cfi: string;
  tocItem?: { label?: string };
  pageItem?: { label?: string };
  location?: { current: number; total: number };
}

interface FoliateView extends HTMLElement {
  book: { toc?: FoliateTocItem[]; dir?: string };
  renderer: HTMLElement & {
    setStyles?(css: string): void;
    getContents(): { doc: Document; index: number }[];
    nextSection(): Promise<void>;
    scrollToAnchor(anchor: Range, select?: boolean): Promise<void>;
  };
  lastLocation: (FoliateLocation & { range?: Range }) | null;
  open(book: string | Blob): Promise<void>;
  init(opts: { lastLocation?: string; showTextStart?: boolean }): Promise<void>;
  close(): void;
  goTo(target: string | number): Promise<unknown>;
  goToFraction(f: number): Promise<void>;
  prev(): Promise<void>;
  next(): Promise<void>;
  goLeft(): Promise<void>;
  goRight(): Promise<void>;
  getCFI(index: number, range: Range): string;
  addAnnotation(a: { value: string; color?: string }): Promise<unknown>;
  deleteAnnotation(a: { value: string }): Promise<unknown>;
  search(opts: {
    query: string;
  }): AsyncGenerator<
    "done" | { index?: number; progress?: number; subitems?: { cfi: string; excerpt: unknown }[] }
  >;
  clearSearch(): void;
  deselect(): void;
}

const OPAQUE: Record<HighlightColor, string> = {
  yellow: "#facc15",
  green: "#4ade80",
  blue: "#60a5fa",
  pink: "#f472b6",
};

let loaded: Promise<unknown> | null = null;
let epubcfi: Promise<typeof import("foliate-js/epubcfi.js")> | null = null;
let overlayer: Promise<{ Overlayer: { highlight: unknown; underline: unknown } }> | null = null;

export class EbookRenderer implements Renderer {
  readonly paged = false;
  private view!: FoliateView;
  private tocItems: TocItem[] = [];
  private annotations = new Map<string, Annotation>(); // by CFI
  private scale = 1;
  private lineHeight = 1.55;
  private theme: PageTheme | null = null;
  private findResults: string[] = [];
  /** Chapter of each result, to start from a search result's chapter. */
  private findSections: number[] = [];
  private findFromSection: number | null = null;
  private findIndex = -1;
  private lastQuery = "";
  private cleanup: (() => void)[] = [];
  private bionic: BionicOptions | null = null;

  constructor(private readonly events: RendererEvents) {}

  async open(container: HTMLElement, url: string, initial: Locator | null) {
    loaded ??= import("foliate-js/view.js");
    overlayer ??= import("foliate-js/overlayer.js") as Promise<{
      Overlayer: { highlight: unknown; underline: unknown };
    }>;
    epubcfi ??= import("foliate-js/epubcfi.js");
    await loaded;
    const { Overlayer } = await overlayer;
    const CFI = await epubcfi;

    // foliate-js recognises formats by file name, so keep the extension.
    const res = await fetch(url);
    if (!res.ok) throw new Error(`The file could not be read (${res.status}).`);
    const name = decodeURIComponent(url.split("/").pop() ?? "book");
    const file = new File([await res.blob()], name);

    this.view = document.createElement("foliate-view") as FoliateView;
    this.view.className = "lb-ebook";
    container.append(this.view);
    await this.view.open(file);
    this.positionsIgnoreBionic(CFI);
    this.view.renderer.setAttribute("flow", "paginated");
    this.view.renderer.setAttribute("margin", "48px");
    this.view.renderer.setAttribute("gap", "6%");
    this.view.renderer.setAttribute("max-inline-size", "720px");
    this.applyStyles();

    const toToc = (items: FoliateTocItem[] | null | undefined): TocItem[] =>
      (items ?? []).map((i) => ({
        label: i.label?.trim() ?? "",
        target: i.href,
        children: toToc(i.subitems),
      }));
    this.tocItems = toToc(this.view.book.toc);

    const on = <T>(name: string, f: (detail: T) => void) => {
      const h = (e: Event) => f((e as CustomEvent<T>).detail);
      this.view.addEventListener(name, h);
      this.cleanup.push(() => this.view.removeEventListener(name, h));
    };

    on<FoliateLocation>("relocate", (d) => {
      const fraction = d.fraction ?? 0;
      const section = d.tocItem?.label?.trim();
      const page = d.pageItem?.label;
      const pct = Math.round(fraction * 100);
      this.events.relocate({
        locator: { type: "cfi", cfi: d.cfi },
        progress: fraction,
        label: page ? `Page ${page}` : "",
        shortLabel: page ? `p. ${page}` : (section ?? `${pct}%`),
        section,
      });
    });

    on<{ doc: Document; index: number }>("load", ({ doc, index }) =>
      this.attachToSection(doc, index),
    );

    // A chapter shown again gets its highlights back.
    on<{ index: number }>("create-overlay", () => {
      for (const cfi of this.annotations.keys()) void this.view.addAnnotation({ value: cfi });
    });

    on<{ draw: (f: unknown, opts: unknown) => void; annotation: { value: string } }>(
      "draw-annotation",
      ({ draw, annotation }) => {
        const a = this.annotations.get(annotation.value);
        const under = a?.kind === "voice" || a?.kind === "link";
        draw(under ? Overlayer.underline : Overlayer.highlight, {
          color: a?.kind === "link" ? "#2563eb" : OPAQUE[a?.color ?? "yellow"],
        });
      },
    );

    on<{ value: string; range: Range }>("show-annotation", ({ value, range }) => {
      const a = this.annotations.get(value);
      if (!a) return;
      const frame = range.startContainer.ownerDocument?.defaultView?.frameElement;
      this.events.annotationClick(a.id, this.toScreen(range.getBoundingClientRect(), frame));
    });

    on<{ href: string }>("external-link", (d) => this.events.externalLink?.(d.href));

    const start = initial?.type === "cfi" ? initial.cfi : undefined;
    await this.view.init({ lastLocation: start, showTextStart: !start });
  }

  /**
   * Positions (CFIs) are made and read as if bionic reading's wrappers
   * were not there, so they are the same with it on or off.
   */
  private positionsIgnoreBionic(CFI: typeof import("foliate-js/epubcfi.js")) {
    const view = this.view as unknown as {
      getCFI(index: number, range?: Range): string;
      resolveCFI(cfi: string): { index: number; anchor: unknown };
    };
    const base = view.getCFI.bind(view);
    view.getCFI = (index, range) => {
      if (!range) return base(index);
      const start = liftBoundary(range.startContainer, range.startOffset);
      const end = liftBoundary(range.endContainer, range.endOffset);
      const inner = CFI.fromRange(
        {
          startContainer: start.node,
          startOffset: start.offset,
          endContainer: end.node,
          endOffset: end.offset,
          collapsed: range.collapsed,
        },
        bionicFilter,
      );
      return CFI.joinIndir(base(index), inner);
    };
    const resolve = view.resolveCFI.bind(view);
    view.resolveCFI = (cfi) => {
      const found = resolve(cfi);
      const parts = CFI.parse(cfi);
      (parts.parent ?? parts).shift();
      return {
        index: found.index,
        anchor: (doc: Document) => CFI.toRange(doc, parts, bionicFilter),
      };
    };
  }

  /** A rectangle inside a book page, in window coordinates. */
  private toScreen(r: DOMRect, frame: Element | null | undefined): DOMRect {
    const f = frame?.getBoundingClientRect();
    return new DOMRect(r.left + (f?.left ?? 0), r.top + (f?.top ?? 0), r.width, r.height);
  }

  private attachToSection(doc: Document, index: number) {
    if (this.bionic) this.bionicIn(doc, this.bionic);
    // The app does not see the pointer over the page's frame: pass it on.
    doc.addEventListener("pointermove", (e) => {
      const f = doc.defaultView?.frameElement?.getBoundingClientRect();
      this.events.pointer?.(e.clientX + (f?.left ?? 0), e.clientY + (f?.top ?? 0));
    });
    // Keys pressed while the book has focus still reach the app's shortcuts.
    doc.addEventListener("keydown", (e) => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: e.key,
          code: e.code,
          ctrlKey: e.ctrlKey,
          metaKey: e.metaKey,
          altKey: e.altKey,
          shiftKey: e.shiftKey,
          bubbles: true,
          cancelable: true,
        }),
      );
    });
    const onUp = () =>
      setTimeout(() => {
        const sel = doc.getSelection();
        if (!sel || sel.isCollapsed || !sel.rangeCount) return this.events.selection(null);
        const range = sel.getRangeAt(0);
        const exact = range.toString().replace(/\s+/g, " ").trim();
        if (!exact) return this.events.selection(null);
        const before = doc.createRange();
        before.setStart(doc.body, 0);
        before.setEnd(range.startContainer, range.startOffset);
        const after = doc.createRange();
        after.setStart(range.endContainer, range.endOffset);
        after.setEnd(doc.body, doc.body.childNodes.length);
        const cfi = this.view.getCFI(index, range);
        const loc = this.view.lastLocation;
        this.events.selection({
          quote: {
            exact,
            prefix: before.toString().replace(/\s+/g, " ").slice(-32),
            suffix: after.toString().replace(/\s+/g, " ").slice(0, 32),
          },
          locator: { type: "cfi", cfi },
          label: loc?.pageItem?.label
            ? `p. ${loc.pageItem.label}`
            : (loc?.tocItem?.label?.trim() ?? ""),
          position: loc?.fraction ?? 0,
          rect: this.toScreen(range.getBoundingClientRect(), doc.defaultView?.frameElement),
        });
      }, 0);
    doc.addEventListener("click", (e) => {
      const formula = (e.target as Element | null)?.closest?.("math");
      if (!formula || !doc.getSelection()?.isCollapsed) return;
      const latex = mathmlToLatex(formula);
      if (latex)
        this.events.mathClick?.(
          latex,
          this.toScreen(formula.getBoundingClientRect(), doc.defaultView?.frameElement),
        );
    });
    doc.addEventListener("pointerup", onUp);
    doc.addEventListener("keyup", onUp);
  }

  destroy() {
    for (const f of this.cleanup) f();
    try {
      this.view?.close();
    } catch {
      /* already closed */
    }
    this.view?.remove();
  }

  toc() {
    return this.tocItems;
  }

  async goTo(target: string | Locator) {
    if (typeof target === "string") await this.view.goTo(target);
    else if (target.type === "cfi") await this.view.goTo(target.cfi);
    else if (target.type === "scroll") await this.view.goToFraction(target.fraction);
  }

  next() {
    void (this.view.book.dir === "rtl" ? this.view.goLeft() : this.view.next());
  }

  prev() {
    void (this.view.book.dir === "rtl" ? this.view.goRight() : this.view.prev());
  }

  /** Pages are turned, not scrolled, so a small step is a page. */
  scrollBy(direction: 1 | -1) {
    if (direction > 0) this.next();
    else this.prev();
  }

  async start() {
    await this.view.goToFraction(0);
  }

  async end() {
    await this.view.goToFraction(1);
  }

  setLineHeight(lineHeight: number) {
    this.lineHeight = lineHeight;
    if (this.view?.renderer) this.applyStyles();
  }

  setAnnotations(list: Annotation[]) {
    const next = new Map<string, Annotation>();
    for (const a of list) {
      if (!isDrawn(a)) continue;
      try {
        const loc = JSON.parse(a.locator) as Locator;
        if (loc.type === "cfi") next.set(loc.cfi, a);
      } catch {
        /* skip */
      }
    }
    for (const cfi of this.annotations.keys()) {
      if (!next.has(cfi)) void this.view.deleteAnnotation({ value: cfi });
    }
    this.annotations = next;
    for (const cfi of next.keys()) void this.view.addAnnotation({ value: cfi });
  }

  async showAnnotation(a: Annotation) {
    try {
      const loc = JSON.parse(a.locator) as Locator;
      if (loc.type === "cfi") await this.view.goTo(loc.cfi);
    } catch {
      /* ignore */
    }
  }

  async find(query: string, backwards = false): Promise<FindResult> {
    if (query !== this.lastQuery) {
      this.lastQuery = query;
      this.findResults = [];
      this.findSections = [];
      this.findIndex = -1;
      if (query.trim()) {
        for await (const r of this.view.search({ query })) {
          if (r === "done") break;
          for (const s of r.subitems ?? []) {
            this.findResults.push(s.cfi);
            this.findSections.push(r.index ?? 0);
          }
          if (this.findResults.length >= 1000) break;
        }
      }
    }
    if (this.findFromSection !== null) {
      const from = this.findFromSection;
      this.findFromSection = null;
      const at = this.findSections.findIndex((i) => i >= from);
      // The next step lands on the first match in that chapter (or just before it).
      if (at > 0) this.findIndex = backwards ? at : at - 1;
    }
    const total = this.findResults.length;
    if (!total) return { current: 0, total: 0 };
    // The first step back from a new search goes to the last match.
    this.findIndex =
      this.findIndex < 0
        ? backwards
          ? total - 1
          : 0
        : (this.findIndex + (backwards ? -1 : 1) + total) % total;
    await this.view.goTo(this.findResults[this.findIndex]!);
    return { current: this.findIndex + 1, total };
  }

  async prepareFind(hint: { page?: number | null; section?: number | null }) {
    if (hint.section == null) return;
    this.findFromSection = hint.section;
    try {
      await this.view.goTo(hint.section);
    } catch {
      /* the chapter may not exist any more */
    }
  }

  clearFind() {
    this.lastQuery = "";
    this.findResults = [];
    this.view?.clearSearch();
  }

  private applyStyles() {
    const t = this.theme;
    const css = `
      html { font-size: ${Math.round(this.scale * 100)}% !important;
        font-family: "Iowan Old Style", "Palatino Linotype", Palatino, Georgia, "Noto Serif", serif; }
      ${
        t
          ? `html, body { color: ${t.fg} !important; background: ${t.bg} !important; }
      a[href], a[href] * { color: ${softLink(t)} !important;
        text-decoration: underline 1px !important;
        text-decoration-color: ${softLink(t)}66 !important;
        text-underline-offset: 0.18em; }
      a[href]:hover, a[href]:hover * { text-decoration-color: ${softLink(t)} !important; }`
          : ""
      }
      body { line-height: ${this.lineHeight}; hyphens: auto; }
      img, svg, video { max-width: 100%; }
      ::selection { background-color: rgba(59, 130, 246, 0.3); }
    `;
    // Fixed-layout books (comics, some EPUBs) have no reflowable text to style.
    if (typeof this.view.renderer.setStyles === "function") this.view.renderer.setStyles(css);
    if (t) this.view.style.background = t.surround;
  }

  setTheme(theme: PageTheme, _mode: PdfDarkMode) {
    this.theme = theme;
    if (this.view?.renderer) this.applyStyles();
  }

  setZoom(zoom: ZoomValue) {
    this.scale = typeof zoom === "number" ? Math.min(2.2, Math.max(0.6, zoom)) : 1;
    this.applyStyles();
  }

  zoom(): ZoomValue {
    return this.scale;
  }

  async readAloud(): Promise<SpeechSource | null> {
    const current = this.view.renderer.getContents()[0];
    if (!current?.doc.body) return null;
    const range = this.view.lastLocation?.range;
    const renderer = this.view.renderer;
    return new DomSpeech(
      { root: current.doc.body, from: range?.startContainer },
      (r) => void renderer.scrollToAnchor(r, false),
      async () => {
        // The next chapter, until the end of the book.
        const before = renderer.getContents()[0]?.index;
        await renderer.nextSection();
        const next = renderer.getContents()[0];
        return next && next.index !== before ? next.doc.body : null;
      },
    );
  }

  private bionicIn(doc: Document, options: BionicOptions | null) {
    // Fixed-layout books (comics, some EPUBs) are laid out by the book.
    if (!doc.body || typeof this.view.renderer.setStyles !== "function") return;
    removeBionic(doc.body);
    let style = doc.getElementById("lb-bionic-css");
    if (options) {
      if (!style) {
        style = doc.createElement("style");
        style.id = "lb-bionic-css";
        style.textContent = BIONIC_CSS;
        (doc.head ?? doc.documentElement).append(style);
      }
      applyBionic(doc.body, options);
    } else style?.remove();
  }

  setBionic(options: BionicOptions | null) {
    if (JSON.stringify(options) === JSON.stringify(this.bionic)) return;
    this.bionic = options;
    if (!this.view?.renderer) return;
    const here = this.view.lastLocation?.cfi;
    for (const { doc } of this.view.renderer.getContents()) this.bionicIn(doc, options);
    // The text moved: draw highlights again and stay at the same place.
    for (const cfi of this.annotations.keys()) void this.view.addAnnotation({ value: cfi });
    if (here) void this.view.goTo(here);
  }

  lineAt(x: number, y: number): LineBox | null {
    for (const { doc } of this.view?.renderer?.getContents() ?? []) {
      const f = doc.defaultView?.frameElement?.getBoundingClientRect();
      if (!f || x < f.left || x > f.right || y < f.top || y > f.bottom) continue;
      const box = lineAt(doc, x - f.left, y - f.top);
      if (!box) return null;
      return {
        top: box.top + f.top,
        bottom: box.bottom + f.top,
        left: box.left + f.left,
        right: box.right + f.left,
      };
    }
    return null;
  }

  async goToFraction(fraction: number) {
    await this.view.goToFraction(Math.max(0, Math.min(1, fraction)));
  }

  clearSelection() {
    for (const { doc } of this.view?.renderer?.getContents() ?? [])
      doc.getSelection()?.removeAllRanges();
  }
}
