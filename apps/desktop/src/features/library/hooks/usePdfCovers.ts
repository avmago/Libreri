import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { bookUrl, commands, unwrap } from "@/lib/ipc";
import { libKey } from "../api";
import type { BookView } from "../model";

/** Files bigger than this keep their generated cover (rendering needs the whole file). */
const MAX_BYTES = 150 * 1024 * 1024;
const WIDTH = 600;

/** Books already tried this session, so a broken PDF is not retried in a loop. */
const attempted = new Set<string>();
let running = false;
const queue: BookView[] = [];

async function renderFirstPage(book: BookView): Promise<string> {
  const pdfjs = await import("pdfjs-dist");
  if (!pdfjs.GlobalWorkerOptions.workerSrc) {
    const worker = await import("pdfjs-dist/build/pdf.worker.min.mjs?url");
    pdfjs.GlobalWorkerOptions.workerSrc = worker.default;
  }
  const response = await fetch(bookUrl(book.relPath));
  if (!response.ok) throw new Error(`HTTP ${response.status}`);
  const data = new Uint8Array(await response.arrayBuffer());
  const task = pdfjs.getDocument({
    data,
    standardFontDataUrl: "/pdfjs/standard_fonts/",
    cMapUrl: "/pdfjs/cmaps/",
    cMapPacked: true,
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
    return canvas.toDataURL("image/jpeg", 0.88);
  } finally {
    void task.destroy();
  }
}

/**
 * Renders the first page of PDFs that have no cover yet (PDF.js is already
 * the reader's renderer) and stores it as their cover. Runs one at a time in
 * the background.
 */
export function usePdfCovers(books: BookView[] | undefined) {
  const qc = useQueryClient();

  useEffect(() => {
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
    void (async () => {
      let saved = 0;
      while (queue.length) {
        const book = queue.shift()!;
        try {
          const image = await renderFirstPage(book);
          await unwrap(commands.saveCover(book.id, image));
          // Refresh the grid now and then, not after every cover.
          if (++saved % 6 === 0 || queue.length === 0) {
            void qc.invalidateQueries({ queryKey: libKey });
          }
        } catch (e) {
          console.warn(`Libreri: no cover for ${book.relPath}:`, e);
        }
        // Let the interface breathe between books.
        await new Promise((r) => setTimeout(r, 30));
      }
      if (saved % 6 !== 0) void qc.invalidateQueries({ queryKey: libKey });
      running = false;
    })();
  }, [books, qc]);
}
