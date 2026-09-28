/** Loads PDF.js on demand (shared by the reader and the page editor). */
type ViewerModule = typeof import("pdfjs-dist/legacy/web/pdf_viewer.mjs");

let loaded: Promise<{
  pdfjs: typeof import("pdfjs-dist/legacy/build/pdf.mjs");
  viewer: ViewerModule;
}> | null = null;

/**
 * PDF.js is big; load it the first time a PDF opens. The "legacy" build is
 * used because the standard one needs the newest JavaScript (such as
 * `Map.prototype.getOrInsertComputed`), which the system web views on older
 * macOS versions and on Linux (WebKitGTK) do not have yet.
 */
export function loadPdfJs() {
  loaded ??= (async () => {
    const pdfjs = await import("pdfjs-dist/legacy/build/pdf.mjs");
    if (!pdfjs.GlobalWorkerOptions.workerSrc) {
      const worker = await import("pdfjs-dist/legacy/build/pdf.worker.min.mjs?url");
      pdfjs.GlobalWorkerOptions.workerSrc = worker.default;
    }
    // The viewer expects the library on globalThis.
    (globalThis as { pdfjsLib?: unknown }).pdfjsLib = pdfjs;
    await import("pdfjs-dist/legacy/web/pdf_viewer.css");
    const viewer = await import("pdfjs-dist/legacy/web/pdf_viewer.mjs");
    return { pdfjs, viewer };
  })();
  return loaded;
}
