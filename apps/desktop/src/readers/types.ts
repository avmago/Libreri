/**
 * The contract every format renderer implements (docs/code-structure.md:
 * "readers/"). A renderer draws one book into a container element; the
 * reader feature talks to it only through this interface, so PDF, ebooks
 * and Markdown share the same toolbar, panels, highlights and notebook.
 */
import type { Annotation, HighlightColor, TextQuote } from "@/lib/ipc";
import type { PageTheme } from "./themes";

/** A place in a book. Stored as JSON (`locator` in the database). */
export type Locator =
  | { type: "pdf"; page: number; top?: number }
  | { type: "pdf-highlight"; page: number; rects: Rect[] }
  | { type: "cfi"; cfi: string }
  | { type: "text"; start: number; end?: number }
  | { type: "scroll"; fraction: number };

/** A rectangle as fractions (0–1) of the page, so it fits any zoom. */
export type Rect = [x: number, y: number, w: number, h: number];

export interface TocItem {
  label: string;
  /** Opaque to the reader feature; passed back to `goTo`. */
  target: string;
  children: TocItem[];
}

export interface ReaderLocation {
  locator: Locator;
  /** 0–1 through the whole book. */
  progress: number;
  /** "Page 4 of 15", "Chapter 3", … */
  label: string;
  /** For bookmarks and highlights: "p. 4" or the chapter. */
  shortLabel: string;
  page?: number;
  pages?: number;
  /** The table-of-contents entry being read, if known. */
  section?: string;
}

export interface SelectionInfo {
  quote: TextQuote;
  locator: Locator;
  label: string;
  position: number;
  /** Where the selection is on screen, for the highlight menu. */
  rect: DOMRect;
}

export interface FindResult {
  current: number;
  total: number;
}

export type ZoomValue = number | "auto" | "page-width" | "page-fit";

export interface RendererEvents {
  relocate: (location: ReaderLocation) => void;
  selection: (selection: SelectionInfo | null) => void;
  /** A highlight was clicked. */
  annotationClick: (id: string, rect: DOMRect) => void;
  /** A `libreri://` link (from notes inside a book) or external link was clicked. */
  externalLink?: (href: string) => void;
}

export interface Renderer {
  /** Loads the book and shows `initial` (or the start). */
  open(container: HTMLElement, url: string, initial: Locator | null): Promise<void>;
  destroy(): void;
  toc(): TocItem[];
  goTo(target: string | Locator): Promise<void>;
  next(): void;
  prev(): void;
  /** Draws these highlights (replaces the previous set). */
  setAnnotations(list: Annotation[]): void;
  /** Scrolls to an annotation. */
  showAnnotation(annotation: Annotation): Promise<void>;
  find(query: string, backwards?: boolean): Promise<FindResult>;
  clearFind(): void;
  setTheme(theme: PageTheme, pdfMode: PdfDarkMode): void;
  /** Text size for reflowable books, page zoom for PDFs (1 = 100 %). */
  setZoom(zoom: ZoomValue): void;
  zoom(): ZoomValue;
  clearSelection(): void;
  /** True for PDFs (real pages you can jump to). */
  readonly paged: boolean;
}

export type PdfDarkMode = "recolour" | "invert" | "dim" | "off";

export const HIGHLIGHT_COLORS: HighlightColor[] = ["yellow", "green", "blue", "pink"];

/** Highlight fills that read well on light and dark pages. */
export function highlightFill(color: HighlightColor | null, dark: boolean): string {
  const light: Record<HighlightColor, string> = {
    yellow: "rgba(250, 204, 21, 0.38)",
    green: "rgba(74, 222, 128, 0.35)",
    blue: "rgba(96, 165, 250, 0.35)",
    pink: "rgba(244, 114, 182, 0.35)",
  };
  const night: Record<HighlightColor, string> = {
    yellow: "rgba(250, 204, 21, 0.28)",
    green: "rgba(74, 222, 128, 0.25)",
    blue: "rgba(96, 165, 250, 0.3)",
    pink: "rgba(244, 114, 182, 0.28)",
  };
  return (dark ? night : light)[color ?? "yellow"];
}

export function parseLocator(json: string | null | undefined): Locator | null {
  if (!json) return null;
  try {
    const v = JSON.parse(json) as Locator;
    return v && typeof v === "object" && "type" in v ? v : null;
  } catch {
    return null;
  }
}

export type { Annotation };
