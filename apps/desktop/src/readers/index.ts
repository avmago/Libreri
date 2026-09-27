import type { FileType } from "@/lib/ipc";
import type { Renderer, RendererEvents } from "./types";

export * from "./types";
export { PAGE_THEMES, pageTheme, type PageTheme, type PageThemeId } from "./themes";

const PAGE_IMAGES: FileType[] = ["djvu", "cbz", "cbr", "cb7", "cbt", "cba"];

/** Formats Libreri's reader opens. The rest open in another app for now. */
export function canRead(type: FileType): boolean {
  return ["pdf", "epub", "mobi", "azw3", "fb2", "md", "txt", ...PAGE_IMAGES].includes(type);
}

/** Books with real pages (zoom, fit, go to page): PDF, DjVu and comics. */
export function isPaged(type: FileType | undefined): boolean {
  return type === "pdf" || (type !== undefined && PAGE_IMAGES.includes(type));
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
