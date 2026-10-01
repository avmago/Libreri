import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { Skeleton } from "@/components/ui/skeleton";
import {
  ArrowLeft,
  ArrowRight,
  Camera,
  ImagePlus,
  Loader2,
  Maximize,
  RotateCcw,
  RotateCw,
  Smartphone,
  Trash2,
} from "lucide-react";
import { open as openFiles } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { commands, unwrap, type CapturePhoto, type CaptureSaved, type Clean } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { CameraPagesDialog, PhonePagesDialog } from "../edit/EditDialogs";

type Corner = [number, number];

interface Page {
  photo: CapturePhoto;
  corners: Corner[];
  clean: Clean;
  turns: number;
  /** The page as it will be saved (small). */
  preview: string | null;
}

const CLEANS: { id: Clean; label: string }[] = [
  { id: "colour", label: "Colour" },
  { id: "grey", label: "Grey" },
  { id: "blackWhite", label: "Black and white" },
  { id: "none", label: "As photographed" },
];

const WHOLE: Corner[] = [
  [0, 0],
  [1, 0],
  [1, 1],
  [0, 1],
];

const spec = (p: Page) => ({ id: p.photo.id, corners: p.corners, clean: p.clean, turns: p.turns });

/**
 * Capturing paper notes (Phase 8b): photos from the camera, a phone or
 * picture files; each page's corners can be moved, and it is straightened
 * and cleaned. Saved as one searchable PDF in the notes folder.
 */
