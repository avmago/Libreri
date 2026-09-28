/**
 * Splitting text into what read aloud speaks in one go: sentences, with
 * long ones cut at commas so every voice keeps up.
 */

export interface Span {
  start: number;
  end: number;
}

const MAX = 320;

/** Sentence spans of `text` (whitespace trimmed, empty ones left out). */
export function sentences(text: string, lang = "en"): Span[] {
  const raw: Span[] = [];
  const Seg = (Intl as unknown as { Segmenter?: typeof Intl.Segmenter }).Segmenter;
  if (Seg) {
    let seg: Intl.Segmenter;
    try {
      seg = new Seg(lang, { granularity: "sentence" });
    } catch {
      seg = new Seg("en", { granularity: "sentence" });
    }
    for (const s of seg.segment(text))
      raw.push({ start: s.index, end: s.index + s.segment.length });
  } else {
    const re = /[^.!?…]+(?:[.!?…]+["'”’)\]]*|$)\s*/g;
    for (let m = re.exec(text); m && m[0]; m = re.exec(text))
      raw.push({ start: m.index, end: m.index + m[0].length });
  }
  const out: Span[] = [];
  for (const s of raw) {
    let { start, end } = s;
    while (start < end && /\s/.test(text[start]!)) start++;
    while (end > start && /\s/.test(text[end - 1]!)) end--;
    if (end <= start) continue;
    // "Dr." or a lone initial: join it to the next sentence.
    const prev = out[out.length - 1];
    if (prev && /(?:^|\s)(?:[A-Z][a-z]{0,3}|[A-Z])\.$/.test(text.slice(prev.start, prev.end))) {
      prev.end = end;
      continue;
    }
    // Stray punctuation or a number on its own goes with the one before.
    if (prev && !/[\p{L}\p{N}]{2}/u.test(text.slice(start, end))) {
      prev.end = end;
      continue;
    }
    out.push({ start, end });
  }
  return out.flatMap((s) => (s.end - s.start > MAX ? cutLong(text, s) : [s]));
}

/** Cuts a long sentence at commas, semicolons or spaces. */
function cutLong(text: string, s: Span): Span[] {
  const out: Span[] = [];
  let start = s.start;
  while (s.end - start > MAX) {
    const window = text.slice(start, start + MAX);
    let cut = Math.max(
      window.lastIndexOf(", "),
      window.lastIndexOf("; "),
      window.lastIndexOf(": "),
    );
    if (cut < MAX / 3) cut = window.lastIndexOf(" ");
    if (cut < MAX / 3) cut = MAX - 1;
    out.push({ start, end: start + cut + 1 });
    start += cut + 1;
    while (start < s.end && /\s/.test(text[start]!)) start++;
  }
  if (start < s.end) out.push({ start, end: s.end });
  return out;
}
