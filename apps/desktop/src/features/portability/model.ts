import type { ArchiveImported, CitationStyle, ExportFormat, ForeignImported } from "@/lib/ipc";

export type ExportOption = "personal" | "notes" | "bookFiles" | "everyone";

export interface FormatInfo {
  id: ExportFormat;
  label: string;
  hint: string;
  group: "Everything" | "Tables and data" | "Citations" | "Other apps";
  /** File extension, or null for formats that make a folder. */
  ext: string | null;
  options: ExportOption[];
  /** Can export just the selected books (the database copy is always whole). */
  selection: boolean;
}

/** Every export format, in the order the dialog shows them. */
export const FORMATS: FormatInfo[] = [
  {
    id: "archive",
    label: "Libreri archive",
    hint: "Everything, to import into Libreri on another computer. Every link in your notes keeps working.",
    group: "Everything",
    ext: "libreri",
    options: ["notes", "bookFiles", "everyone"],
    selection: true,
  },
  {
    id: "xlsx",
    label: "Excel workbook",
    hint: "One row per book, with a filter on every column.",
    group: "Tables and data",
    ext: "xlsx",
    options: ["personal"],
    selection: true,
  },
  {
    id: "csv",
    label: "CSV",
    hint: "One row per book; opens in any spreadsheet.",
    group: "Tables and data",
    ext: "csv",
    options: ["personal"],
    selection: true,
  },
  {
    id: "json",
    label: "JSON",
    hint: "Every detail, and optionally your highlights and notebooks, for scripts and other tools.",
    group: "Tables and data",
    ext: "json",
    options: ["personal", "notes"],
    selection: true,
  },
  {
    id: "sqlite",
    label: "Catalogue database",
    hint: "A copy of Libreri's SQLite catalogue for your own queries. Always the whole library.",
    group: "Tables and data",
    ext: "db",
    options: ["everyone"],
    selection: false,
  },
  {
    id: "bibtex",
    label: "BibTeX",
    hint: "For LaTeX, JabRef and Zotero.",
    group: "Citations",
    ext: "bib",
    options: [],
    selection: true,
  },
  {
    id: "ris",
    label: "RIS",
    hint: "For EndNote, Mendeley and Zotero.",
    group: "Citations",
    ext: "ris",
    options: [],
    selection: true,
  },
  {
    id: "cslJson",
    label: "CSL-JSON",
    hint: "For Pandoc, citeproc and Zotero.",
    group: "Citations",
    ext: "json",
    options: [],
    selection: true,
  },
  {
    id: "obsidian",
    label: "Obsidian notes",
    hint: "A folder of Markdown notes: one per book, with your highlights linking back to their place in Libreri.",
    group: "Other apps",
    ext: null,
    options: ["personal", "notes"],
    selection: true,
  },
  {
    id: "calibre",
    label: "Calibre folders",
    hint: "Each book in its own folder with metadata.opf and its cover. In Calibre: Add books › Add from folders.",
    group: "Other apps",
    ext: null,
    options: ["personal", "bookFiles"],
    selection: true,
  },
];

export const OPTION_LABEL: Record<ExportOption, string> = {
  personal: "My reading status, ratings and favourites",
  notes: "My highlights, bookmarks and notebooks",
  bookFiles: "The book files",
  everyone: "Everyone's notes (all profiles)",
};

export function formatInfo(id: ExportFormat): FormatInfo {
  return FORMATS.find((f) => f.id === id) ?? FORMATS[0]!;
}

