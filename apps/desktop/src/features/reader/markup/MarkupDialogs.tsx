import { useEffect, useRef, useState } from "react";
import { Trash2 } from "lucide-react";
import { getStroke } from "perfect-freehand";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Textarea } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { markupModel, type Mark, type Pt, type Scale, type Unit } from "@/readers";
import { Floating } from "../components/Popovers";
import { pickPicture } from "./pickPicture";

/** The text of a sticky note, next to it. */
export function NotePopover({
  mark,
  rect,
  onSave,
  onDelete,
  onClose,
}: {
  mark: Mark;
  rect: DOMRect;
  onSave: (text: string) => void;
  onDelete: () => void;
  onClose: () => void;
}) {
  const [text, setText] = useState(mark.note ?? "");
  return (
    <Floating rect={rect} onClose={() => (text !== (mark.note ?? "") ? onSave(text) : onClose())}>
      <div className="flex w-72 flex-col gap-2 p-1">
        <Textarea
          autoFocus
          value={text}
          onChange={(e) => setText(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) onSave(text);
          }}
          placeholder="Write a note…"
          aria-label="Note"
          className="min-h-24 text-[13px]"
        />
        <div className="flex items-center gap-2">
          <Button variant="ghost" size="icon" aria-label="Delete the note" onClick={onDelete}>
            <Trash2 />
          </Button>
          <div className="flex-1" />
          <Button size="sm" onClick={() => onSave(text)}>
            Done
          </Button>
        </div>
      </div>
    </Floating>
  );
}

const UNITS: Unit[] = ["mm", "cm", "m", "in", "ft"];

/** "This line is 2.5 m long": sets the book's scale. */
export function CalibrateDialog({
  calibration,
  onDone,
}: {
  calibration: { from: Pt; to: Pt; page: number; size: [number, number] } | null;
  onDone: (scale: Scale | null) => void;
}) {
  const [length, setLength] = useState("");
  const [unit, setUnit] = useState<Unit>("m");
  const value = Number(length.replace(",", "."));
  return (
    <Dialog
      open={calibration !== null}
      onOpenChange={(o) => !o && onDone(null)}
      title="Calibrate"
      description="How long is the line you drew, in real life? Measurements on this book will use that scale."
    >
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          if (!calibration || !(value > 0)) return;
          onDone(
            markupModel.calibrate(calibration.from, calibration.to, calibration.size, value, unit),
          );
        }}
      >
        <div className="flex gap-2">
          <input
            autoFocus
            inputMode="decimal"
            value={length}
            onChange={(e) => setLength(e.target.value)}
            aria-label="Length"
            placeholder="2.5"
            className="h-9 flex-1 rounded-md border border-input bg-background px-2.5 outline-none focus-visible:border-ring"
          />
          <select
            value={unit}
            onChange={(e) => setUnit(e.target.value as Unit)}
            aria-label="Unit"
            className="h-9 rounded-md border bg-background px-2"
          >
            {UNITS.map((u) => (
              <option key={u} value={u}>
                {u}
              </option>
            ))}
          </select>
        </div>
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={() => onDone(null)}>
            Cancel
          </Button>
          <Button type="submit" disabled={!(value > 0)}>
            Set the scale
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

/** Crops a canvas to what was drawn, as a PNG data URL with its shape. */
function trimmed(canvas: HTMLCanvasElement): { src: string; aspect: number } | null {
  const ctx = canvas.getContext("2d")!;
  const { width, height } = canvas;
  const data = ctx.getImageData(0, 0, width, height).data;
  let x0 = width;
  let y0 = height;
  let x1 = -1;
  let y1 = -1;
  for (let y = 0; y < height; y++)
    for (let x = 0; x < width; x++)
      if (data[(y * width + x) * 4 + 3]! > 8) {
        x0 = Math.min(x0, x);
        y0 = Math.min(y0, y);
        x1 = Math.max(x1, x);
        y1 = Math.max(y1, y);
      }
  if (x1 < 0) return null;
  const pad = 6;
  x0 = Math.max(0, x0 - pad);
  y0 = Math.max(0, y0 - pad);
  x1 = Math.min(width - 1, x1 + pad);
  y1 = Math.min(height - 1, y1 + pad);
  const out = document.createElement("canvas");
  out.width = x1 - x0 + 1;
  out.height = y1 - y0 + 1;
  out
    .getContext("2d")!
    .drawImage(canvas, x0, y0, out.width, out.height, 0, 0, out.width, out.height);
  return { src: out.toDataURL("image/png"), aspect: out.width / out.height };
}

