/**
 * Text quotes: the highlighted words plus a little context, and finding them
 * again in a text (the fallback anchor described in docs/data-portability.md).
 */
import type { TextQuote } from "@/lib/ipc";

const CONTEXT = 32;

export function makeQuote(text: string, start: number, end: number): TextQuote {
  return {
    exact: text.slice(start, end),
    prefix: text.slice(Math.max(0, start - CONTEXT), start),
    suffix: text.slice(end, end + CONTEXT),
  };
}

/** Length of the common ending of `a` and the start of... used for scoring. */
function commonSuffix(a: string, b: string): number {
  let n = 0;
  while (n < a.length && n < b.length && a[a.length - 1 - n] === b[b.length - 1 - n]) n++;
  return n;
}

function commonPrefix(a: string, b: string): number {
  let n = 0;
  while (n < a.length && n < b.length && a[n] === b[n]) n++;
  return n;
}

/**
 * Finds `quote` in `text`. Among several matches, prefers the one whose
 * context matches best, then the one nearest `hint` (a character offset).
 * Returns `[start, end]` or `null`.
 */
export function findQuote(text: string, quote: TextQuote, hint = 0): [number, number] | null {
  if (!quote.exact) return null;
  let best: [number, number] | null = null;
  let bestScore = -Infinity;
  for (let i = text.indexOf(quote.exact); i !== -1; i = text.indexOf(quote.exact, i + 1)) {
    const end = i + quote.exact.length;
    const before = text.slice(Math.max(0, i - CONTEXT), i);
    const after = text.slice(end, end + CONTEXT);
    const score =
      commonSuffix(before, quote.prefix ?? "") +
      commonPrefix(after, quote.suffix ?? "") -
      Math.abs(i - hint) / Math.max(1, text.length);
    if (score > bestScore) {
      bestScore = score;
      best = [i, end];
    }
  }
  return best;
}
