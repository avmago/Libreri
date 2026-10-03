import { ResizablePanel } from "@/components/ResizablePanel";
import { useState } from "react";
import { Maximize2, Minimize2, PenLine, Plus, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import type { PageClip } from "@/readers";
import { cn } from "@/lib/utils";
import { PAPERS, useCanvases, useCreateCanvas, useDeleteCanvas, type Paper } from "./api";
import { CanvasEditor } from "./CanvasEditor";

const selectClass =
  "h-7 min-w-0 rounded-md border border-input bg-background px-1.5 text-[12.5px] outline-none focus-visible:border-ring";

/**
 * The reader's canvas panel: this book's handwriting canvases,
 * drawn beside the page, with figures clipped from it.
 */
export function CanvasPanel({
  bookId,
  dark,
  lang,
  onLink,
  clip,
  onClose,
}: {
  bookId: string;
  dark: boolean;
  lang?: string | null;
  onLink: (href: string) => void;
  clip?: () => Promise<{ clip: PageClip; link: string | null; label: string } | null>;
  onClose: () => void;
}) {
  const { data: list = [], isPending } = useCanvases(bookId);
  const create = useCreateCanvas();
  const remove = useDeleteCanvas();
  const [chosen, setChosen] = useState<string | null>(null);
  const [paper, setPaper] = useState<Paper>("plain");
  const [wide, setWide] = useState(false);
  const current = list.find((c) => c.relPath === chosen) ?? list[0] ?? null;

  const start = () =>
    create.mutate(
      { title: "", book: bookId, paper },
      {
        onSuccess: (rel) => setChosen(rel),
        onError: (e) => toast.error("Could not start a canvas", { description: e.message }),
      },
    );

  return (
    <ResizablePanel
      key={wide ? "wide" : "narrow"}
      id={wide ? "canvas.wide" : "canvas"}
      initial={wide ? 1024 : 576}
      min={320}
      max={1600}
      side="right"
      label="canvas"
      className="max-w-[75%]"
    >
      <aside aria-label="Canvas" className="flex min-w-0 flex-1 flex-col border-l bg-sidebar">
        <div className="flex h-10 shrink-0 items-center gap-1 border-b pr-2 pl-3">
          <PenLine className="size-4 shrink-0 text-muted-foreground" aria-hidden />
          {current ? (
            <select
              className={cn(selectClass, "flex-1")}
              aria-label="Canvas"
              value={current.relPath}
              onChange={(e) => setChosen(e.target.value)}
            >
              {list.map((c) => (
                <option key={c.relPath} value={c.relPath}>
                  {c.title}
                </option>
              ))}
            </select>
          ) : (
            <span className="flex-1 text-[12.5px] font-medium">Canvas</span>
          )}
          {current && (
            <>
              <Button
                variant="ghost"
                size="icon"
                className="size-7"
                aria-label="New canvas"
                title="New canvas"
                onClick={start}
                disabled={create.isPending}
              >
                <Plus />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="size-7"
                aria-label="Delete this canvas"
                title="Delete this canvas (it goes to the trash)"
                onClick={() =>
                  remove.mutate(current.relPath, {
                    onSuccess: () => {
                      setChosen(null);
                      toast(`“${current.title}” was moved to the trash`);
                    },
                    onError: (e) => toast.error(e.message),
                  })
                }
              >
                <Trash2 />
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label={wide ? "Narrower" : "Wider"}
            aria-pressed={wide}
            onClick={() => setWide((w) => !w)}
          >
            {wide ? <Minimize2 /> : <Maximize2 />}
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label="Close canvas"
            onClick={onClose}
          >
            <X />
          </Button>
        </div>
        {current ? (
          <CanvasEditor
            key={current.relPath}
            relPath={current.relPath}
            paper={current.paper as Paper}
            dark={dark}
            lang={lang}
            onLink={onLink}
            clip={clip}
          />
        ) : isPending ? (
          <p className="p-4 text-muted-foreground">Opening…</p>
        ) : (
          <div className="flex flex-col gap-3 p-4">
            <p className="text-muted-foreground">
              Write and draw by hand next to the page, and clip figures from it. Canvases are saved
              in your notes folder as Excalidraw files.
            </p>
            <label className="flex items-center gap-2">
              <span>Paper</span>
              <select
                className={selectClass}
                value={paper}
                onChange={(e) => setPaper(e.target.value as Paper)}
              >
                {PAPERS.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.label}
                  </option>
                ))}
              </select>
            </label>
            <Button className="self-start" onClick={start} disabled={create.isPending}>
              <PenLine /> Start a canvas
            </Button>
          </div>
        )}
      </aside>
    </ResizablePanel>
  );
}
