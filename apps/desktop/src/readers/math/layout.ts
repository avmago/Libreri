/**
 * Maths selected in a PDF, rebuilt as LaTeX from where each character sits
 * on the page (Phase 8b). A PDF's text is only characters in reading
 * order, so "∑ n=1 ∞" or "x2n" say nothing of limits or scripts; their
 * positions and sizes do. Smaller characters raised after a base are a
 * superscript, lowered ones a subscript, characters stacked over and
 * under a large operator are its limits, and two stacked rows with
 * nothing on the main line between them are a fraction.
 */
import { textToLatex } from "./latex";

/** A character and its box on the screen. */
export interface Glyph {
  ch: string;
  x: number;
  y: number;
  w: number;
  h: number;
  /** Followed by a variation selector: TeX's large sizes of brackets and
   * operators ("(︂", "∑︁"). */
  sized?: boolean;
}

/** The characters of a selection with their boxes. */
export function glyphsOf(range: Range, limit = 2000): Glyph[] {
  const out: Glyph[] = [];
  const root = range.commonAncestorContainer;
  const doc = root.ownerDocument ?? document;
  const walker = doc.createTreeWalker(
    root.nodeType === Node.TEXT_NODE ? (root.parentNode ?? root) : root,
    NodeFilter.SHOW_TEXT,
  );
  const one = doc.createRange();
  for (let node = walker.nextNode(); node && out.length < limit; node = walker.nextNode()) {
    if (!range.intersectsNode(node)) continue;
    const text = node.textContent ?? "";
    const start = node === range.startContainer ? range.startOffset : 0;
    const end = node === range.endContainer ? range.endOffset : text.length;
    let i = start;
    while (i < end && out.length < limit) {
      const cp = text.codePointAt(i) ?? 0;
      const len = cp > 0xffff ? 2 : 1;
      const ch = text.slice(i, i + len);
      // Variation selectors and zero-width characters have no box of
      // their own (TeX writes "∑︁").
      if (/^[\uFE00-\uFE0F]$/.test(ch)) {
        const last = out[out.length - 1];
        if (last) last.sized = true;
      } else if (ch.trim() && !/^[\u200B-\u200D\u2060-\u2064]$/.test(ch)) {
        one.setStart(node, i);
        one.setEnd(node, i + len);
        const r = one.getBoundingClientRect();
        if (r.width > 0 && r.height > 0)
          out.push({ ch, x: r.left, y: r.top, w: r.width, h: r.height });
      }
      i += len;
    }
  }
  return out;
}

/** Where a character sits: its box's bottom less the room fonts leave
 * below the baseline. Comparing baselines, not box centres, keeps a small
 * digit on the line level with a large letter. */
const base = (g: Glyph) => g.y + g.h * 0.78;

const BIG = new Set([..."∑∏∐∫∬∭∮⋃⋂⨁⨂⨀⋁⋀"]);
const isBig = (g: Glyph) => BIG.has(g.ch);
/** Brackets: TeX draws tall ones from their own font, with boxes off the
 * line, so full-size ones are not used to find the line. */
const isFence = (g: Glyph) => "()[]{}⟨⟩⌊⌋⌈⌉".includes(g.ch);
const CLOSE: Record<string, string> = {
  "(": ")",
  "[": "]",
  "{": "}",
  "⟨": "⟩",
  "⌊": "⌋",
  "⌈": "⌉",
};
/** Brackets found around a stack (tall ones), which are on the line
 * wherever their boxes are. */
const tallFences = new WeakSet<Glyph>();
const isCore = (g: Glyph) => /[\p{L}\p{N}]/u.test(g.ch) && !isBig(g);

function median(v: number[]): number {
  const s = [...v].sort((a, b) => a - b);
  const m = s.length >> 1;
  return s.length % 2 ? s[m]! : (s[m - 1]! + s[m]!) / 2;
}

/** Runs of characters side by side (a gap of more than 0.3 of the main
 * size splits them; a large operator is always a run of its own). */
function runs(gs: Glyph[], H: number): Glyph[][] {
  const out: Glyph[][] = [];
  let end = -Infinity;
  for (const g of [...gs].sort((a, b) => a.x - b.x)) {
    const last = out[out.length - 1];
    const prev = last?.[last.length - 1];
    const joins =
      last &&
      prev &&
      !isBig(g) &&
      !isBig(prev) &&
      g.x - end < 0.25 * H &&
      Math.abs(base(g) - base(prev)) < 0.2 * H;
    if (joins) last.push(g);
    else {
      out.push([g]);
      end = -Infinity;
    }
    end = Math.max(end, g.x + g.w);
  }
  return out;
}

