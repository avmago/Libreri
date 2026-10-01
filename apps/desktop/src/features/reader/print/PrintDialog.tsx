import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Loader2, Printer } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { printWindow } from "@/lib/print";
import { loadPdfJs, PDF_ASSETS } from "@/readers";
import { parsePages } from "./pages";

type Which = "all" | "current" | "range";

/** Pixels per inch pages are drawn at for printing: sharp text, sane memory. */
const DPI = 150;

/**
 * Print a PDF: all pages, the page shown, or a range. The pages are drawn
 * as pictures into a print-only part of the window, then the system's
 * print dialog opens (choose a printer, or save as PDF there).
 */
export function PrintDialog({
  open,
  url,
  title,
  page,
  pages,
  onClose,
}: {
  open: boolean;
  /** The PDF's `book://` address. */
  url: string;
  title: string;
  /** The page shown (1-based) and how many there are. */
  page: number;
  pages: number;
  onClose: () => void;
}) {
  const [which, setWhich] = useState<Which>("all");
  const [range, setRange] = useState("");
  const [busy, setBusy] = useState<{ done: number; total: number } | null>(null);
  const [images, setImages] = useState<string[]>([]);
  const stop = useRef(false);

  // Free the pictures when done.
  useEffect(
    () => () => {
      for (const u of images) URL.revokeObjectURL(u);
    },
    [images],
  );

  const chosen =
    which === "all"
      ? Array.from({ length: pages }, (_, i) => i + 1)
      : which === "current"
        ? [page]
        : parsePages(range, pages);

  const run = async () => {
    if (!chosen?.length) return;
    stop.current = false;
    setBusy({ done: 0, total: chosen.length });
    const made: string[] = [];
    try {
      const { pdfjs } = await loadPdfJs();
      const task = pdfjs.getDocument({ url, ...PDF_ASSETS });
      const doc = await task.promise;
      try {
        for (const n of chosen) {
          if (stop.current) throw new Error("cancelled");
          const p = await doc.getPage(n);
          const viewport = p.getViewport({ scale: DPI / 72 });
          const canvas = document.createElement("canvas");
          canvas.width = Math.ceil(viewport.width);
          canvas.height = Math.ceil(viewport.height);
          const ctx = canvas.getContext("2d")!;
          ctx.fillStyle = "#fff";
          ctx.fillRect(0, 0, canvas.width, canvas.height);
          await p.render({ canvas, canvasContext: ctx, viewport }).promise;
          const blob = await new Promise<Blob | null>((ok) =>
            canvas.toBlob(ok, "image/jpeg", 0.92),
          );
          canvas.width = canvas.height = 0;
          p.cleanup();
          if (blob) made.push(URL.createObjectURL(blob));
          setBusy({ done: made.length, total: chosen.length });
        }
      } finally {
        void task.destroy();
      }
      setImages(made);
      // Let the pictures load before the dialog takes its snapshot.
      await new Promise((r) => setTimeout(r, 300));
      await printWindow();
      onClose();
      // Kept a while: some systems read the pages after the dialog closes.
      setTimeout(() => setImages([]), 60_000);
    } catch (e) {
      for (const u of made) URL.revokeObjectURL(u);
      const msg = e instanceof Error ? e.message : String(e);
      if (msg !== "cancelled") toast.error("Could not print", { description: msg });
    } finally {
      setBusy(null);
    }
  };

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(o) => {
          if (o) return;
          stop.current = true;
          onClose();
        }}
        title="Print"
        description={title}
        className="w-[420px]"
      >
        <fieldset className="flex flex-col gap-2" disabled={!!busy}>
          <legend className="sr-only">Pages</legend>
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="print-which"
              checked={which === "all"}
              onChange={() => setWhich("all")}
            />
            All pages ({pages})
          </label>
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="print-which"
              checked={which === "current"}
              onChange={() => setWhich("current")}
            />
            This page ({page})
          </label>
          <label className="flex items-center gap-2">
            <input
              type="radio"
              name="print-which"
              checked={which === "range"}
              onChange={() => setWhich("range")}
            />
            Pages
            <Input
              aria-label="Pages to print"
              placeholder={`e.g. 1-5, 8, 12-${pages}`}
              value={range}
              onChange={(e) => {
                setRange(e.target.value);
                setWhich("range");
              }}
              className="h-8 flex-1"
            />
          </label>
          {which === "range" && range.trim() && !chosen && (
            <p className="text-[12px] text-destructive">
              Type pages between 1 and {pages}, like 1-5, 8.
            </p>
          )}
        </fieldset>
        <p className="text-[12px] text-muted-foreground">
          The system's print dialog opens next: choose the printer, copies and paper there, or save
          as PDF. Your highlights are not printed.
        </p>
        {busy && (
          <div className="flex flex-col gap-1.5" role="status">
            <div className="h-1.5 overflow-hidden rounded-full bg-muted">
              <div
                className="h-full bg-foreground/70 transition-[width]"
                style={{ width: `${(busy.done / busy.total) * 100}%` }}
              />
            </div>
            <span className="text-[12px] text-muted-foreground tabular-nums">
              Preparing page {Math.min(busy.done + 1, busy.total)} of {busy.total}…
            </span>
          </div>
        )}
        <div className="flex justify-end gap-2">
          <Button
            variant="ghost"
            onClick={() => {
              stop.current = true;
              if (!busy) onClose();
            }}
          >
            {busy ? "Stop" : "Cancel"}
          </Button>
          <Button disabled={!!busy || !chosen?.length} onClick={() => void run()}>
            {busy ? <Loader2 className="animate-spin" /> : <Printer />} Print
          </Button>
        </div>
      </Dialog>
      {images.length > 0 &&
        createPortal(
          <div className="lb-print-pages" aria-hidden>
            {images.map((src, i) => (
              <img key={i} src={src} alt="" />
            ))}
          </div>,
          document.body,
        )}
    </>
  );
}
