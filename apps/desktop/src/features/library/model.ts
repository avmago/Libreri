/**
 * View-side helpers for books: labels, formatting and the normalised shape
 * the components use. Rules about books live in Rust; this is presentation.
 */
import type {
  BookDto,
  BookMetadata,
  BookUserState,
  ContentType,
  FileType,
  ReadingStatus,
  SortKey,
} from "@/lib/ipc";

/** A book with every optional field filled in, so components need no `??`. */
export type BookView = Omit<BookDto, "metadata" | "user"> & {
  metadata: Required<BookMetadata>;
  user: Required<BookUserState>;
};

export const EMPTY_METADATA: Required<BookMetadata> = {
  title: "",
  subtitle: null,
  authors: [],
  contributors: [],
  about: null,
  tags: [],
  categories: [],
  year: null,
  publisher: null,
  pages: null,
  isbn13: null,
  isbn10: null,
  edition: null,
  language: null,
  contentType: "book",
  series: null,
  seriesNumber: null,
  doi: null,
  arxivId: null,
  journal: null,
  volume: null,
  issue: null,
  url: null,
};

const EMPTY_USER: Required<BookUserState> = {
  status: "none",
  rating: 0,
  favorite: false,
  progress: 0,
  lastOpened: null,
};

export function toView(b: BookDto): BookView {
  return {
    ...b,
    metadata: { ...EMPTY_METADATA, ...b.metadata },
    user: { ...EMPTY_USER, ...b.user },
  };
}

export const FILE_TYPE_LABEL: Record<FileType, string> = {
  pdf: "PDF",
  epub: "EPUB",
  mobi: "MOBI",
  azw3: "AZW3",
  fb2: "FB2",
  txt: "Text",
  md: "Markdown",
  djvu: "DjVu",
  cbz: "CBZ",
  cbr: "CBR",
  cb7: "CB7",
  cba: "CBA",
  cbt: "CBT",
  mp3: "MP3",
  m4b: "M4B",
  m4a: "M4A",
  aac: "AAC",
  ogg: "OGG",
  opus: "Opus",
  flac: "FLAC",
};

export const CONTENT_TYPE_LABEL: Record<ContentType, string> = {
  book: "Book",
  textbook: "Textbook",
  researchPaper: "Research paper",
  conferencePaper: "Conference paper",
  preprint: "Preprint",
  thesis: "Thesis / dissertation",
  lectureNotes: "Lecture notes",
  slides: "Slides",
  technicalReport: "Technical report",
  whitePaper: "White paper",
  manual: "Manual / documentation",
  reference: "Reference / handbook",
  standard: "Standard / specification",
  magazine: "Magazine / periodical",
  article: "Article / essay",
  comic: "Comic / manga",
  cheatSheet: "Cheat sheet",
  personalNotes: "Personal notes",
  audiobook: "Audiobook",
  other: "Other",
};

export const STATUS_LABEL: Record<ReadingStatus, string> = {
  none: "No status",
  wantToRead: "Want to read",
  reading: "Reading",
  finished: "Finished",
  abandoned: "Stopped reading",
};

export const SORT_LABEL: Record<SortKey, string> = {
  title: "Title",
  author: "Author",
  added: "Date added",
  year: "Year",
  size: "File size",
  pages: "Pages",
  lastOpened: "Last opened",
  rating: "Rating",
};

export function formatSize(bytes: number): string {
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

export function formatDate(iso: string | null): string {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime())
    ? ""
    : d.toLocaleDateString(undefined, { year: "numeric", month: "short", day: "numeric" });
}

export function authorsText(authors: string[], max = 2): string {
  if (authors.length === 0) return "Unknown author";
  if (authors.length <= max) return authors.join(", ");
  return `${authors.slice(0, max).join(", ")} +${authors.length - max}`;
}

/** File name part of a library-relative path. */
export function fileName(relPath: string): string {
  return relPath.slice(relPath.lastIndexOf("/") + 1);
}

/**
 * A stable colour pair for a generated cover, picked from the title so a
 * book always looks the same. Muted tones that work in light and dark.
 */
const COVER_TONES = [
  ["#1f2937", "#f9fafb"],
  ["#3f3a36", "#f5efe6"],
  ["#1e3a5f", "#e8f0fa"],
  ["#3b2f4a", "#f1ebf7"],
  ["#23413a", "#e7f4ef"],
  ["#5a2a27", "#fbeeed"],
  ["#4a4130", "#f7f1e1"],
  ["#2d3a45", "#eaf1f6"],
] as const;

export function coverTone(seed: string): readonly [string, string] {
  let h = 0;
  for (let i = 0; i < seed.length; i++) h = (h * 31 + seed.charCodeAt(i)) | 0;
  return COVER_TONES[Math.abs(h) % COVER_TONES.length] ?? COVER_TONES[0];
}

/**
 * Adds what was typed into a chip field (several at once when separated by
 * commas, semicolons or new lines), skipping ones already there.
 */
export function addListItems(list: string[], typed: string, splitOnComma = true): string[] {
  const items = typed
    .split(splitOnComma ? /[,;\n]/ : /[;\n]/)
    .map((s) => s.trim())
    .filter(Boolean);
  const added = items.filter((item, i) => !list.includes(item) && items.indexOf(item) === i);
  return added.length ? [...list, ...added] : list;
}

/** A number field as typed: empty = null, `undefined` = not a number. */
export function parseNumberField(text: string): number | null | undefined {
  const t = text.trim().replace(",", ".");
  if (t === "") return null;
  const n = Number(t);
  return Number.isFinite(n) ? n : undefined;
}
