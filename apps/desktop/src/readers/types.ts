/**
 * The contract every format renderer implements (docs/code-structure.md:
 * "readers/"). A renderer draws one book into a container element; the
 * reader feature talks to it only through this interface, so PDF, ebooks
 * and Markdown share the same toolbar, panels, highlights and notebook.
 */
import type { Annotation, HighlightColor, TextQuote } from "@/lib/ipc";
import type { MarkupLayer } from "./markup/MarkupLayer";
import type { PageTheme } from "./themes";
import type { SpeechSource } from "./speech/types";
import type { PageClip } from "./clip";
import type { Glyph } from "./math/layout";
import type { BionicOptions, LineBox } from "./focus";

/** A place in a book. Stored as JSON (`locator` in the database). */
export type Locator =
  | { type: "pdf"; page: number; top?: number }
  | { type: "pdf-highlight"; page: number; rects: Rect[] }
  | { type: "cfi"; cfi: string }
  | { type: "text"; start: number; end?: number }
  | { type: "scroll"; fraction: number }
  | { type: "audio"; t: number };

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
  /** Page-based books: the selected characters with their boxes, for
   * rebuilding maths as LaTeX. */
  glyphs?: () => Glyph[];
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
  /** A formula was clicked: its LaTeX and where it is on screen. */
  mathClick?: (latex: string, rect: DOMRect) => void;
  /** PDF forms: fields were filled in (true) or put back (false). */
  formChanged?: (dirty: boolean) => void;
  /** The pointer moved over a page drawn in its own frame (EPUB), in
   * window coordinates: the app does not see those moves. */
  pointer?: (x: number, y: number) => void;
}

export interface Renderer {
  /** Loads the book and shows `initial` (or the start). */
  open(container: HTMLElement, url: string, initial: Locator | null): Promise<void>;
  destroy(): void;
  toc(): TocItem[];
  goTo(target: string | Locator): Promise<void>;
  next(): void;
  prev(): void;
  /** Scrolls a little (arrow keys, Vim j/k); paged books turn the page. */
  scrollBy(direction: 1 | -1): void;
  /** Goes to the first or last page. */
  start(): Promise<void>;
  end(): Promise<void>;
  /** Line spacing for reflowable text (ignored by PDFs and comics). */
  setLineHeight(lineHeight: number): void;
  /** Draws these highlights (replaces the previous set). */
  setAnnotations(list: Annotation[]): void;
  /** Scrolls to an annotation. */
  showAnnotation(annotation: Annotation): Promise<void>;
  find(query: string, backwards?: boolean): Promise<FindResult>;
  clearFind(): void;
  /**
   * Before finding words from a search result: goes to where they were
   * found, so the next `find` starts there.
   */
  prepareFind?(hint: { page?: number | null; section?: number | null }): Promise<void>;
  setTheme(theme: PageTheme, pdfMode: PdfDarkMode): void;
  /** Text size for reflowable books, page zoom for PDFs (1 = 100 %). */
  setZoom(zoom: ZoomValue): void;
  /** Zooms by a factor around a point on screen (pinch, Ctrl + wheel),
   * keeping that point under the fingers. Renderers without it are zoomed
   * with `setZoom`. */
  zoomBy?(factor: number, clientX: number, clientY: number): void;
  zoom(): ZoomValue;
  clearSelection(): void;
  /** True for PDFs, DjVu and comics (real pages you can jump to). */
  readonly paged: boolean;
  /** Comics read right to left: the arrow keys turn the other way. */
  readonly rightToLeft?: boolean;
  /** Fixed pages: draws markup on each page (null detaches it). */
  attachMarkup?(layer: MarkupLayer | null): void;
  /** Page size in points (PDF) or pixels (images), for measuring. */
  pageSize?(page: number): { size: [number, number]; points: boolean } | null;
  /** Comics: how pages are laid out (null for other books). */
  layoutOptions?(): PageLayout | null;
  setLayout?(layout: Partial<PageLayout>): void;
  /** PDF forms: the PDF with the fields as filled in. */
  saveForm?(): Promise<Uint8Array>;
  /** Read aloud: the text from the place shown, a sentence at a time
   * (null when the book has no text to read). */
  readAloud?(): Promise<SpeechSource | null>;
  /** The part of a page under a rectangle on the screen, as a picture
   * (page-based books). */
  clipPicture?(rect: DOMRect): Promise<PageClip | null>;
  /** Goes to a place given as 0–1 through the book (audiobook sync). */
  goToFraction?(fraction: number): Promise<void>;
  /** ADHD reading: the start of each word bold (null takes it off).
   * Only books whose text Libreri lays out. */
  setBionic?(options: BionicOptions | null): void;
  /** The line of text at a point in the window, for books drawn in
   * frames (others are looked up in the window's own document). */
  lineAt?(x: number, y: number): LineBox | null;
}

/** How comic pages are shown. */
export interface PageLayout {
  mode: "scroll" | "single" | "spread";
  rightToLeft: boolean;
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

/** Annotations drawn on the text: highlights, and voice notes and links
 * about selected text (drawn with dotted and dashed underlines). */
export function isDrawn(a: Annotation): boolean {
  return a.kind === "highlight" || ((a.kind === "voice" || a.kind === "link") && !!a.quote);
}

/** Classes for a drawn annotation. */
export function drawnClass(base: string, a: Annotation): string {
  const own = a.kind === "voice" ? " lb-hl-voice" : a.kind === "link" ? " lb-hl-link" : "";
  const note = a.note && a.kind === "highlight" ? " lb-hl-note" : "";
  return `${base}${note}${own}`;
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

/** The drawn annotation (a `.lb-pdf-hl` box) under a point on the screen.
 * Found by position: the boxes let clicks through to the text below (so it
 * can be selected), which also hides them from `elementsFromPoint`. */
export function highlightAt(root: HTMLElement, x: number, y: number): HTMLElement | null {
  let best: HTMLElement | null = null;
  let area = Infinity;
  for (const el of root.querySelectorAll<HTMLElement>(".lb-pdf-hl")) {
    const r = el.getBoundingClientRect();
    // A little room below for underlines (voice notes and links).
    if (x < r.left || x > r.right || y < r.top || y > r.bottom + 3) continue;
    // The smallest box wins where highlights overlap.
    if (r.width * r.height < area) {
      area = r.width * r.height;
      best = el;
    }
  }
  return best;
}
