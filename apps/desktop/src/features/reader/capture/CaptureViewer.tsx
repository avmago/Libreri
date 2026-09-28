import { useEffect, useRef, useState } from "react";
import { ExternalLink, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { bookUrl, commands } from "@/lib/ipc";
import { loadPdfJs } from "@/readers";

/** The captured pages, shown one under another. */
export function CaptureViewer({
  path,
  title,
  onClose,
}: {
  path: string | null;
  title: string;
  onClose: () => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [state, setState] = useState<"loading" | "ready" | "failed">("loading");

  useEffect(() => {
    if (!path) return;
    let live = true;
    let doc: { destroy: () => Promise<void> } | null = null;
    void (async () => {
      try {
        const { pdfjs } = await loadPdfJs();
        const task = pdfjs.getDocument({ url: bookUrl(path) });
        doc = task;
        const pdf = await task.promise;
        const host = box.current;
        if (!live || !host) return;
        host.replaceChildren();
        for (let n = 1; n <= pdf.numPages; n++) {
          const page = await pdf.getPage(n);
          const width = Math.min(760, host.clientWidth || 760);
          const base = page.getViewport({ scale: 1 });
          const viewport = page.getViewport({
            scale: (width / base.width) * (window.devicePixelRatio || 1),
          });
          const canvas = document.createElement("canvas");
          canvas.width = viewport.width;
          canvas.height = viewport.height;
          canvas.style.width = `${width}px`;
          canvas.className = "mx-auto mb-3 rounded border bg-white shadow-sm";
          canvas.setAttribute("aria-label", `Page ${n}`);
          host.append(canvas);
          await page.render({ canvas, viewport }).promise;
          if (!live) return;
        }
        setState("ready");
      } catch {
        if (live) setState("failed");
      }
    })();
    return () => {
      live = false;
      void doc?.destroy();
    };
  }, [path]);

  return (
    <Dialog
      open={!!path}
      onOpenChange={(o) => !o && onClose()}
      title={title}
      description={path ?? undefined}
      className="w-[820px] max-h-[90vh]"
    >
      {state === "loading" && (
        <p className="flex items-center gap-2 text-muted-foreground">
          <Loader2 className="size-4 animate-spin" /> Opening the pages…
        </p>
      )}
      {state === "failed" && <p className="text-destructive">The pages could not be shown.</p>}
      <div ref={box} className="min-h-0 overflow-y-auto" />
      <div className="flex justify-end">
        <Button
          variant="outline"
          size="sm"
          onClick={() =>
            path &&
            void commands.openNoteFile(path).then((r) => {
              if (r.status === "error") toast.error(r.error.message);
            })
          }
        >
          <ExternalLink /> Open in another app
        </Button>
      </div>
    </Dialog>
  );
}
