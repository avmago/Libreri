import type { FileType, Hit, TextState } from "@/lib/ipc";

/** Where a match is, for people: "Page 12", "Chapter: The Storm", "Chapter 3". */
export function hitPlace(hit: Hit): string {
  if (hit.page != null) return `Page ${hit.page}`;
  if (hit.label) return hit.label;
  if (hit.section != null) return `Part ${hit.section + 1}`;
  return "In the text";
}

export const PAGED: FileType[] = ["pdf", "djvu"];

/** One line about a book's text, and how it should look. */
export function describeText(
  state: TextState | null,
  s: { emptyPages: number; pages: number; ocrPages: number },
): { label: string; tone: "ok" | "warn" | "muted" | "error" } {
  switch (state) {
    case null:
      return { label: "Not indexed yet", tone: "muted" };
    case "text":
      return {
        label: s.ocrPages ? `Searchable (${s.ocrPages} pages read with OCR)` : "Searchable",
        tone: "ok",
      };
    case "partial":
      return {
        label: `${s.emptyPages} of ${s.pages} pages are scans without text`,
        tone: "warn",
      };
    case "noText":
      return { label: "Scanned pages without text", tone: "warn" };
    case "noWords":
      return { label: "Pictures only (nothing to search)", tone: "muted" };
    case "failed":
      return { label: "The text could not be read", tone: "error" };
  }
}

/** Splits text on matches of `query`'s words, for highlighting titles and notes. */
export function markWords(text: string, query: string): { text: string; hit: boolean }[] {
  const words = query
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter((w) => w.length > 0 && !w.startsWith("-"));
  if (!words.length || !text) return [{ text, hit: false }];
  const escaped = words.map((w) => w.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"));
  const re = new RegExp(`(${escaped.join("|")})`, "giu");
  return text
    .split(re)
    .filter((p) => p !== "")
    .map((p) => ({ text: p, hit: words.includes(p.toLowerCase()) }));
}
