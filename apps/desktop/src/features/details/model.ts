/**
 * The merge screen's rules: which found details to take by default, and
 * how picked details combine with the book's own. Presentation only; the
 * details themselves come from Rust (`libreri-metadata`).
 */
import type { BookMetadata, Candidate, ContentType, Query, Source } from "@/lib/ipc";
import { CONTENT_TYPE_LABEL, EMPTY_METADATA } from "@/features/library";

export type Meta = Required<BookMetadata>;
export type FieldKey = Exclude<keyof Meta, never>;

export interface FieldDef {
  key: FieldKey;
  label: string;
  /** Lists can be added to (tags, categories) or replaced (authors). */
  kind: "text" | "long" | "list" | "addList" | "number" | "type";
}

/** Fields in the order the merge screen shows them. */
export const FIELDS: FieldDef[] = [
  { key: "title", label: "Title", kind: "text" },
  { key: "subtitle", label: "Subtitle", kind: "text" },
  { key: "authors", label: "Authors", kind: "list" },
  { key: "contributors", label: "Contributors", kind: "list" },
  { key: "contentType", label: "Type", kind: "type" },
  { key: "year", label: "Year", kind: "number" },
  { key: "publisher", label: "Publisher", kind: "text" },
  { key: "edition", label: "Edition", kind: "text" },
  { key: "pages", label: "Pages", kind: "number" },
  { key: "language", label: "Language", kind: "text" },
  { key: "series", label: "Series", kind: "text" },
  { key: "seriesNumber", label: "Number in series", kind: "number" },
  { key: "isbn13", label: "ISBN-13", kind: "text" },
  { key: "isbn10", label: "ISBN-10", kind: "text" },
  { key: "doi", label: "DOI", kind: "text" },
  { key: "arxivId", label: "arXiv", kind: "text" },
  { key: "journal", label: "Journal", kind: "text" },
  { key: "volume", label: "Volume", kind: "text" },
  { key: "issue", label: "Issue", kind: "text" },
  { key: "tags", label: "Tags", kind: "addList" },
  { key: "categories", label: "Categories", kind: "addList" },
  { key: "about", label: "About", kind: "long" },
  { key: "url", label: "Web page", kind: "text" },
];

export const SOURCE_LABEL: Record<Source, string> = {
  openLibrary: "Open Library",
  googleBooks: "Google Books",
  crossref: "Crossref",
  openAlex: "OpenAlex",
  semanticScholar: "Semantic Scholar",
  arxiv: "arXiv",
  comicVine: "ComicVine",
  isbndb: "ISBNdb",
};

/** Found details with every field present. */
export function full(m: BookMetadata): Meta {
  return { ...EMPTY_METADATA, ...m };
}

export function isEmpty(value: unknown): boolean {
  return (
    value === null ||
    value === undefined ||
    value === "" ||
    (Array.isArray(value) && value.length === 0)
  );
}

/** A field's value as text, for the merge table. */
export function show(field: FieldDef, value: unknown): string {
  if (isEmpty(value)) return "";
  if (field.kind === "type") return CONTENT_TYPE_LABEL[value as ContentType];
  if (Array.isArray(value)) return value.join(field.key === "categories" ? " · " : ", ");
  return String(value);
}

function same(a: unknown, b: unknown): boolean {
  if (Array.isArray(a) && Array.isArray(b)) {
    return (
      a.length === b.length &&
      a.every((x, i) => String(x).toLowerCase() === String(b[i]).toLowerCase())
    );
  }
  return typeof a === "string" && typeof b === "string"
    ? a.trim().toLowerCase() === b.trim().toLowerCase()
    : a === b;
}

/** Items of `found` not already in `current` (case-insensitive). */
export function newItems(current: string[], found: string[]): string[] {
  const have = new Set(current.map((x) => x.toLowerCase()));
  return found.filter((x) => !have.has(x.toLowerCase()));
}

/** True if taking this field from the candidate would change the book. */
export function differs(field: FieldDef, current: Meta, found: Meta): boolean {
  const f = found[field.key];
  if (isEmpty(f)) return false;
  // Sources call anything they cannot tell apart a "book": no news.
  if (field.key === "contentType") return f !== "book" && f !== current.contentType;
  if (field.kind === "addList")
    return newItems(current[field.key] as string[], f as string[]).length > 0;
  return !same(current[field.key], f);
}

/** Which candidate each field is taken from; fields not listed stay. */
export type Picks = Partial<Record<FieldKey, number>>;

/**
 * The fields taken without asking: those the book has empty, new tags and
 * categories, and a more specific type than "Book".
 */
export function defaultPicks(current: Meta, found: Meta, index: number): Picks {
  const picks: Picks = {};
  for (const field of FIELDS) {
    if (!differs(field, current, found)) continue;
    const empty =
      field.key === "contentType" ? current.contentType === "book" : isEmpty(current[field.key]);
    if (empty || field.kind === "addList") picks[field.key] = index;
  }
  return picks;
}

/** The book's details with the picked fields taken from the candidates. */
export function merge(current: Meta, candidates: Meta[], picks: Picks): Meta {
  const out: Meta = { ...current };
  for (const field of FIELDS) {
    const i = picks[field.key];
    const found = i === undefined ? undefined : candidates[i];
    if (!found || isEmpty(found[field.key])) continue;
    if (field.kind === "addList") {
      const list = current[field.key] as string[];
      (out[field.key] as string[]) = [...list, ...newItems(list, found[field.key] as string[])];
    } else {
      (out as Record<string, unknown>)[field.key] = found[field.key];
    }
  }
  return out;
}

/** How sure a candidate is, in words. */
export function matchLabel(c: Candidate): string {
  const s = c.score ?? 0;
  if (s >= 1) return "Same identifier";
  if (s >= 0.85) return "Close match";
  if (s >= 0.6) return "Possible match";
  return "Weak match";
}

/** Tidies what the reader typed into the search form. */
export function cleanQuery(q: Query): Query {
  const t = (v: string | null | undefined) => {
    const s = (v ?? "").trim();
    return s ? s : null;
  };
  return {
    isbn: t(q.isbn),
    doi: t(q.doi),
    arxivId: t(q.arxivId),
    title: t(q.title),
    author: t(q.author),
    contentType: q.contentType ?? null,
  };
}
