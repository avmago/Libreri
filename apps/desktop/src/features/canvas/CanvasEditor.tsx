import { Suspense, lazy, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Crop, Loader2, Type } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import type { PageClip } from "@/readers";
import {
  canvasesKey,
  inkToText,
  readCanvas,
  setCanvasPaper,
  writeCanvas,
  PAPERS,
  type Paper,
} from "./api";
import type { CanvasHandle } from "./ExcalidrawHost";
import { ErrorBoundary } from "@/components/ErrorBoundary";

const Host = lazy(() => import("./ExcalidrawHost"));

const selectClass =
  "h-7 rounded-md border border-input bg-background px-1.5 text-[12.5px] outline-none focus-visible:border-ring";

/**
 * One canvas: Excalidraw on the chosen paper, saved as the canvas file as
 * you draw. In the reader it can clip figures from the page.
 */
export function CanvasEditor({
  relPath,
  paper: initialPaper,
  dark,
  lang,
  onLink,
  clip,
}: {
  relPath: string;
  paper: Paper;
  dark: boolean;
  /** The book's language, for reading handwriting. */
  lang?: string | null;
  onLink: (href: string) => void;
  /** Lets the person draw a box on the page; resolves to the clip. */
  clip?: () => Promise<{ clip: PageClip; link: string | null; label: string } | null>;
}) {
  const qc = useQueryClient();
  const { data: content, error } = useQuery({
    queryKey: ["canvas", relPath],
    queryFn: () => readCanvas(relPath),
    staleTime: Infinity,
    gcTime: 0,
  });
  const [paper, setPaper] = useState<Paper>(initialPaper);
  const [reading, setReading] = useState(false);
  const [status, setStatus] = useState<"saved" | "saving" | "error">("saved");
  const handle = useRef<CanvasHandle | null>(null);

  const save = (json: string) => {
    setStatus("saving");
    writeCanvas(relPath, json)
      .then(() => {
        setStatus("saved");
        void qc.invalidateQueries({ queryKey: canvasesKey });
      })
      .catch((e: unknown) => {
        setStatus("error");
        toast.error("The canvas could not be saved", { description: String(e) });
      });
  };

  const readInk = async () => {
    const h = handle.current;
    if (!h) return;
    const ink = await h.selectedInk();
    if (!ink) {
      toast("Select handwriting first", {
        description: "Drag a box around it with the selection tool, then try again.",
      });
      return;
    }
    setReading(true);
    try {
      const text = (await inkToText(ink.png, lang ?? null)).trim();
      if (!text) toast("No writing could be read there");
      else {
        await h.addText(text, ink.below);
        toast.success("Written as text below the handwriting");
      }
    } catch (e) {
      toast.error("The handwriting could not be read", { description: String(e) });
    } finally {
      setReading(false);
    }
  };

  const clipNow = async () => {
    if (!clip) return;
    const got = await clip();
    if (got) await handle.current?.addClip(got.clip, got.link, got.label);
  };

  if (error) return <p className="p-4 text-destructive">{String(error)}</p>;
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-9 shrink-0 items-center gap-1.5 border-b px-2 text-[12.5px]">
        <select
          className={selectClass}
          value={paper}
          aria-label="Paper"
          onChange={(e) => {
            const p = e.target.value as Paper;
            setPaper(p);
            void setCanvasPaper(relPath, p).then(() =>
              qc.invalidateQueries({ queryKey: canvasesKey }),
            );
          }}
        >
          {PAPERS.map((p) => (
            <option key={p.id} value={p.id}>
              {p.label} paper
            </option>
          ))}
        </select>
        {clip && (
          <Button
            variant="ghost"
            size="sm"
            onClick={() => void clipNow()}
            title="Draw a box on the page to copy that part here, linked back to its page"
          >
            <Crop /> Clip from page
          </Button>
        )}
        <Button
          variant="ghost"
          size="sm"
          onClick={() => void readInk()}
          disabled={reading}
          title="Turn the selected handwriting into typed text"
        >
          {reading ? <Loader2 className="animate-spin" /> : <Type />} Ink to text
        </Button>
        <span className="ml-auto text-[11px] text-muted-foreground" aria-live="polite">
          {status === "saving" ? "Saving…" : status === "error" ? "Not saved" : "Saved"}
        </span>
      </div>
      <div className="relative min-h-0 flex-1">
        {content === undefined ? (
          <p className="p-4 text-muted-foreground">Opening…</p>
        ) : (
          <ErrorBoundary
            fallback={(e) => (
              <div className="flex flex-col gap-2 p-4 text-[13px]">
                <p className="font-medium">The canvas editor could not start.</p>
                <p className="text-muted-foreground">
                  The rest of Libreri keeps working, and the canvas file is unchanged. If you run
                  Libreri from its source code, run <code>pnpm install</code> and start it again.
                </p>
                <p className="font-mono text-[11.5px] break-all text-muted-foreground">
                  {e.message}
                </p>
              </div>
            )}
          >
            <Suspense
              fallback={
                <p className="flex items-center gap-2 p-4 text-muted-foreground">
                  <Loader2 className="size-4 animate-spin" /> Getting the pens ready…
                </p>
              }
            >
              <Host
                key={relPath}
                content={content}
                paper={paper}
                dark={dark}
                onSave={save}
                onLink={onLink}
                onReady={(h) => (handle.current = h)}
              />
            </Suspense>
          </ErrorBoundary>
        )}
      </div>
    </div>
  );
}