export function CaptureDialog({
  open,
  defaultTitle,
  onClose,
  onSaved,
}: {
  open: boolean;
  defaultTitle: string;
  onClose: () => void;
  onSaved: (saved: CaptureSaved, title: string) => void;
}) {
  const [pages, setPages] = useState<Page[]>([]);
  const [at, setAt] = useState(0);
  const [adding, setAdding] = useState(0);
  const [camera, setCamera] = useState(false);
  const [phone, setPhone] = useState(false);
  const [title, setTitle] = useState(defaultTitle);
  const [readText, setReadText] = useState(true);
  const [saving, setSaving] = useState(false);
  const current = pages[at] ?? null;
  const pagesRef = useRef(pages);
  useEffect(() => {
    pagesRef.current = pages;
  });

  const update = (i: number, change: Partial<Page>) =>
    setPages((list) => list.map((p, k) => (k === i ? { ...p, ...change, preview: null } : p)));

  // The saved-page preview follows the settings, a moment after a change.
  useEffect(() => {
    const p = current;
    if (!p || p.preview) return;
    const id = p.photo.id;
    const t = setTimeout(() => {
      unwrap(commands.capturePreview(spec(p)))
        .then((preview) =>
          setPages((list) =>
            list.map((x) =>
              x.photo.id === id &&
              x.corners === p.corners &&
              x.clean === p.clean &&
              x.turns === p.turns
                ? { ...x, preview }
                : x,
            ),
          ),
        )
        .catch(() => {});
    }, 250);
    return () => clearTimeout(t);
  }, [current]);

  const added = (photo: CapturePhoto) =>
    setPages((list) => {
      setAt(list.length);
      return [
        ...list,
        {
          photo,
          corners: photo.corners.map((c) => [c[0] ?? 0, c[1] ?? 0] as Corner),
          clean: "colour",
          turns: 0,
          preview: null,
        },
      ];
    });

  const addPhoto = (src: string) => {
    setAdding((n) => n + 1);
    unwrap(commands.captureAdd(src))
      .then(added)
      .catch((e: Error) => toast.error("That photo could not be added", { description: e.message }))
      .finally(() => setAdding((n) => n - 1));
  };

  const addFiles = async () => {
    const chosen = await openFiles({
      multiple: true,
      filters: [{ name: "Pictures", extensions: ["jpg", "jpeg", "png", "webp", "heic", "heif"] }],
    });
    const list = Array.isArray(chosen) ? chosen : chosen ? [chosen] : [];
    for (const path of list) {
      setAdding((n) => n + 1);
      try {
        added(await unwrap(commands.captureAddFile(path)));
      } catch (e) {
        toast.error("That picture could not be added", { description: String(e) });
      } finally {
        setAdding((n) => n - 1);
      }
    }
  };

  const close = () => {
    const ids = pagesRef.current.map((p) => p.photo.id);
    if (ids.length) void commands.captureDiscard(ids);
    setPages([]);
    setAt(0);
    onClose();
  };

  const save = async () => {
    if (!pages.length) return;
    setSaving(true);
    try {
      const saved = await unwrap(
        commands.captureSave(pages.map(spec), title.trim() || defaultTitle, readText),
      );
      setPages([]);
      setAt(0);
      onSaved(saved, title.trim() || defaultTitle);
    } catch (e) {
      toast.error("The pages could not be saved", { description: String(e) });
    } finally {
      setSaving(false);
    }
  };

  const move = (d: -1 | 1) => {
    const j = at + d;
    if (j < 0 || j >= pages.length) return;
    setPages((list) => {
      const next = [...list];
      [next[at], next[j]] = [next[j]!, next[at]!];
      return next;
    });
    setAt(j);
  };

  return (
    <>
      <Dialog
        open={open}
        onOpenChange={(o) => !o && !saving && close()}
        title="Capture paper notes"
        description="Photograph handwritten or printed pages. Each page is found, straightened and cleaned, and all of them are saved as one PDF in your notes, linked to this place in the book."
        className="w-[min(1100px,calc(100vw-48px))] max-h-[92vh]"
      >
        <div className="flex flex-wrap items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => setCamera(true)}>
            <Camera /> Camera
          </Button>
          <Button variant="outline" size="sm" onClick={() => setPhone(true)}>
            <Smartphone /> Phone
          </Button>
          <Button variant="outline" size="sm" onClick={() => void addFiles()}>
            <ImagePlus /> Pictures…
          </Button>
          {adding > 0 && (
            <span className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground">
              <Loader2 className="size-3.5 animate-spin" /> Finding the page…
            </span>
          )}
        </div>

        {current ? (
          <div className="grid shrink-0 grid-cols-[1fr_minmax(0,340px)] gap-4">
            <CornerEditor
              key={current.photo.id}
              src={current.photo.preview}
              ratio={current.photo.width / Math.max(1, current.photo.height)}
              corners={current.corners}
              onChange={(corners) => update(at, { corners })}
            />
            <div className="flex min-w-0 flex-col gap-3">
              <div className="flex h-[40vh] items-center justify-center overflow-hidden rounded-lg border bg-muted/40">
                {current.preview ? (
                  <img
                    src={current.preview}
                    alt={`Page ${at + 1} as it will be saved`}
                    className="max-h-full max-w-full object-contain shadow"
                  />
                ) : (
                  <Skeleton
                    className="aspect-[1/1.35] h-full max-h-full"
                    aria-label="Preparing the page"
                  />
                )}
              </div>
              <div className="flex flex-wrap items-center gap-1.5">
                <select
                  aria-label="Clean up"
                  className="h-8 rounded-md border border-input bg-background px-2 text-[13px] outline-none focus-visible:border-ring"
                  value={current.clean}
                  onChange={(e) => update(at, { clean: e.target.value as Clean })}
                >
                  {CLEANS.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.label}
                    </option>
                  ))}
                </select>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Turn left"
                  onClick={() => update(at, { turns: current.turns + 3 })}
                >
                  <RotateCcw />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Turn right"
                  onClick={() => update(at, { turns: current.turns + 1 })}
                >
                  <RotateCw />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Use the whole photo"
                  title="Use the whole photo"
                  onClick={() => update(at, { corners: WHOLE })}
                >
                  <Maximize />
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Remove this page"
                  className="ml-auto text-destructive"
                  onClick={() => {
                    void commands.captureDiscard([current.photo.id]);
                    setPages((list) => list.filter((_, k) => k !== at));
                    setAt((i) => Math.max(0, Math.min(i, pages.length - 2)));
                  }}
                >
                  <Trash2 />
                </Button>
              </div>
              <p className="text-[12px] text-muted-foreground">
                Drag the corners to the edges of the page if they are not quite right.
              </p>
            </div>
          </div>
        ) : (
          <div className="flex h-72 flex-col items-center justify-center gap-2 rounded-lg border border-dashed text-center text-muted-foreground">
            <Camera className="size-6" aria-hidden />
            <p>Add pages with the camera, your phone, or pictures on this computer.</p>
            <p className="text-[12px]">Lay each page flat, in good light, on a darker surface.</p>
          </div>
        )}

        {pages.length > 0 && (
          <div className="flex shrink-0 items-center gap-2 overflow-x-auto pb-1">
            {pages.map((p, i) => (
              <button
                key={p.photo.id}
                type="button"
                onClick={() => setAt(i)}
                aria-label={`Page ${i + 1}`}
                aria-current={i === at ? "true" : undefined}
                className={cn(
                  "relative h-20 w-16 shrink-0 overflow-hidden rounded border bg-muted",
                  i === at && "ring-2 ring-primary",
                )}
              >
                <img src={p.preview ?? p.photo.preview} alt="" className="size-full object-cover" />
                <span className="absolute right-0.5 bottom-0.5 rounded bg-background/90 px-1 text-[10px]">
                  {i + 1}
                </span>
              </button>
            ))}
            <div className="ml-1 flex gap-0.5">
              <Button
                variant="ghost"
                size="icon"
                aria-label="Move page earlier"
                onClick={() => move(-1)}
                disabled={at === 0}
              >
                <ArrowLeft />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Move page later"
                onClick={() => move(1)}
                disabled={at >= pages.length - 1}
              >
                <ArrowRight />
              </Button>
            </div>
          </div>
        )}

        <div className="flex flex-wrap items-center gap-3 border-t pt-3">
          <label className="flex min-w-60 flex-1 items-center gap-2">
            <span className="shrink-0">Name</span>
            <Input value={title} onChange={(e) => setTitle(e.target.value)} />
          </label>
          <label className="flex items-center gap-1.5 text-[12.5px]">
            <input
              type="checkbox"
              checked={readText}
              onChange={(e) => setReadText(e.target.checked)}
            />
            Read the text so it can be searched
          </label>
          <Button variant="ghost" onClick={close} disabled={saving}>
            Cancel
          </Button>
          <Button onClick={() => void save()} disabled={!pages.length || saving || adding > 0}>
            {saving ? <Loader2 className="animate-spin" /> : null}
            {saving
              ? readText
                ? "Reading and saving…"
                : "Saving…"
              : `Save ${pages.length || ""} ${pages.length === 1 ? "page" : "pages"}`}
          </Button>
        </div>
      </Dialog>
      <CameraPagesDialog open={camera} onClose={() => setCamera(false)} onPhoto={addPhoto} />
      <PhonePagesDialog open={phone} onClose={() => setPhone(false)} onPhoto={addPhoto} />
    </>
  );
}

