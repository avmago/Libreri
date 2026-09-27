/**
 * Markdown and plain-text books: rendered as one scrolling HTML document.
 *
 * Positions and highlights are character offsets into the rendered text
 * (`{type:"text", start, end}`) with a text quote as fallback, so they
 * survive small edits to the file.
 */
import { renderMarkdown, renderMermaid } from "@/lib/markdown";
import type { Annotation } from "@/lib/ipc";
import { findQuote, makeQuote } from "../quote";
import type { PageTheme } from "../themes";
import {
  highlightFill,
  type FindResult,
  type Locator,
  type PdfDarkMode,
  type Renderer,
  type RendererEvents,
  type TocItem,
  type ZoomValue,
} from "../types";

const BASE_FONT = 17;

/** Every text node under `root`, in order. */
function textNodes(root: Node): Text[] {
  const out: Text[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) out.push(n as Text);
  return out;
}

/** Character offset of (node, offset) within `root`'s text. */
function offsetOf(root: Node, node: Node, offset: number): number {
  let total = 0;
  for (const t of textNodes(root)) {
    if (t === node) return total + offset;
    if (node.contains(t) || node === t) break;
    total += t.data.length;
  }
  // `node` is an element: count the text before its offset-th child.
  if (node.nodeType === Node.ELEMENT_NODE) {
    const range = document.createRange();
    range.setStart(root, 0);
    range.setEnd(node, offset);
    return range.toString().length;
  }
  return total;
}

/** Wraps characters [start, end) of `root`'s text in marks made by `make`. */
function wrapRange(
  root: HTMLElement,
  start: number,
  end: number,
  make: () => HTMLElement,
): HTMLElement[] {
  const marks: HTMLElement[] = [];
  let pos = 0;
  for (const node of textNodes(root)) {
    const len = node.data.length;
    const from = Math.max(start, pos);
    const to = Math.min(end, pos + len);
    if (from < to) {
      let target = node;
      if (from > pos) target = target.splitText(from - pos);
      if (to < pos + len) target.splitText(to - from);
      // Keep maths and diagrams intact: skip text inside them.
      if (!target.parentElement?.closest(".katex, svg, .mermaid")) {
        const mark = make();
        target.parentNode?.insertBefore(mark, target);
        mark.appendChild(target);
        marks.push(mark);
      }
    }
    pos += len;
    if (pos >= end) break;
  }
  return marks;
}

function unwrap(marks: Iterable<Element>) {
  for (const m of Array.from(marks)) {
    const parent = m.parentNode;
    if (!parent) continue;
    while (m.firstChild) parent.insertBefore(m.firstChild, m);
    parent.removeChild(m);
    parent.normalize();
  }
}

export class DocumentRenderer implements Renderer {
  readonly paged = false;
  private scroller!: HTMLDivElement;
  private article!: HTMLElement;
  private tocItems: TocItem[] = [];
  private scale = 1;
  private dark = false;
  private annotations: Annotation[] = [];
  private findMatches: HTMLElement[][] = [];
  private findIndex = -1;
  private lastQuery = "";
  private cleanup: (() => void)[] = [];
  private frame = 0;

  constructor(
    private readonly kind: "md" | "txt",
    private readonly events: RendererEvents,
  ) {}

