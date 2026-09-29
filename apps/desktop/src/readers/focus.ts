/**
 * ADHD reading (Settings › Reader): bionic reading for text Libreri lays
 * out, and finding the line under the pointer for the line highlight and
 * the reading mask.
 *
 * Bionic reading wraps the start of each word in `<lb-b>` and the rest in
 * `<lb-r>`. The text itself is unchanged, so character offsets still
 * match; EPUB positions are made as if the wrappers were not there (see
 * `bionicFilter` and `liftBoundary`).
 */

export interface BionicOptions {
  /** Share of each word made bold, 0.3–0.7. */
  fixation: number;
  /** Opacity of the rest of the word, 0.4–1. */
  fade: number;
}

const BOLD = "lb-b";
const REST = "lb-r";

/** Text never changed: code, maths, diagrams and form fields. */
const SKIP =
  "script, style, noscript, code, pre, kbd, samp, math, svg, textarea, input, select, rt, rp, .katex, .mermaid, lb-b, lb-r";

/** Words in scripts written with spaces between words and no joined
 * letters (Latin, Greek, Cyrillic, Armenian, Georgian). */
const WORD =
  /[\p{sc=Latin}\p{sc=Greek}\p{sc=Cyrillic}\p{sc=Armenian}\p{sc=Georgian}][\p{sc=Latin}\p{sc=Greek}\p{sc=Cyrillic}\p{sc=Armenian}\p{sc=Georgian}\p{M}'’]*/gu;

/** How many letters of a word of `length` letters are bold. */
export function boldLength(length: number, fixation: number): number {
  if (length <= 3) return 1;
  return Math.min(length, Math.max(1, Math.round(length * fixation)));
}

export function isBionic(node: Node | null): boolean {
  if (!node || node.nodeType !== Node.ELEMENT_NODE) return false;
  const name = (node as Element).localName;
  return name === BOLD || name === REST;
}

/** The pieces a text is split into: plain text, bold starts and rests. */
export function bionicPieces(
  text: string,
  fixation: number,
): { kind: "text" | "bold" | "rest"; text: string }[] {
  const out: { kind: "text" | "bold" | "rest"; text: string }[] = [];
  let last = 0;
  // matchAll starts where the last search stopped.
  WORD.lastIndex = 0;
  for (const m of text.matchAll(WORD)) {
    const word = m[0];
    const at = m.index ?? 0;
    if (at > last) out.push({ kind: "text", text: text.slice(last, at) });
    // Letters, not code units: keep accents and surrogate pairs whole.
    const letters = Array.from(word);
    const n = boldLength(letters.filter((c) => !/\p{M}/u.test(c)).length, fixation);
    let bold = 0;
    let seen = 0;
    while (bold < letters.length && seen < n) {
      if (!/\p{M}/u.test(letters[bold]!)) seen++;
      bold++;
    }
    // Accents that follow the last bold letter stay with it.
    while (bold < letters.length && /\p{M}/u.test(letters[bold]!)) bold++;
    out.push({ kind: "bold", text: letters.slice(0, bold).join("") });
    if (bold < letters.length) out.push({ kind: "rest", text: letters.slice(bold).join("") });
    last = at + word.length;
  }
  // Always end with plain text (it may be empty), so EPUB positions in a
  // run of text can be given from a text node that is not wrapped.
  out.push({ kind: "text", text: text.slice(last) });
  return out;
}

/** Makes the start of each word under `root` bold. */
export function applyBionic(root: Element, options: BionicOptions) {
  const doc = root.ownerDocument;
  (root as HTMLElement).style?.setProperty("--lb-bn-fade", String(options.fade));
  const walker = doc.createTreeWalker(root, NodeFilter.SHOW_TEXT | NodeFilter.SHOW_ELEMENT, {
    acceptNode(n) {
      if (n.nodeType === Node.ELEMENT_NODE)
        return (n as Element).matches(SKIP) ? NodeFilter.FILTER_REJECT : NodeFilter.FILTER_SKIP;
      return NodeFilter.FILTER_ACCEPT;
    },
  });
  const texts: Text[] = [];
  for (let n = walker.nextNode(); n; n = walker.nextNode()) texts.push(n as Text);
  for (const t of texts) {
    if (!t.data.trim()) continue;
    WORD.lastIndex = 0;
    if (!WORD.test(t.data)) continue;
    const pieces = bionicPieces(t.data, options.fixation);
    const frag = doc.createDocumentFragment();
    for (const p of pieces) {
      if (p.kind === "text") frag.append(doc.createTextNode(p.text));
      else {
        const el = doc.createElement(p.kind === "bold" ? BOLD : REST);
        el.textContent = p.text;
        frag.append(el);
      }
    }
    t.replaceWith(frag);
  }
}

/** Takes bionic reading off again. */
export function removeBionic(root: Element) {
  const parents = new Set<Node>();
  for (const el of Array.from(root.querySelectorAll(`${BOLD}, ${REST}`))) {
    const parent = el.parentNode;
    if (!parent) continue;
    parents.add(parent);
    el.replaceWith(...Array.from(el.childNodes));
  }
  for (const p of parents) p.normalize();
  (root as HTMLElement).style?.removeProperty("--lb-bn-fade");
}

/** CSS for bionic reading, for documents Libreri does not style itself. */
export const BIONIC_CSS = `${BOLD} { font-weight: bolder; } ${REST} { opacity: var(--lb-bn-fade, 1); }`;

/** For EPUB positions (foliate-js's `NodeFilter`): the wrappers are looked
 * through, so a position is the same with bionic reading on or off. */
export function bionicFilter(node: Node): number {
  return isBionic(node) ? NodeFilter.FILTER_SKIP : NodeFilter.FILTER_ACCEPT;
}

/**
 * foliate-js counts a place in a run of text from the text node's parent.
 * For text inside a wrapper, the same place is given from a text node of
 * the run that is not wrapped (the offset may then be negative or past
 * that node's end, which foliate-js adds up correctly).
 */
export function liftBoundary(node: Node, offset: number): { node: Node; offset: number } {
  const wrapper = node.parentNode;
  if (node.nodeType !== Node.TEXT_NODE || !isBionic(wrapper) || !wrapper?.parentNode)
    return { node, offset };
  const inRun = (n: Node | null) =>
    !!n && (n.nodeType === Node.TEXT_NODE || n.nodeType === Node.COMMENT_NODE || isBionic(n));
  let first: Node = wrapper;
  while (inRun(first.previousSibling)) first = first.previousSibling!;
  let pos = 0;
  let target = -1;
  let direct: { node: Node; at: number } | null = null;
  for (let n: Node | null = first; inRun(n); n = n!.nextSibling) {
    if (n!.nodeType === Node.TEXT_NODE) {
      direct ??= { node: n!, at: pos };
      pos += (n as Text).data.length;
    } else if (n!.nodeType === Node.ELEMENT_NODE) {
      for (const c of Array.from(n!.childNodes)) {
        if (c === node) target = pos;
        if (c.nodeType === Node.TEXT_NODE) pos += (c as Text).data.length;
      }
    }
    if (direct && target >= 0) break;
  }
  if (!direct || target < 0) return { node, offset };
  return { node: direct.node, offset: target + offset - direct.at };
}

/** A line of text on the screen. */
export interface LineBox {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

const BLOCK =
  "p, li, dt, dd, h1, h2, h3, h4, h5, h6, blockquote, pre, td, th, figcaption, caption, div, section, article, .textLayer";

/**
 * The line of text at (x, y) in `doc`'s window, or null where there is no
 * text. Across: the paragraph (or PDF page) the line is in.
 */
export function lineAt(doc: Document, x: number, y: number): LineBox | null {
  let node: Node | null = null;
  let offset = 0;
  const d = doc as Document & {
    caretPositionFromPoint?: (x: number, y: number) => { offsetNode: Node; offset: number } | null;
  };
  try {
    if (typeof doc.caretRangeFromPoint === "function") {
      const r = doc.caretRangeFromPoint(x, y);
      if (r) {
        node = r.startContainer;
        offset = r.startOffset;
      }
    } else if (typeof d.caretPositionFromPoint === "function") {
      const p = d.caretPositionFromPoint(x, y);
      if (p) {
        node = p.offsetNode;
        offset = p.offset;
      }
    }
  } catch {
    return null;
  }
  if (!node || node.nodeType !== Node.TEXT_NODE) return null;
  const text = node as Text;
  if (!text.data.trim()) return null;
  const range = doc.createRange();
  // The character after the caret, or the one before at the end of a node.
  const i = Math.max(0, Math.min(offset, text.data.length - 1));
  range.setStart(text, i);
  range.setEnd(text, i + 1);
  let rect: DOMRect | null = null;
  for (const r of Array.from(range.getClientRects())) {
    if (
      r.height > 0 &&
      (!rect || Math.abs(r.top + r.height / 2 - y) < Math.abs(rect.top + rect.height / 2 - y))
    )
      rect = r;
  }
  if (!rect) return null;
  // Past the text (margins, far between paragraphs): no line.
  if (y < rect.top - rect.height * 0.75 || y > rect.bottom + rect.height * 0.75) return null;
  const block = text.parentElement?.closest(BLOCK);
  let across: { left: number; right: number } = rect;
  if (block?.classList.contains("textLayer")) {
    // A PDF page: from the first to the last piece of text on this line.
    const mid = (rect.top + rect.bottom) / 2;
    let left = Infinity;
    let right = -Infinity;
    for (const span of Array.from(block.querySelectorAll("span"))) {
      if (span.firstElementChild) continue;
      const r = span.getBoundingClientRect();
      if (r.width && r.top <= mid && r.bottom >= mid) {
        left = Math.min(left, r.left);
        right = Math.max(right, r.right);
      }
    }
    if (left < right) across = { left, right };
  } else if (block) {
    // A paragraph split across columns has a box for each part.
    const parts = Array.from(block.getClientRects());
    across =
      parts.find((p) => x >= p.left && x <= p.right && y >= p.top && y <= p.bottom) ??
      block.getBoundingClientRect();
  }
  return { top: rect.top, bottom: rect.bottom, left: across.left, right: across.right };
}