/** Draw, type or pick a signature; saved to your profile. */
export function SignatureDialog({
  open,
  onClose,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  onSaved: (sig: { src: string; aspect: number }) => void;
}) {
  const [mode, setMode] = useState<"draw" | "type">("draw");
  const [name, setName] = useState("");
  const canvas = useRef<HTMLCanvasElement>(null);
  const strokes = useRef<[number, number, number][][]>([]);
  const [drawn, setDrawn] = useState(false);

  const paint = () => {
    const c = canvas.current;
    if (!c) return;
    const ctx = c.getContext("2d")!;
    ctx.clearRect(0, 0, c.width, c.height);
    ctx.fillStyle = "#111827";
    if (mode === "type") {
      ctx.font = `64px "Segoe Script", "Brush Script MT", "Bradley Hand", "Comic Sans MS", cursive`;
      ctx.textBaseline = "middle";
      ctx.fillText(name, 20, c.height / 2);
      return;
    }
    for (const s of strokes.current) {
      const outline = getStroke(s, { size: 5, thinning: 0.6, smoothing: 0.5, streamline: 0.5 });
      if (!outline.length) continue;
      ctx.beginPath();
      ctx.moveTo(outline[0]![0]!, outline[0]![1]!);
      for (const [x, y] of outline) ctx.lineTo(x!, y!);
      ctx.closePath();
      ctx.fill();
    }
  };
  useEffect(paint);

  const clear = () => {
    strokes.current = [];
    setDrawn(false);
    setName("");
    paint();
  };

  const point = (e: React.PointerEvent<HTMLCanvasElement>): [number, number, number] => {
    const r = e.currentTarget.getBoundingClientRect();
    const k = e.currentTarget.width / r.width;
    return [
      (e.clientX - r.left) * k,
      (e.clientY - r.top) * k,
      e.pointerType === "pen" ? e.pressure : 0.5,
    ];
  };

  const save = () => {
    const c = canvas.current;
    const sig = c ? trimmed(c) : null;
    if (!sig) return;
    onSaved(sig);
    clear();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="New signature"
      className="w-[560px]"
    >
      <div className="flex gap-1" role="tablist">
        {(["draw", "type"] as const).map((id) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={mode === id}
            onClick={() => {
              setMode(id);
              strokes.current = [];
              setDrawn(false);
            }}
            className={cn(
              "h-7 rounded-md px-3 text-[12.5px]",
              mode === id ? "bg-muted font-medium" : "text-muted-foreground hover:text-foreground",
            )}
          >
            {id === "draw" ? "Draw" : "Type"}
          </button>
        ))}
        <button
          type="button"
          className="h-7 rounded-md px-3 text-[12.5px] text-muted-foreground hover:text-foreground"
          onClick={() =>
            void pickPicture().then((p) => {
              if (p) onSaved(p);
            })
          }
        >
          Use a picture…
        </button>
      </div>
      {mode === "type" && (
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="Your name"
          aria-label="Your name"
          className="h-9 rounded-md border border-input bg-background px-2.5 outline-none focus-visible:border-ring"
        />
      )}
      <canvas
        ref={canvas}
        width={1000}
        height={300}
        aria-label="Signature pad"
        className="w-full touch-none rounded-lg border bg-white"
        onPointerDown={(e) => {
          if (mode !== "draw") return;
          e.currentTarget.setPointerCapture(e.pointerId);
          strokes.current.push([point(e)]);
          setDrawn(true);
        }}
        onPointerMove={(e) => {
          if (mode !== "draw" || !e.buttons) return;
          strokes.current[strokes.current.length - 1]?.push(point(e));
          paint();
        }}
      />
      <p className="text-[12px] text-muted-foreground">
        {mode === "draw"
          ? "Sign with the mouse, a trackpad or a pen."
          : "Your name in a handwriting font."}{" "}
        Signatures are kept in your profile only.
      </p>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={clear}>
          Clear
        </Button>
        <Button onClick={save} disabled={mode === "draw" ? !drawn : !name.trim()}>
          Save signature
        </Button>
      </div>
    </Dialog>
  );
}

/** A stamp with your own words. */
export function NewStampDialog({
  open,
  onClose,
  onSaved,
}: {
  open: boolean;
  onClose: () => void;
  onSaved: (text: string) => void;
}) {
  const [text, setText] = useState("");
  return (
    <Dialog open={open} onOpenChange={(o) => !o && onClose()} title="New stamp">
      <form
        className="flex flex-col gap-4"
        onSubmit={(e) => {
          e.preventDefault();
          const t = text.trim().toUpperCase().slice(0, 30);
          if (t) {
            onSaved(t);
            setText("");
          }
        }}
      >
        <input
          autoFocus
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="CHECKED"
          aria-label="Stamp text"
          maxLength={30}
          className="h-9 rounded-md border border-input bg-background px-2.5 uppercase outline-none focus-visible:border-ring"
        />
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={!text.trim()}>
            Add stamp
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

/** Where to save the marked-up copy, and whether to add it to the library. */
export function ExportMarkupDialog({
  open,
  count,
  canAdd,
  canSaveInto,
  busy,
  onClose,
  onExport,
  onSaveInto,
}: {
  open: boolean;
  count: number;
  canAdd: boolean;
  /** PDFs: the markup can be written into the book as PDF annotations. */
  canSaveInto: boolean;
  busy: boolean;
  onClose: () => void;
  onExport: (addToLibrary: boolean) => void;
  onSaveInto: () => void;
}) {
  const [add, setAdd] = useState(false);
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Export marked-up copy"
      description="Makes a new PDF with your markup drawn on the pages, for sharing or printing. The book itself stays as it is."
    >
      <p className="text-[13px]">
        {count === 1 ? "1 mark" : `${count} marks`} on the shown layers will be drawn in. Sticky
        notes appear as note icons; their text stays in Libreri.
      </p>
      {canAdd && (
        <label className="flex items-center gap-2 text-[13px]">
          <input type="checkbox" checked={add} onChange={(e) => setAdd(e.target.checked)} />
          Also add the copy to the library, next to the book
        </label>
      )}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button onClick={() => onExport(add)} disabled={busy || count === 0}>
          {busy ? "Exporting…" : "Choose where to save…"}
        </Button>
      </div>
      {canSaveInto && (
        <div className="flex flex-col gap-2 border-t pt-3">
          <p className="text-[13px]">
            <span className="font-medium">Or save the markup into this PDF.</span>{" "}
            <span className="text-muted-foreground">
              The marks become standard PDF annotations that other PDF apps show and can change, and
              leave Libreri’s markup list. The file as it is now is kept in Version history.
            </span>
          </p>
          <Button
            variant="outline"
            className="self-end"
            onClick={onSaveInto}
            disabled={busy || count === 0}
          >
            Save into the PDF
          </Button>
        </div>
      )}
    </Dialog>
  );
}