const extent = (gs: Glyph[]) => ({
  x0: Math.min(...gs.map((g) => g.x)),
  x1: Math.max(...gs.map((g) => g.x + g.w)),
});

function overlap(a: { x0: number; x1: number }, b: { x0: number; x1: number }): boolean {
  const o = Math.min(a.x1, b.x1) - Math.max(a.x0, b.x0);
  return o > 0.25 * Math.min(a.x1 - a.x0, b.x1 - b.x0);
}

/** The main size and the height of the main line (its baseline). */
function measure(gs: Glyph[]): { H: number; axis: number } {
  const pool = gs.filter(isCore).length ? gs.filter(isCore) : gs;
  const maxH = Math.max(...pool.map((g) => g.h));
  const H = median(pool.filter((g) => g.h >= 0.8 * maxH).map((g) => g.h));
  // Large operators are left out: their boxes sit off the line.
  const full = gs.filter((g) => g.h >= 0.8 * maxH && !isBig(g) && !isFence(g));
  const main = full.length ? full : gs;
  const centre = base;
  // Rows of full-size characters.
  const rows: Glyph[][] = [];
  for (const g of [...main].sort((a, b) => centre(a) - centre(b))) {
    const last = rows[rows.length - 1];
    if (last && Math.abs(centre(g) - centre(last[last.length - 1]!)) < 0.3 * H) last.push(g);
    else rows.push([g]);
  }
  if (rows.length === 1) return { H, axis: median(rows[0]!.map(centre)) };
  // The main line is the row that nothing is stacked over or under (a
  // fraction's rows overlap each other); the longest if there are several.
  const pieces = rows.map((row, i) => runs(row, H).map((r) => ({ row: i, r, ...extent(r) })));
  const all = pieces.flat();
  let best: { row: number; free: number } | null = null;
  pieces.forEach((list, i) => {
    const stacked = list.some((p) => all.some((o) => o.row !== i && overlap(p, o)));
    const free = rows[i]!.length;
    if (!stacked && (!best || free > best.free)) best = { row: i, free };
  });
  if (best) return { H, axis: median(rows[(best as { row: number }).row]!.map(centre)) };
  // Nothing on a main line (a fraction alone): between the rows.
  const top = Math.min(...gs.map((g) => g.y));
  const bottom = Math.max(...gs.map((g) => g.y + g.h));
  return { H, axis: (top + bottom) / 2 };
}

type Place = "line" | "up" | "down";

function place(g: Glyph, H: number, axis: number): Place {
  if (isBig(g)) return "line";
  const d = (base(g) - axis) / H;
  // TeX hangs a root sign from its top, so its box sits high.
  if (g.ch === "√" && d > -0.9 && d < 0.3) return "line";
  if (tallFences.has(g)) return "line";
  const small = g.h < 0.85 * H;
  if (d < (small ? -0.12 : -0.3)) return "up";
  if (d > (small ? 0.1 : 0.3)) return "down";
  return "line";
}

interface Block {
  glyphs: Glyph[];
  x0: number;
  x1: number;
}

/** Runs on the main line, raised or lowered, merged where they overlap
 * across (a stack: a fraction, limits, a subscript under a superscript). */
function blocks(gs: Glyph[], H: number, axis: number): Block[] {
  const byPlace = new Map<Place, Glyph[]>();
  for (const g of gs) {
    const p = place(g, H, axis);
    byPlace.set(p, [...(byPlace.get(p) ?? []), g]);
  }
  const pieces = [...byPlace.values()]
    .flatMap((list) => runs(list, H))
    .map((r) => ({ glyphs: r, ...extent(r) }))
    .sort((a, b) => a.x0 - b.x0);
  const out: Block[] = [];
  for (const p of pieces) {
    const b = out[out.length - 1];
    if (b && overlap(b, p)) {
      b.glyphs.push(...p.glyphs);
      b.x1 = Math.max(b.x1, p.x1);
    } else out.push({ ...p, glyphs: [...p.glyphs] });
  }
  // A fraction's rows are rarely the same width: the parts of the wider
  // row beside the stack (full size, off the main line) belong to it.
  const where = (g: Glyph) => place(g, H, axis);
  const isFraction = (b: Block) =>
    !b.glyphs.some((g) => where(g) === "line") &&
    b.glyphs.some((g) => where(g) === "up" && g.h >= 0.85 * H) &&
    b.glyphs.some((g) => where(g) === "down" && g.h >= 0.85 * H);
  const joins = (b: Block) =>
    !b.glyphs.some((g) => where(g) === "line") && b.glyphs.some((g) => g.h >= 0.85 * H);
  for (let i = 0; i < out.length; i++) {
    const f = out[i]!;
    if (!isFraction(f)) continue;
    while (i > 0 && joins(out[i - 1]!) && f.x0 - out[i - 1]!.x1 < 0.6 * H) {
      const b = out.splice(--i, 1)[0]!;
      f.glyphs.push(...b.glyphs);
      f.x0 = b.x0;
    }
    while (out[i + 1] && joins(out[i + 1]!) && out[i + 1]!.x0 - f.x1 < 0.6 * H) {
      const b = out.splice(i + 1, 1)[0]!;
      f.glyphs.push(...b.glyphs);
      f.x1 = Math.max(f.x1, b.x1);
    }
  }
  return out;
}