  async open(container: HTMLElement, url: string, initial: Locator | null) {
    const res = await fetch(url);
    if (!res.ok) throw new Error(`The file could not be read (${res.status}).`);
    const text = await res.text();

    this.scroller = document.createElement("div");
    this.scroller.className = "lb-doc-scroller";
    this.article = document.createElement("article");
    this.article.className = `lb-doc lb-doc-${this.kind}`;
    if (this.kind === "md") {
      this.article.innerHTML = renderMarkdown(text);
    } else {
      this.article.textContent = text;
    }
    this.scroller.append(this.article);
    container.append(this.scroller);
    if (this.kind === "md") void renderMermaid(this.article, this.dark);

    this.tocItems = Array.from(this.article.querySelectorAll<HTMLElement>("h1, h2, h3")).reduce<
      TocItem[]
    >((items, h) => {
      const item = { label: h.textContent ?? "", target: `#${h.id}`, children: [] };
      const last = items[items.length - 1];
      if (h.tagName !== "H1" && last && items.length) last.children.push(item);
      else items.push(item);
      return items;
    }, []);

    const onScroll = () => {
      cancelAnimationFrame(this.frame);
      this.frame = requestAnimationFrame(() => this.emitLocation());
    };
    this.scroller.addEventListener("scroll", onScroll, { passive: true });

    const onSelection = () => {
      const sel = document.getSelection();
      if (!sel || sel.isCollapsed || !sel.rangeCount) return this.events.selection(null);
      const range = sel.getRangeAt(0);
      if (!this.article.contains(range.commonAncestorContainer)) return;
      const all = this.article.textContent ?? "";
      const start = offsetOf(this.article, range.startContainer, range.startOffset);
      const end = offsetOf(this.article, range.endContainer, range.endOffset);
      if (end <= start || !all.slice(start, end).trim()) return this.events.selection(null);
      this.events.selection({
        quote: makeQuote(all, start, end),
        locator: { type: "text", start, end },
        label: this.sectionAt(range.getBoundingClientRect().top) ?? "",
        position: all.length ? start / all.length : 0,
        rect: range.getBoundingClientRect(),
      });
    };
    const onUp = () => setTimeout(onSelection, 0);
    this.article.addEventListener("pointerup", onUp);
    this.article.addEventListener("keyup", onUp);

    const onClick = (e: MouseEvent) => {
      const target = e.target as HTMLElement;
      const mark = target.closest<HTMLElement>("mark[data-annotation]");
      if (mark && document.getSelection()?.isCollapsed) {
        this.events.annotationClick(mark.dataset.annotation!, mark.getBoundingClientRect());
        return;
      }
      const a = target.closest<HTMLAnchorElement>("a[href]");
      if (!a) return;
      e.preventDefault();
      const href = a.getAttribute("href") ?? "";
      if (href.startsWith("#")) void this.goTo(href);
      else this.events.externalLink?.(href);
    };
    this.article.addEventListener("click", onClick);

    this.cleanup.push(() => {
      this.scroller.removeEventListener("scroll", onScroll);
      this.article.removeEventListener("pointerup", onUp);
      this.article.removeEventListener("keyup", onUp);
      this.article.removeEventListener("click", onClick);
    });

    if (initial) await this.goTo(initial);
    this.emitLocation();
  }

  destroy() {
    cancelAnimationFrame(this.frame);
    for (const f of this.cleanup) f();
    this.scroller?.remove();
  }

  toc() {
    return this.tocItems;
  }

  private sectionAt(y: number): string | undefined {
    let label: string | undefined;
    for (const h of this.article.querySelectorAll<HTMLElement>("h1, h2, h3")) {
      if (h.getBoundingClientRect().top <= y + 4) label = h.textContent ?? undefined;
      else break;
    }
    return label;
  }

  private emitLocation() {
    const max = this.scroller.scrollHeight - this.scroller.clientHeight;
    const fraction = max > 0 ? this.scroller.scrollTop / max : 0;
    const top = this.scroller.getBoundingClientRect().top;
    const section = this.sectionAt(top + 40);
    const pct = Math.round(fraction * 100);
    this.events.relocate({
      locator: { type: "scroll", fraction },
      progress: fraction,
      label: section ?? "",
      shortLabel: section ?? `${pct}%`,
      section,
    });
  }

  async goTo(target: string | Locator) {
    if (typeof target === "string") {
      if (target.startsWith("#")) {
        const el = this.article.querySelector<HTMLElement>(`[id="${CSS.escape(target.slice(1))}"]`);
        el?.scrollIntoView({ block: "start" });
      }
      return;
    }
    if (target.type === "scroll") {
      // Wait for layout so scrollHeight is right.
      await new Promise((r) => requestAnimationFrame(() => r(null)));
      const max = this.scroller.scrollHeight - this.scroller.clientHeight;
      this.scroller.scrollTop = target.fraction * max;
    } else if (target.type === "text") {
      const marks = wrapRange(this.article, target.start, (target.end ?? target.start) + 1, () =>
        document.createElement("span"),
      );
      marks[0]?.scrollIntoView({ block: "center" });
      unwrap(marks);
    }
  }

  next() {
    this.scroller.scrollBy({ top: this.scroller.clientHeight * 0.9, behavior: "smooth" });
  }

  prev() {
    this.scroller.scrollBy({ top: -this.scroller.clientHeight * 0.9, behavior: "smooth" });
  }

  scrollBy(direction: 1 | -1) {
    this.scroller.scrollBy({ top: direction * 60, behavior: "smooth" });
  }

  async start() {
    await this.goTo({ type: "scroll", fraction: 0 });
  }

