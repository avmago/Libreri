import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { bookUrl, commands, unwrap } from "@/lib/ipc";
import { PDF_ASSETS } from "@/readers";
import type { BookView } from "../model";

/** Files bigger than this keep their generated cover (rendering needs the whole file). */
const MAX_BYTES = 150 * 1024 * 1024;
const WIDTH = 600;

/** Books already tried this session, so a broken PDF is not retried in a loop. */
const attempted = new Set<string>();
let running = false;
const queue: BookView[] = [];

async function renderFirstPage(book: BookView): Promise<string> {
  const pdfjs = await import("pdfjs-dist/legacy/build/pdf.mjs");
  if (!pdfjs.GlobalWorkerOptions.workerSrc) {
    const worker = await import("pdfjs-dist/legacy/build/pdf.worker.min.mjs?url");
    pdfjs.GlobalWorkerOptions.workerSrc = worker.default;
  }
  const response = await fetch(bookUrl(book.relPath));
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const data = new Uint8Array(await response.arrayBuffer());
  const task = pdfjs.getDocument({
    data,
    ...PDF_ASSETS,
  });
  const doc = await task.promise;
  try {
    const page = await doc.getPage(1);
    const base = page.getViewport({ scale: 1 });
    const viewport = page.getViewport({ scale: WIDTH / base.width });
    const canvas = document.createElement("canvas");
    canvas.width = Math.round(viewport.width);
    canvas.height = Math.round(viewport.height);
    const ctx = canvas.getContext("2d");
    if (!ctx) throw new Error("no canvas");
    ctx.fillStyle = "#ffffff";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    await page.render({ canvas, canvasContext: ctx, viewport }).promise;
    // A plain page means nothing could be drawn (or truly a blank page):
    // keep no cover, so the generated cover with the title shows instead.
    if (isPlain(ctx.getImageData(0, 0, canvas.width, canvas.height).data)) {
      throw new Error("blank first page");
    }
    return canvas.toDataURL("image/jpeg", 0.88);
  } finally {
    void task.destroy();
  }
}

/** True if every sampled pixel is almost the same colour. */
export function isPlain(rgba: Uint8ClampedArray): boolean {
  const step = Math.max(4, Math.floor(rgba.length / 4 / 4000) * 4);
  const [r, g, b] = [rgba[0]!, rgba[1]!, rgba[2]!];
  for (let i = 0; i < rgba.length; i += step) {
    if (
      Math.abs(rgba[i]! - r) > 12 ||
      Math.abs(rgba[i + 1]! - g) > 12 ||
      Math.abs(rgba[i + 2]! - b) > 12
    ) {
      return false;
    }
  }
  return true;
}

/**
 * Renders the first page of PDFs that have no cover yet (PDF.js is already
 * the reader's renderer) and stores it as their cover. Runs one at a time in
 * the background, and only for profiles that may change the library (the
 * whole file is read for each cover, and others could not save it).
 */
export function usePdfCovers(books: BookView[] | undefined, enabled: boolean) {
  const qc = useQueryClient();

  useEffect(() => {
    if (!enabled) {
      // Signed in as someone who cannot save covers: stop what is waiting.
      queue.length = 0;
      return;
    }
    if (!books) return;
    for (const b of books) {
      if (
        b.fileType === "pdf" &&
        !b.hasCover &&
        !b.missing &&
        b.fileSize <= MAX_BYTES &&
        !attempted.has(b.id)
      ) {
        attempted.add(b.id);
        queue.push(b);
      }
    }
    if (running) return;
    running = true;
    // Only the lists of books show covers: the rest of the library stays.
    const refresh = () =>
      void qc.invalidateQueries({
        predicate: (q) =>
          q.queryKey[0] === "lib" && (q.queryKey[1] === "books" || q.queryKey[1] === "book"),
      });
    void (async () => {
      let unshown = 0;
      while (queue.length) {
        const book = queue.shift()!;
        try {
          const image = await renderFirstPage(book);
          await unwrap(commands.saveCover(book.id, image));
          // Refresh the grid now and then, not after every cover.
          if (++unshown === 6) {
            unshown = 0;
            refresh();
          }
        } catch (e) {
          console.warn(`Libreri: no cover for ${book.relPath}:`, e);
        }
        // Let the interface breathe between books.
        await new Promise((r) => setTimeout(r, 30));
      }
      if (unshown > 0) refresh();
      running = false;
    })();
  }, [books, enabled, qc]);
}