/** Mathematical italic, bold and other styled letters (U+1D400…). */
const isItalic = (ch: string) => (ch.codePointAt(0) ?? 0) >= 0x1d400;

const ACCENTS: Record<string, string> = {
  ˆ: "\\hat",
  "^": "\\hat",
  "˜": "\\tilde",
  "~": "\\tilde",
  "¯": "\\bar",
  "˙": "\\dot",
  "¨": "\\ddot",
  "→": "\\vec",
  "⃗": "\\vec",
};

const RELATIONS = new Set([..."=<>≤≥≠≈≡∼≅→⇒⇔"]);

const brace = (s: string) => ([...s].length === 1 ? s : `{${s}}`);

/** Characters of the main line as text, with a space where there is a
 * gap (so "sin x" stays two words). */
function lineText(gs: Glyph[], H: number): string {
  const s = [...gs].sort((a, b) => a.x - b.x);
  let out = "";
  s.forEach((g, i) => {
    const prev = s[i - 1];
    const letters = prev && /\p{L}/u.test(prev.ch) && /\p{L}/u.test(g.ch);
    // Upright letters (a function name) next to italic ones (a variable).
    const styleChange = letters && isItalic(prev.ch) !== isItalic(g.ch);
    if (letters && (styleChange || g.x - (prev.x + prev.w) > 0.15 * H)) out += " ";
    out += g.ch;
  });
  return out;
}

function scripts(up: string, down: string): string {
  let s = "";
  if (down) s += `_${brace(down)}`;
  if (up) s += /^'+$/.test(up) ? up : `^${brace(up)}`;
  return s;
}