  async end() {
    await this.goTo({ type: "scroll", fraction: 1 });
  }

  setLineHeight(lineHeight: number) {
    this.scroller.style.setProperty("--page-line-height", String(lineHeight));
  }

  /** Where an annotation's text is now: its offsets, or its quote. */
  private resolve(a: Annotation): [number, number] | null {
    const all = this.article.textContent ?? "";
    let loc: Locator | null = null;
    try {
      loc = JSON.parse(a.locator) as Locator;
    } catch {
      /* use the quote */
    }
    if (
      loc?.type === "text" &&
      loc.end !== undefined &&
      a.quote &&
      all.slice(loc.start, loc.end) === a.quote.exact
    ) {
      return [loc.start, loc.end];
    }
    if (!a.quote) return null;
    return findQuote(all, a.quote, loc?.type === "text" ? loc.start : 0);
  }

  setAnnotations(list: Annotation[]) {
    this.annotations = list;
    this.clearFind();
    unwrap(this.article.querySelectorAll("mark[data-annotation]"));
    for (const a of list) {
      if (a.kind !== "highlight") continue;
      const r = this.resolve(a);
      if (!r) continue;
      wrapRange(this.article, r[0], r[1], () => {
        const m = document.createElement("mark");
        m.dataset.annotation = a.id;
        m.className = `lb-hl${a.note ? " lb-hl-note" : ""}`;
        m.style.background = highlightFill(a.color, this.dark);
        return m;
      });
    }
  }

  async showAnnotation(a: Annotation) {
    const mark = this.article.querySelector<HTMLElement>(
      `mark[data-annotation="${CSS.escape(a.id)}"]`,
    );
    if (mark) mark.scrollIntoView({ block: "center" });
    else {
      const loc = JSON.parse(a.locator) as Locator;
      await this.goTo(loc);
    }
  }

  async find(query: string, backwards = false): Promise<FindResult> {
    if (query !== this.lastQuery) {
      this.clearFind();
      this.lastQuery = query;
      const q = query.trim().toLowerCase();
      if (q) {
        const all = (this.article.textContent ?? "").toLowerCase();
        const starts: number[] = [];
        for (
          let i = all.indexOf(q);
          i !== -1 && starts.length < 2000;
          i = all.indexOf(q, i + q.length)
        ) {
          starts.push(i);
        }
        // Wrap from the end so earlier offsets stay valid.
        this.findMatches = starts
          .reverse()
          .map((s) =>
            wrapRange(this.article, s, s + q.length, () => {
              const m = document.createElement("mark");
              m.className = "lb-find";
              return m;
            }),
          )
          .reverse();
      }
      this.findIndex = -1;
    }
    const total = this.findMatches.length;
    if (!total) return { current: 0, total: 0 };
    this.findMatches[this.findIndex]?.forEach((m) => m.classList.remove("lb-find-current"));
    this.findIndex = backwards
      ? (this.findIndex - 1 + total) % total
      : (this.findIndex + 1) % total;
    const current = this.findMatches[this.findIndex] ?? [];
    current.forEach((m) => m.classList.add("lb-find-current"));
    current[0]?.scrollIntoView({ block: "center" });
    return { current: this.findIndex + 1, total };
  }

  clearFind() {
    unwrap(this.article?.querySelectorAll("mark.lb-find") ?? []);
    this.findMatches = [];
    this.findIndex = -1;
    this.lastQuery = "";
  }

  setTheme(theme: PageTheme, _mode: PdfDarkMode) {
    this.dark = theme.dark;
    const s = this.scroller.style;
    s.setProperty("--page-bg", theme.bg);
    s.setProperty("--page-fg", theme.fg);
    s.setProperty("--page-link", theme.link);
    this.scroller.classList.toggle("lb-dark", theme.dark);
    // Recolour existing highlights for the new background.
    for (const m of this.article.querySelectorAll<HTMLElement>("mark[data-annotation]")) {
      const a = this.annotations.find((x) => x.id === m.dataset.annotation);
      m.style.background = highlightFill(a?.color ?? null, theme.dark);
    }
  }

  setZoom(zoom: ZoomValue) {
    this.scale = typeof zoom === "number" ? Math.min(2.2, Math.max(0.6, zoom)) : 1;
    this.scroller.style.setProperty("--page-font-size", `${BASE_FONT * this.scale}px`);
  }

  zoom(): ZoomValue {
    return this.scale;
  }

  clearSelection() {
    document.getSelection()?.removeAllRanges();
  }
}
