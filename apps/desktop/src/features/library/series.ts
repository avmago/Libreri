/**
 * Series (Phase 10): books grouped by their series, in reading order, with
 * how far you are and which book comes next.
 */
import type { BookView } from "./model";

export interface Series {
  name: string;
  books: BookView[];
  read: number;
  reading: number;
  authors: string[];
  /** The book to read next: the one being read, else the first not read. */
  next: BookView | null;
}

const order = (a: BookView, b: BookView) =>
  (a.metadata.seriesNumber ?? Infinity) - (b.metadata.seriesNumber ?? Infinity) ||
  a.metadata.title.localeCompare(b.metadata.title);

/** One series' books, in order. */
export function inSeries(books: BookView[], name: string): BookView[] {
  const key = name.trim().toLowerCase();
  return books.filter((b) => b.metadata.series?.trim().toLowerCase() === key).sort(order);
}

/** Every series (two or more books, or one numbered book), by name. */
export function allSeries(books: BookView[]): Series[] {
  const groups = new Map<string, BookView[]>();
  for (const b of books) {
    const s = b.metadata.series?.trim();
    if (!s) continue;
    const k = s.toLowerCase();
    groups.set(k, [...(groups.get(k) ?? []), b]);
  }
  const out: Series[] = [];
  for (const list of groups.values()) {
    if (list.length < 2 && list[0]?.metadata.seriesNumber == null) continue;
    list.sort(order);
    const read = list.filter((b) => b.user.status === "finished").length;
    const reading = list.filter((b) => b.user.status === "reading").length;
    const counts = new Map<string, number>();
    for (const b of list)
      for (const a of b.metadata.authors) counts.set(a, (counts.get(a) ?? 0) + 1);
    out.push({
      name: list[0]!.metadata.series!.trim(),
      books: list,
      read,
      reading,
      authors: [...counts.entries()]
        .sort((a, b) => b[1] - a[1])
        .map(([a]) => a)
        .slice(0, 2),
      next:
        list.find((b) => b.user.status === "reading") ??
        list.find((b) => b.user.status !== "finished") ??
        null,
    });
  }
  return out.sort((a, b) => a.name.localeCompare(b.name));
}