function build(input: Glyph[], depth = 0): string {
  const gs = input.filter((g) => g.ch.trim() && g.w > 0 && g.h > 0);
  if (!gs.length) return "";
  if (depth > 6) return textToLatex(lineText(gs, gs[0]!.h));
  const { H, axis } = measure(gs);
  // Tall brackets: a pair with a stack between them (above and below the
  // line). One stack alone between round ones is a binomial.
  const sorted = [...gs].sort((a, b) => a.x - b.x);
  for (const open of sorted) {
    const want = CLOSE[open.ch];
    const tall = (g: Glyph) => g.sized || g.h >= 1.3 * H;
    if (!want || !tall(open) || tallFences.has(open)) continue;
    const close = sorted.find(
      (g) => g.ch === want && tall(g) && g.x > open.x && Math.abs(base(g) - base(open)) < 0.1 * H,
    );
    if (!close) continue;
    const inner = gs.filter(
      (g) => g.x >= open.x + open.w * 0.5 && g.x + g.w <= close.x + close.w * 0.5,
    );
    const places = inner.map((g) => place(g, H, axis));
    if (!places.includes("up") || !places.includes("down")) continue;
    tallFences.add(open);
    tallFences.add(close);
    if (open.ch === "(" && !places.includes("line")) {
      const up = inner.filter((_, i) => places[i] === "up");
      const down = inner.filter((_, i) => places[i] === "down");
      const head = build(
        gs.filter((g) => g.x < open.x),
        depth + 1,
      );
      const tail = build(
        gs.filter((g) => g.x > close.x),
        depth + 1,
      );
      return `${head}\\binom{${build(up, depth + 1)}}{${build(down, depth + 1)}}${tail}`;
    }
  }
  // A root: its bar is a drawn line, not text, so what follows it (to the
  // next relation sign) is taken as what is under it.
  const root = gs.find((g) => g.ch === "√" && place(g, H, axis) === "line");
  if (root) {
    const after = gs.filter((g) => g !== root && g.x >= root.x + root.w * 0.5);
    const rel = after
      .filter((g) => RELATIONS.has(g.ch) && place(g, H, axis) === "line")
      .sort((a, b) => a.x - b.x)[0];
    const inside = after.filter((g) => !rel || g.x < rel.x);
    const before = gs.filter((g) => g !== root && !after.includes(g));
    const rest = after.filter((g) => !inside.includes(g));
    if (inside.length) {
      const head = build(before, depth + 1);
      const tail = build(rest, depth + 1);
      const sqrt = `\\sqrt{${build(inside, depth + 1)}}`;
      return `${head}${/\\[a-zA-Z]+$/.test(head) ? " " : ""}${sqrt}${tail}`;
    }
  }
  const parts: string[] = [];
  let run: Glyph[] = [];
  let pending: { up: Glyph[]; down: Glyph[] } | null = null;
  let lastEnd = -Infinity;
  const flushRun = () => {
    if (run.length) parts.push(textToLatex(lineText(run, H)));
    run = [];
  };
  const flushScripts = () => {
    if (!pending) return;
    const s = scripts(build(pending.up, depth + 1), build(pending.down, depth + 1));
    if (!parts.length) parts.push("{}");
    parts[parts.length - 1] += s;
    pending = null;
  };
  for (const b of blocks(gs, H, axis)) {
    const where = { line: [] as Glyph[], up: [] as Glyph[], down: [] as Glyph[] };
    for (const g of b.glyphs) where[place(g, H, axis)].push(g);
    const onLine = where.line.length > 0;
    const stacked = where.up.length > 0 && where.down.length > 0;
    const allSmall = b.glyphs.every((g) => g.h < 0.85 * H);
    const touching = b.x0 - lastEnd < 0.45 * H;
    if (onLine && !where.up.length && !where.down.length) {
      flushScripts();
      run.push(...where.line);
    } else if (onLine) {
      // A base with limits or scripts stacked on it (∑, lim, a letter):
      // the base is what lies under or over them.
      flushScripts();
      const over = extent([...where.up, ...where.down]);
      const base = where.line.filter((g) => overlap(over, { x0: g.x, x1: g.x + g.w }));
      const mid = (over.x0 + over.x1) / 2;
      const b0 = Math.min(...base.map((g) => g.x));
      const b1 = Math.max(...base.map((g) => g.x + g.w));
      const centred = base.length > 0 && mid > b0 && mid < b1;
      const operator = base.some(isBig) || base.every((g) => /^[a-z]$/.test(g.ch));
      const accent = where.up.length === 1 && !where.down.length && ACCENTS[where.up[0]!.ch];
      if (centred && accent) {
        // An accent over a letter: x̂, x̄.
        run.push(...where.line.filter((g) => !base.includes(g) && g.x < over.x0));
        flushRun();
        parts.push(`${accent}{${build(base, depth + 1)}}`);
        run.push(...where.line.filter((g) => !base.includes(g) && g.x >= over.x0));
        lastEnd = b.x1;
        continue;
      }
      if (centred && !operator && (where.up.length === 0) !== (where.down.length === 0)) {
        // Only one side stacked over a line character that is not an
        // operator: a fraction whose other part is smaller (a/b over c).
        flushRun();
        const top = where.up.length ? build(where.up, depth + 1) : build(where.line, depth + 1);
        const bottom = where.up.length
          ? build(where.line, depth + 1)
          : build(where.down, depth + 1);
        parts.push(`\\frac{${top}}{${bottom}}`);
        lastEnd = b.x1;
        continue;
      }
      const before = where.line.filter((g) => !base.includes(g) && g.x < over.x0);
      const after = where.line.filter((g) => !base.includes(g) && g.x >= over.x0);
      run.push(...before);
      flushRun();
      parts.push(build(base.length ? base : where.line, depth + 1).trimEnd());
      parts[parts.length - 1] += scripts(build(where.up, depth + 1), build(where.down, depth + 1));
      run.push(...(base.length ? after : []));
    } else if (stacked && !(allSmall && touching)) {
      // Two rows with nothing between them on the main line: a fraction.
      flushScripts();
      flushRun();
      parts.push(`\\frac{${build(where.up, depth + 1)}}{${build(where.down, depth + 1)}}`);
    } else {
      // Raised or lowered after a base: scripts, gathered across blocks.
      flushRun();
      pending ??= { up: [], down: [] };
      pending.up.push(...where.up);
      pending.down.push(...where.down);
    }
    lastEnd = b.x1;
  }
  flushRun();
  flushScripts();
  return parts
    .map((p, i) =>
      // Keep a command from running into the next letter.
      i < parts.length - 1 && /\\[a-zA-Z]+$/.test(p) && /^[a-zA-Z]/.test(parts[i + 1]!)
        ? `${p} `
        : p,
    )
    .join("");
}

/** LaTeX for characters laid out on a page. */
export function layoutToLatex(glyphs: Glyph[]): string {
  return build(glyphs).trim();
}
