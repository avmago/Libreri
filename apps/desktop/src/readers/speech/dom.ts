/**
 * Read aloud over HTML (Markdown and text books, EPUB sections): blocks in
 * reading order, split into sentences, each with a DOM range to mark.
 * Maths is read from its MathML.
 */
import { mathmlToSpeech, texToSpeech } from "./math";
import { sentences } from "./sentences";
import type { SpeechPiece, SpeechSource } from "./types";

const BLOCK =
  "p, li, dt, dd, h1, h2, h3, h4, h5, h6, blockquote, pre, td, th, figcaption, caption, div, section, article, aside, header, footer, body";

/** Elements read as one unit (maths), and elements never read. */
const UNIT = ".katex, math";
const SKIP =
  ".katex-mathml, script, style, noscript, rt, rp, sup.footnote-ref, [aria-hidden='true']";

interface Part {
  /** A text node, or a maths element read as a whole. */
  node: Node;
  from: number;
  to: number;
}

export interface DomPiece extends SpeechPiece {
  range: Range;
}

function spokenMath(el: Element): string {
  const math = el.localName === "math" ? el : el.querySelector("math");
  if (math) {
    const said = mathmlToSpeech(math);
    if (said) return said;
  }
  const tex = el.querySelector("annotation[encoding='application/x-tex']")?.textContent;
  return tex ? texToSpeech(tex) : (el.textContent ?? "");
}

/** The blocks under `root`, each with its text and where the text came from. */
function* blocks(
  root: Element,
  from?: Node,
): Generator<{ text: string; parts: Part[]; lang: string }> {
  const doc = root.ownerDocument;
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT, {
    acceptNode(n) {
      if (n.nodeType === Node.ELEMENT_NODE) {
        const el = n as Element;
        if (el.matches(SKIP)) return NodeFilter.FILTER_REJECT;
        return NodeFilter.FILTER_ACCEPT;
      }
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  let started = !from;
  let block: Element | null = null;
  let text = "";
  let parts: Part[] = [];
  const flush = function* () {
    if (text.trim())
      yield {
        text,
        parts,
        lang: block?.closest("[lang]")?.getAttribute("lang") ?? doc.documentElement.lang ?? "",
      };
    text = "";
    parts = [];
  };
  for (let n = walker.nextNode(); n; n = walker.nextNode()) {
    if (!started) {
      if (
        n === from ||
        (from && n.compareDocumentPosition(from) & Node.DOCUMENT_POSITION_PRECEDING)
      )
        started = true;
      else continue;
    }
    if (n.nodeType === Node.ELEMENT_NODE) {
      const el = n as Element;
      if (el.matches(UNIT)) {
        const b = el.parentElement?.closest(BLOCK) ?? null;
        if (b !== block) {
          yield* flush();
          block = b;
        }
        const said = ` ${spokenMath(el)} `;
        parts.push({ node: el, from: text.length, to: text.length + said.length });
        text += said;
        // Do not read inside it again.
        let last: Node = el;
        while (last.lastChild) last = last.lastChild;
        walker.currentNode = last;
      }
      continue;
    }
    const t = n as Text;
    if (!t.data) continue;
    const b = t.parentElement?.closest(BLOCK) ?? null;
    if (b !== block) {
      yield* flush();
      block = b;
    }
    parts.push({ node: t, from: text.length, to: text.length + t.data.length });
    text += t.data;
  }
  yield* flush();
}

/** A range covering characters [start, end) of a block's text. */
function rangeFor(doc: Document, parts: Part[], start: number, end: number): Range | null {
  const range = doc.createRange();
  const first = parts.find((p) => p.to > start);
  const last = [...parts].reverse().find((p) => p.from < end);
  if (!first || !last) return null;
  if (first.node.nodeType === Node.TEXT_NODE)
    range.setStart(first.node, Math.max(0, start - first.from));
  else range.setStartBefore(first.node);
  if (last.node.nodeType === Node.TEXT_NODE)
    range.setEnd(last.node, Math.min((last.node as Text).data.length, end - last.from));
  else range.setEndAfter(last.node);
  return range;
}

const HL = "lb-tts";

/** Marks a range with the CSS Custom Highlight API (where the web view has it). */
export function markRange(range: Range | null, doc: Document) {
  const win = doc.defaultView as (Window & typeof globalThis) | null;
  const api = (win as unknown as { CSS?: { highlights?: Map<string, unknown> } })?.CSS?.highlights;
  const Highlight = (win as unknown as { Highlight?: new (...r: Range[]) => unknown })?.Highlight;
  if (!api || !Highlight) return false;
  if (!doc.getElementById("lb-tts-style")) {
    const style = doc.createElement("style");
    style.id = "lb-tts-style";
    style.textContent = `::highlight(${HL}) { background-color: rgba(250, 204, 21, 0.45); color: inherit; }`;
    (doc.head ?? doc.documentElement).append(style);
  }
  if (range) api.set(HL, new Highlight(range));
  else api.delete(HL);
  return true;
}

/**
 * Sentences of HTML documents one after another. `docs` gives the next
 * document when one is read (EPUB sections), or null at the end.
 */
export class DomSpeech implements SpeechSource {
  private iter: Generator<{ text: string; parts: Part[]; lang: string }> | null = null;
  private queue: DomPiece[] = [];
  private doc: Document | null = null;

  constructor(
    private readonly start: { root: Element; from?: Node },
    private readonly reveal: (range: Range) => void,
    private readonly nextRoot: () => Promise<Element | null> = async () => null,
    private readonly fallbackMark?: (range: Range | null) => void,
  ) {}

  async next(): Promise<SpeechPiece | null> {
    while (!this.queue.length) {
      if (!this.iter) {
        this.doc = this.start.root.ownerDocument;
        this.iter = blocks(this.start.root, this.start.from);
      }
      const b = this.iter.next();
      if (b.done) {
        const root = await this.nextRoot();
        if (!root) return null;
        this.doc = root.ownerDocument;
        this.iter = blocks(root);
        continue;
      }
      this.fill(b.value);
    }
    return this.queue.shift()!;
  }

  peek(): SpeechPiece | null {
    while (!this.queue.length && this.iter) {
      const b = this.iter.next();
      if (b.done) {
        // Put back an iterator that is done; next() turns the chapter.
        this.iter = (function* () {})();
        return null;
      }
      this.fill(b.value);
    }
    return this.queue[0] ?? null;
  }

  private fill({ text, parts, lang }: { text: string; parts: Part[]; lang: string }) {
    for (const s of sentences(text, lang || undefined)) {
      const range = rangeFor(this.doc!, parts, s.start, s.end);
      const said = text.slice(s.start, s.end).replace(/\s+/g, " ").trim();
      if (range && said) this.queue.push({ text: said, lang: lang || undefined, range });
    }
  }

  show(piece: SpeechPiece, follow: boolean) {
    const p = piece as DomPiece;
    const doc = p.range.startContainer.ownerDocument ?? document;
    if (!markRange(p.range, doc)) this.fallbackMark?.(p.range);
    if (follow) this.reveal(p.range);
  }

  clear() {
    if (this.doc && !markRange(null, this.doc)) this.fallbackMark?.(null);
  }
}
