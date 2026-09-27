import type { FileType } from "@/lib/ipc";
import type { Renderer, RendererEvents } from "./types";

export * from "./types";
export { PAGE_THEMES, pageTheme, type PageTheme, type PageThemeId } from "./themes";

/** Formats Libreri's reader opens. The rest open in another app for now. */
export function canRead(type: FileType): boolean {
  return ["pdf", "epub", "mobi", "azw3", "fb2", "cbz", "md", "txt"].includes(type);
}

/** Loads the renderer for a format (each is its own chunk). */
export async function createRenderer(type: FileType, events: RendererEvents): Promise<Renderer> {
  switch (type) {
    case "pdf": {
      const { PdfRenderer } = await import("./pdf/PdfRenderer");
      return new PdfRenderer(events);
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
