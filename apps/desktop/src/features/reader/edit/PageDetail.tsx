import { useEffect, useMemo, useRef, useState } from "react";
import { Search, Trash2, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Correction } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import type { Box } from "./model";
import { findBoxes, textUnder, type PageText } from "./pdfPages";

export type DetailMode = "crop" | "redact" | "correct";

const FONTS = [
  { id: "sans", label: "Sans serif", css: "Helvetica, Arial, sans-serif" },
  { id: "serif", label: "Serif", css: "'Times New Roman', Times, serif" },
  { id: "mono", label: "Monospace", css: "'Courier New', Courier, monospace" },
] as const;

const cssFont = (id: string) => FONTS.find((f) => f.id === id)?.css ?? FONTS[0].css;

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

function normalise(a: [number, number], b: [number, number]): Box {
  const x = clamp01(Math.min(a[0], b[0]));
  const y = clamp01(Math.min(a[1], b[1]));
  return [x, y, clamp01(Math.max(a[0], b[0])) - x, clamp01(Math.max(a[1], b[1])) - y];
}

const hex = (r: number, g: number, b: number) =>
  `#${[r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("")}`;

/** The paper colour around a box: the most common colour along its edge. */
function paperAround(img: HTMLImageElement, [x, y, w, h]: Box): string {
  const canvas = document.createElement("canvas");
  canvas.width = img.naturalWidth;
  canvas.height = img.naturalHeight;
  const ctx = canvas.getContext("2d", { willReadFrequently: true });
  if (!ctx || !canvas.width) return "#ffffff";
  ctx.drawImage(img, 0, 0);
  const counts = new Map<string, number>();
  const actual = new Map<string, string>();
  const px = (fx: number, fy: number) => {
    const X = Math.round(clamp01(fx) * (canvas.width - 1));
    const Y = Math.round(clamp01(fy) * (canvas.height - 1));
    const [r, g, b] = ctx.getImageData(X, Y, 1, 1).data;
    // Round so paper grain counts as one colour.
    const key = hex((r! >> 3) << 3, (g! >> 3) << 3, (b! >> 3) << 3);
    counts.set(key, (counts.get(key) ?? 0) + 1);
    if (!actual.has(key)) actual.set(key, hex(r!, g!, b!));
  };
  const m = 0.004;
  for (let i = 0; i <= 12; i++) {
    const t = i / 12;
    px(x + w * t, y - m);
    px(x + w * t, y + h + m);
    px(x - m, y + h * t);
    px(x + w + m, y + h * t);
  }
  const top = [...counts.entries()].sort((a, b) => b[1] - a[1])[0]?.[0];
  return (top && actual.get(top)) ?? "#ffffff";
}

/** The correction drawn as a picture (for letters the PDF fonts lack). */
function correctionPicture(c: Correction, pagePoints: [number, number]): string {
  const [, , bw, bh] = c.rect as [number, number, number, number];
  const scale = 4;
  const w = Math.max(1, Math.round(bw * pagePoints[0] * scale));
  const h = Math.max(1, Math.round(bh * pagePoints[1] * scale));
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = c.background;
  ctx.fillRect(0, 0, w, h);
  const size = (c.size ?? 0.02) * pagePoints[0] * scale;
  ctx.font = `${size}px ${cssFont(c.font)}`;
  ctx.fillStyle = c.color;
  ctx.textBaseline = "top";
  c.text
    .split("\n")
    .forEach((line, i) => ctx.fillText(line, size * 0.1, size * 0.05 + i * size * 1.2));
  return canvas.toDataURL("image/png");
}

/**
 * One page, large, for cropping, blacking out or correcting words. Boxes
 * are fractions of the page as shown here.
 */