/** The photo with the page's four corners, which can be dragged. */
function CornerEditor({
  src,
  ratio,
  corners,
  onChange,
}: {
  src: string;
  ratio: number;
  corners: Corner[];
  onChange: (c: Corner[]) => void;
}) {
  const box = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<number | null>(null);
  const [live, setLive] = useState<Corner[] | null>(null);
  const shown = live ?? corners;

  const at = (e: ReactPointerEvent): Corner => {
    const r = box.current!.getBoundingClientRect();
    return [
      Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)),
      Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)),
    ];
  };
  const points = shown.map(([x, y]) => `${x * 100},${y * 100}`).join(" ");

  return (
    <div className="flex min-h-0 items-center justify-center rounded-lg bg-black/80 p-3">
      <div
        ref={box}
        className="relative max-h-[50vh] w-full touch-none select-none"
        style={{ aspectRatio: String(ratio), maxWidth: `calc(50vh * ${ratio})` }}
        onPointerMove={(e) => {
          if (drag === null) return;
          const next = shown.map((c, i) => (i === drag ? at(e) : c));
          setLive(next);
        }}
        onPointerUp={() => {
          if (drag !== null && live) onChange(live);
          setDrag(null);
          setLive(null);
        }}
      >
        <img src={src} alt="The photo" className="size-full" draggable={false} />
        <svg
          viewBox="0 0 100 100"
          preserveAspectRatio="none"
          className="pointer-events-none absolute inset-0 size-full"
          aria-hidden
        >
          <polygon
            points={points}
            fill="rgba(37,99,235,0.12)"
            stroke="#2563eb"
            strokeWidth={0.5}
            vectorEffect="non-scaling-stroke"
          />
        </svg>
        {shown.map(([x, y], i) => (
          <button
            key={i}
            type="button"
            aria-label={
              ["Top-left corner", "Top-right corner", "Bottom-right corner", "Bottom-left corner"][
                i
              ]
            }
            className="absolute size-5 -translate-x-1/2 -translate-y-1/2 cursor-grab rounded-full border-2 border-white bg-blue-600 shadow active:cursor-grabbing"
            style={{ left: `${x * 100}%`, top: `${y * 100}%` }}
            onPointerDown={(e) => {
              e.currentTarget.parentElement?.setPointerCapture(e.pointerId);
              setDrag(i);
            }}
            onKeyDown={(e) => {
              const step = e.shiftKey ? 0.02 : 0.005;
              const d: Record<string, Corner> = {
                ArrowLeft: [-step, 0],
                ArrowRight: [step, 0],
                ArrowUp: [0, -step],
                ArrowDown: [0, step],
              };
              const m = d[e.key];
              if (!m) return;
              e.preventDefault();
              onChange(
                corners.map((c, k) =>
                  k === i
                    ? [Math.min(1, Math.max(0, c[0] + m[0])), Math.min(1, Math.max(0, c[1] + m[1]))]
                    : c,
                ),
              );
            }}
          />
        ))}
      </div>
    </div>
  );
}
