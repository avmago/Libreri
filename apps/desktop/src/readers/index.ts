import type { FileType } from "@/lib/ipc";
import type { Renderer, RendererEvents } from "./types";

export * from "./types";
export { PAGE_THEMES, pageTheme, type PageTheme, type PageThemeId } from "./themes";
export { lineAt, type BionicOptions, type LineBox } from "./focus";

const PAGE_IMAGES: FileType[] = ["djvu", "cbz", "cbr", "cb7", "cbt", "cba"];

const AUDIO: FileType[] = ["mp3", "m4b", "m4a", "aac", "ogg", "opus", "flac"];

/** Formats Libreri's reader opens. The rest open in another app for now. */
export function canRead(type: FileType): boolean {
  return [
    "pdf",
    "epub",
    "mobi",
    "azw3",
    "fb2",
    "md",
    "rtf",
    "txt",
    ...PAGE_IMAGES,
    ...AUDIO,
  ].includes(type);
}

/** Audiobooks: opened in the player. */
export function isAudio(type: FileType | undefined): boolean {
  return type !== undefined && AUDIO.includes(type);
}

/** Books with real pages (zoom, fit, go to page): PDF, DjVu and comics. */
export function isPaged(type: FileType | undefined): boolean {
  return type === "pdf" || (type !== undefined && PAGE_IMAGES.includes(type));
}

/** Books whose text Libreri lays out, so bionic reading can change it. */
export function canBionic(type: FileType | undefined): boolean {
  return (
    type !== undefined &&
    ["epub", "mobi", "azw3", "fb2", "md", "rtf", "txt", "pdf", "djvu"].includes(type)
  );
}

/** Loads the renderer for a format (each is its own chunk). */
export async function createRenderer(
  type: FileType,
  events: RendererEvents,
  bookId: string,
): Promise<Renderer> {
  if (PAGE_IMAGES.includes(type)) {
    const { PageRenderer } = await import("./pages/PageRenderer");
    return new PageRenderer(events, bookId);
  }
  switch (type) {
    case "pdf": {
      const { PdfRenderer } = await import("./pdf/PdfRenderer");
      return new PdfRenderer(events, bookId);
    }
    case "md":
    case "rtf":
    case "txt": {
      const { DocumentRenderer } = await import("./document/DocumentRenderer");
      return new DocumentRenderer(type, events);
    }
    default: {
      const { EbookRenderer } = await import("./ebook/EbookRenderer");
      return new EbookRenderer(events);
    }
  }
}
export { PDF_ASSETS } from "./pdf/assets";
export { MarkupLayer, DEFAULT_STYLE, stampColor, type MarkupStyle } from "./markup/MarkupLayer";
export * as markupModel from "./markup/model";
export type { Mark, MarkupItem, ToolName, Scale, Unit, MeasureKind, Pt } from "./markup/model";
export { drawList, pdfAnnots } from "./markup/export";
export { loadPdfJs } from "./pdf/load";
export type { SpeechPiece, SpeechSource } from "./speech/types";
export { sentences, mathmlToSpeech, texToSpeech } from "./speech";
export type { PageClip } from "./clip";
export { mathmlToLatex, textToLatex } from "./math/latex";
export { layoutToLatex } from "./math/layout";