export function PageDetail({
  mode,
  picture,
  aspect,
  title,
  pagePoints,
  text,
  book: bookPage,
  initialBoxes,
  initialCorrections,
  selectedCount,
  onFindEverywhere,
  onDone,
  onCancel,
}: {
  mode: DetailMode;
  picture: string;
  /** Height over width as shown. */
  aspect: number;
  title: string;
  pagePoints: [number, number];
  text: PageText | null;
  /** The book page (redact and correct). */
  book: number;
  initialBoxes: Box[];
  initialCorrections: Correction[];
  /** Pages selected in the grid (crop can apply to all of them). */
  selectedCount: number;
  onFindEverywhere?: (query: string) => void;
  onDone: (result: { boxes: Box[]; corrections: Correction[]; allSelected: boolean }) => void;
  onCancel: () => void;
}) {
  const [boxes, setBoxes] = useState<Box[]>(initialBoxes);
  const [fixes, setFixes] = useState<Correction[]>(initialCorrections);
  const [editing, setEditing] = useState<number | null>(null);
  const [drag, setDrag] = useState<{ from: [number, number]; to: [number, number] } | null>(null);
  const [picked, setPicked] = useState<number | null>(null);
  const [query, setQuery] = useState("");
  const [allSelected, setAllSelected] = useState(selectedCount > 1);
  const stage = useRef<HTMLDivElement>(null);
  const img = useRef<HTMLImageElement>(null);
  const [fit, setFit] = useState<{ w: number; h: number }>({ w: 400, h: 400 * aspect });

  // Fit the page into the space.
  useEffect(() => {
    const el = stage.current;
    if (!el) return;
    const measure = () => {
      const W = el.clientWidth - 32;
      const H = el.clientHeight - 32;
      const w = Math.min(W, H / aspect);
      setFit({ w, h: w * aspect });
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [aspect]);

  const at = (e: React.PointerEvent): [number, number] => {
    const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
    return [clamp01((e.clientX - r.left) / r.width), clamp01((e.clientY - r.top) / r.height)];
  };

  const finish = () => {
    if (!drag) return;
    const b = normalise(drag.from, drag.to);
    setDrag(null);
    if (b[2] < 0.004 || b[3] < 0.004) return;
    if (mode === "crop") setBoxes([b]);
    else if (mode === "redact") {
      setBoxes((list) => [...list, b]);
      setPicked(boxes.length);
    } else {
      const height = b[3] * pagePoints[1];
      const size = Math.max(4, Math.min(72, height * 0.72)) / pagePoints[0];
      const found = text ? textUnder(text, b) : "";
      const background = img.current ? paperAround(img.current, b) : "#ffffff";
      setFixes((list) => [
        ...list,
        {
          page: bookPage,
          rect: b,
          text: found,
          size,
          color: "#000000",
          background,
          font: "sans",
          picture: null,
        },
      ]);
      setEditing(fixes.length);
    }
  };

  const shown = drag ? normalise(drag.from, drag.to) : null;
  const crop = mode === "crop" ? (boxes[0] ?? null) : null;
  const editingFix = editing !== null ? fixes[editing] : undefined;
  const updateFix = (patch: Partial<Correction>) =>
    setFixes((list) => list.map((c, i) => (i === editing ? { ...c, ...patch } : c)));

  const findHere = () => {
    if (!text || !query.trim()) return;
    const found = findBoxes(text, query);
    setBoxes((list) => [...list, ...found]);
  };

  const help = useMemo(
    () =>
      ({
        crop: "Drag over the part of the page to keep.",
        redact:
          "Drag over what to black out. Text, pictures and drawings under the boxes are removed from the file, not just covered.",
        correct:
          "Drag over the words to change. They are removed and the new text is written in their place.",
      })[mode],
    [mode],
  );

  const save = () =>
    onDone({
      boxes,
      corrections: fixes
        .filter((c) => c.text.trim())
        .map((c) => ({ ...c, picture: correctionPicture(c, pagePoints) })),
      allSelected,
    });

  return (
    <div className="flex min-h-0 flex-1">
      <div
        ref={stage}
        className="relative flex min-w-0 flex-1 items-center justify-center overflow-hidden bg-muted/60"
      >
        <div
          className={cn("relative shadow-lg select-none", mode !== "crop" && "cursor-crosshair")}
          style={{ width: fit.w, height: fit.h, cursor: "crosshair" }}
          onPointerDown={(e) => {
            if ((e.target as HTMLElement).dataset.box) return;
            (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
            const p = at(e);
            setDrag({ from: p, to: p });
            setPicked(null);
          }}
          onPointerMove={(e) => drag && setDrag({ ...drag, to: at(e) })}
          onPointerUp={finish}
        >
          <img
            ref={img}
            src={picture}
            alt={title}
            draggable={false}
            className="pointer-events-none size-full bg-white"
          />
          {crop && (
            <div
              className="pointer-events-none absolute outline outline-2 outline-primary"
              style={{
                left: `${crop[0] * 100}%`,
                top: `${crop[1] * 100}%`,
                width: `${crop[2] * 100}%`,
                height: `${crop[3] * 100}%`,
                boxShadow: "0 0 0 9999px rgba(0,0,0,0.45)",
              }}
            />
          )}
          {mode === "redact" &&
            boxes.map((b, i) => (
              <div
                key={i}
                data-box="1"
                role="button"
                tabIndex={0}
                aria-label={`Redaction ${i + 1}`}
                onClick={() => setPicked(i)}
                onKeyDown={(e) => {
                  if (e.key === "Delete" || e.key === "Backspace") {
                    setBoxes((list) => list.filter((_, j) => j !== i));
                    setPicked(null);
                  }
                }}
                className={cn(
                  "absolute bg-black/85",
                  picked === i && "outline outline-2 outline-offset-1 outline-primary",
                )}
                style={{
                  left: `${b[0] * 100}%`,
                  top: `${b[1] * 100}%`,
                  width: `${b[2] * 100}%`,
                  height: `${b[3] * 100}%`,
                }}
              />
            ))}
          {mode === "correct" &&
            fixes.map((c, i) => {
              const [x, y, w, h] = c.rect as Box;
              return (
                <div
                  key={i}
                  data-box="1"
                  role="button"
                  tabIndex={0}
                  onClick={() => setEditing(i)}
                  className={cn(
                    "absolute overflow-hidden whitespace-pre",
                    editing === i && "outline outline-2 outline-offset-1 outline-primary",
                  )}
                  style={{
                    left: `${x * 100}%`,
                    top: `${y * 100}%`,
                    width: `${w * 100}%`,
                    height: `${h * 100}%`,
                    background: c.background,
                    color: c.color,
                    fontFamily: cssFont(c.font),
                    fontSize: (c.size ?? 0.02) * fit.w,
                    lineHeight: 1.2,
                    paddingLeft: (c.size ?? 0.02) * fit.w * 0.1,
                  }}
                >
                  {c.text}
                </div>
              );
            })}
          {shown && (
            <div
              className={cn(
                "pointer-events-none absolute border-2 border-dashed",
                mode === "redact" ? "border-black bg-black/40" : "border-primary bg-primary/10",
              )}
              style={{
                left: `${shown[0] * 100}%`,
                top: `${shown[1] * 100}%`,
                width: `${shown[2] * 100}%`,
                height: `${shown[3] * 100}%`,
              }}
            />
          )}
        </div>
      </div>

      <aside className="flex w-72 shrink-0 flex-col gap-3 overflow-auto border-l p-4 text-[13px]">
        <div className="flex items-start justify-between gap-2">
          <div>
            <h3 className="text-[14px] font-semibold">
              {{ crop: "Crop", redact: "Redact", correct: "Correct text" }[mode]}
            </h3>
            <p className="text-muted-foreground">{title}</p>
          </div>
          <Button variant="ghost" size="icon" aria-label="Close" onClick={onCancel}>
            <X />
          </Button>
        </div>
        <p className="text-[12.5px] text-muted-foreground">{help}</p>

        {mode === "crop" && (
          <>
            {crop && (
              <Button variant="outline" size="sm" onClick={() => setBoxes([])}>
                Keep the whole page
              </Button>
            )}
            {selectedCount > 1 && (
              <label className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={allSelected}
                  onChange={(e) => setAllSelected(e.target.checked)}
                />
                The same on all {selectedCount} selected pages
              </label>
            )}
          </>
        )}

        {mode === "redact" && (
          <>
            <form
              className="flex flex-col gap-2"
              onSubmit={(e) => {
                e.preventDefault();
                findHere();
              }}
            >
              <label className="font-medium" htmlFor="lb-redact-find">
                Black out words
              </label>
              <div className="flex gap-1.5">
                <input
                  id="lb-redact-find"
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  placeholder="A name, a number…"
                  className="h-8 min-w-0 flex-1 rounded-md border border-input bg-background px-2 outline-none focus-visible:border-ring"
                />
                <Button type="submit" variant="outline" size="sm" disabled={!text}>
                  <Search /> This page
                </Button>
              </div>
              {onFindEverywhere && (
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  className="self-start"
                  disabled={!query.trim()}
                  onClick={() => onFindEverywhere(query)}
                >
                  On every page…
                </Button>
              )}
              {!text && (
                <p className="text-[12px] text-muted-foreground">
                  This page has no text to search; draw boxes by hand.
                </p>
              )}
            </form>
            <p className="text-muted-foreground">
              {boxes.length === 0
                ? "No boxes yet."
                : `${boxes.length} ${boxes.length === 1 ? "box" : "boxes"} on this page.`}
            </p>
            {picked !== null && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => {
                  setBoxes((list) => list.filter((_, j) => j !== picked));
                  setPicked(null);
                }}
              >
                <Trash2 /> Remove this box
              </Button>
            )}
            {boxes.length > 0 && (
              <Button variant="ghost" size="sm" onClick={() => setBoxes([])}>
                Remove all boxes
              </Button>
            )}
          </>
        )}

        {mode === "correct" && editingFix && (
          <div className="flex flex-col gap-2 rounded-lg border p-3">
            <label className="font-medium" htmlFor="lb-fix-text">
              New text
            </label>
            <textarea
              id="lb-fix-text"
              autoFocus
              rows={3}
              value={editingFix.text}
              onChange={(e) => updateFix({ text: e.target.value })}
              className="rounded-md border border-input bg-background px-2 py-1.5 outline-none focus-visible:border-ring"
            />
            <div className="grid grid-cols-2 gap-2">
              <label className="flex flex-col gap-1">
                <span className="text-[12px] text-muted-foreground">Font</span>
                <select
                  value={editingFix.font}
                  onChange={(e) => updateFix({ font: e.target.value })}
                  className="h-8 rounded-md border border-input bg-background px-1.5"
                >
                  {FONTS.map((f) => (
                    <option key={f.id} value={f.id}>
                      {f.label}
                    </option>
                  ))}
                </select>
              </label>
              <label className="flex flex-col gap-1">
                <span className="text-[12px] text-muted-foreground">Size (pt)</span>
                <input
                  type="number"
                  min={4}
                  max={96}
                  step={0.5}
                  value={Math.round((editingFix.size ?? 0.02) * pagePoints[0] * 2) / 2}
                  onChange={(e) =>
                    updateFix({ size: Math.max(2, Number(e.target.value)) / pagePoints[0] })
                  }
                  className="h-8 rounded-md border border-input bg-background px-1.5"
                />
              </label>
              <label className="flex flex-col gap-1">
                <span className="text-[12px] text-muted-foreground">Text colour</span>
                <input
                  type="color"
                  value={editingFix.color}
                  onChange={(e) => updateFix({ color: e.target.value })}
                  className="h-8 w-full rounded-md border border-input"
                />
              </label>
              <label className="flex flex-col gap-1">
                <span className="text-[12px] text-muted-foreground">Paper</span>
                <input
                  type="color"
                  value={editingFix.background}
                  onChange={(e) => updateFix({ background: e.target.value })}
                  className="h-8 w-full rounded-md border border-input"
                />
              </label>
            </div>
            <Button
              variant="ghost"
              size="sm"
              className="self-start"
              onClick={() => {
                setFixes((list) => list.filter((_, i) => i !== editing));
                setEditing(null);
              }}
            >
              <Trash2 /> Remove this correction
            </Button>
          </div>
        )}
        {mode === "correct" && !editingFix && (
          <p className="text-muted-foreground">
            {fixes.length
              ? `${fixes.length} ${fixes.length === 1 ? "correction" : "corrections"} on this page. Click one to change it.`
              : "No corrections yet."}
          </p>
        )}

        <div className="mt-auto flex justify-end gap-2 pt-2">
          <Button variant="ghost" onClick={onCancel}>
            Cancel
          </Button>
          <Button onClick={save}>Done</Button>
        </div>
      </aside>
    </div>
  );
}