/** Characters no file system accepts become spaces. */
export function safeName(s: string): string {
  const cleaned = s
    .replace(/[/\\:*?"<>|]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^\.+|\.+$/g, "");
  return cleaned.slice(0, 100) || "Libreri export";
}

/** "Home library 2026-09-28.xlsx", "Optics.bib", "Home library – 3 books.csv". */
export function defaultFileName(
  format: FormatInfo,
  library: string,
  books: { title: string }[] | null,
  today = new Date(),
): string {
  const date = today.toISOString().slice(0, 10);
  const stem =
    books === null
      ? `${library} ${date}`
      : books.length === 1
        ? books[0]!.title
        : `${library} – ${books.length} books`;
  const suffix =
    format.id === "obsidian" ? " notes" : format.id === "calibre" ? " for Calibre" : "";
  const name = safeName(stem + suffix);
  return format.ext ? `${name}.${format.ext}` : name;
}

export const CITATION_STYLES: { id: CitationStyle; label: string }[] = [
  { id: "apa", label: "APA" },
  { id: "mla", label: "MLA" },
  { id: "chicago", label: "Chicago" },
  { id: "harvard", label: "Harvard" },
  { id: "ieee", label: "IEEE" },
  { id: "bibtex", label: "BibTeX" },
];

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}

/** "248 books linked · 3 matched another copy · 1 missing". */
export function summariseImport(r: ArchiveImported) {
  const books: string[] = [];
  if (r.linked) books.push(`${plural(r.linked, "book")} already here`);
  if (r.added) books.push(`${plural(r.added, "book")} added`);
  if (r.otherFile) books.push(`${r.otherFile} matched another copy`);
  if (r.missing) books.push(`${r.missing} without a file`);
  if (r.filesRestored) books.push(`${plural(r.filesRestored, "missing file")} put back`);
  const notes: string[] = [];
  if (r.notesAdded) notes.push(`${plural(r.notesAdded, "note")} added`);
  if (r.notesUpdated) notes.push(`${r.notesUpdated} updated`);
  if (r.noteFilesAdded) notes.push(`${plural(r.noteFilesAdded, "notebook file")} added`);
  if (r.noteConflicts.length)
    notes.push(`${plural(r.noteConflicts.length, "notebook")} kept side by side`);
  const lines = [books.join(" · "), notes.join(" · ")];
  if (r.profilesCreated.length) lines.push(`New profiles: ${r.profilesCreated.join(", ")}`);
  lines.push(...r.warnings.slice(0, 2));
  return {
    title: r.notesAdded || r.added ? "Archive imported" : "Archive imported; nothing new",
    description: lines.filter(Boolean).join("\n"),
  };
}

/** "1.2 MB". */
export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

/** "28 Sep 2026, 14:30" in the reader's own locale. */
export function formatWhen(iso: string | null | undefined): string {
  if (!iso) return "never";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

/** "Imported 298 books from Calibre · 14 were already here…". */
export function summariseForeign(r: ForeignImported) {
  const lines: string[] = [];
  const parts: string[] = [];
  if (r.alreadyHere) parts.push(`${plural(r.alreadyHere, "book")} already here`);
  if (r.detailsAdded) parts.push(`details added to ${plural(r.detailsAdded, "book")}`);
  if (r.highlightsAdded) parts.push(`${plural(r.highlightsAdded, "highlight")} added`);
  if (r.notesAdded) parts.push(`${plural(r.notesAdded, "note")} added to notebooks`);
  if (r.personalUpdated)
    parts.push(`reading status or rating of ${plural(r.personalUpdated, "book")} updated`);
  lines.push(parts.join(" · "));
  if (r.withoutFileCount)
    lines.push(
      `${plural(r.withoutFileCount, "book")} had no file and no match here: ${r.withoutFile.slice(0, 3).join(", ")}${r.withoutFileCount > 3 ? "…" : ""}`,
    );
  if (r.unmatchedCount)
    lines.push(
      `${plural(r.unmatchedCount, "book")} not in this library: ${r.unmatched.slice(0, 3).join(", ")}${r.unmatchedCount > 3 ? "…" : ""}`,
    );
  if (r.failed.length) lines.push(...r.failed.slice(0, 2).map((f) => `${f.file}: ${f.reason}`));
  const title = r.added
    ? `Imported ${plural(r.added, "book")} from ${r.source}`
    : r.personalUpdated || r.detailsAdded || r.highlightsAdded
      ? `Updated from ${r.source}`
      : `Nothing new from ${r.source}`;
  return { title, description: lines.filter(Boolean).join("\n") };
}
