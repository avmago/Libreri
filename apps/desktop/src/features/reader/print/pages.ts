/**
 * Reads page numbers typed as "1-5, 8, 10-12" (1-based, within `total`),
 * in order and without repeats. Returns null when something cannot be read.
 */
export function parsePages(text: string, total: number): number[] | null {
  const out: number[] = [];
  const seen = new Set<number>();
  const parts = text
    .split(/[,;]+/)
    .map((p) => p.trim())
    .filter(Boolean);
  if (!parts.length) return null;
  for (const part of parts) {
    const m = /^(\d+)\s*(?:[-–—]\s*(\d+)?)?$/.exec(part);
    if (!m) return null;
    const from = Number(m[1]);
    const to = m[2] !== undefined ? Number(m[2]) : part.includes("-") ? total : from;
    if (from < 1 || to < from || from > total) return null;
    for (let p = from; p <= Math.min(to, total); p++) {
      if (!seen.has(p)) {
        seen.add(p);
        out.push(p);
      }
    }
  }
  return out;
}
