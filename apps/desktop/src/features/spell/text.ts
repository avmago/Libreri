/**
 * Small text helpers for spell check in text boxes (kept apart from the
 * DOM so they can be tested).
 */

export interface Miss {
  start: number;
  end: number;
  word: string;
}

const WORD = /[\p{L}\p{M}\p{N}'’]/u;

/** The word being typed at `caret`: where it starts and what is typed so
 * far, if the caret is at its end. */
export function wordBefore(text: string, caret: number): { start: number; prefix: string } | null {
  if (caret < 1 || (caret < text.length && WORD.test(text[caret]!))) return null;
  let start = caret;
  while (start > 0 && WORD.test(text[start - 1]!)) start--;
  const prefix = text.slice(start, caret).replace(/^['’]+/, "");
  if (!/^\p{L}/u.test(prefix)) return null;
  return { start: caret - prefix.length, prefix };
}

/**
 * Moves marks after an edit so they stay on their words until the text is
 * checked again: marks before the edit stay, marks after it move, and a
 * mark the edit touched is dropped.
 */
export function shiftMisses(misses: Miss[], before: string, after: string): Miss[] {
  let head = 0;
  const max = Math.min(before.length, after.length);
  while (head < max && before[head] === after[head]) head++;
  let tail = 0;
  while (tail < max - head && before[before.length - 1 - tail] === after[after.length - 1 - tail])
    tail++;
  const oldEnd = before.length - tail;
  const delta = after.length - before.length;
  const out: Miss[] = [];
  for (const m of misses) {
    if (m.end < head) out.push(m);
    else if (m.start > oldEnd) out.push({ ...m, start: m.start + delta, end: m.end + delta });
  }
  return out;
}

/** Pieces of the text for drawing: plain runs and marked words. */
export function pieces(text: string, misses: Miss[]): { text: string; miss: number | null }[] {
  const out: { text: string; miss: number | null }[] = [];
  let at = 0;
  misses.forEach((m, i) => {
    if (m.start < at || m.end > text.length) return;
    if (m.start > at) out.push({ text: text.slice(at, m.start), miss: null });
    out.push({ text: text.slice(m.start, m.end), miss: i });
    at = m.end;
  });
  if (at < text.length) out.push({ text: text.slice(at), miss: null });
  return out;
}

/** A correction keeps the typed word's capital letter. */
export function matchCase(word: string, suggestion: string): string {
  if (word.length > 1 && word === word.toUpperCase() && /\p{L}/u.test(word))
    return suggestion.toUpperCase();
  if (/^\p{Lu}/u.test(word) && /^\p{Ll}/u.test(suggestion))
    return suggestion[0]!.toUpperCase() + suggestion.slice(1);
  return suggestion;
}
