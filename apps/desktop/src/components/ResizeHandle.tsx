import { useRef, type KeyboardEvent, type PointerEvent } from "react";
import { cn } from "@/lib/utils";

/**
 * A handle on the edge of a column, dragged to make it wider or narrower.
 * Arrow keys change it by 16 px (Shift: 64); double-click puts it back.
 */
export function ResizeHandle({
  width,
  onWidth,
  min,
  max,
  initial,
  label,
  side = "right",
}: {
  width: number;
  onWidth: (width: number) => void;
  min: number;
  max: number;
  /** The width a double-click puts back. */
  initial: number;
  /** What is resized, for screen readers ("Feeds list"). */
  label: string;
  /** The column's edge the handle is on. */
  side?: "right" | "left";
}) {
  const drag = useRef<{ x: number; width: number } | null>(null);
  const clamp = (w: number) => Math.round(Math.min(max, Math.max(min, w)));
  const dir = side === "right" ? 1 : -1;

  const down = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, width };
  };
  const move = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (d) onWidth(clamp(d.width + (e.clientX - d.x) * dir));
  };
  const up = () => {
    drag.current = null;
  };
  const key = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = e.shiftKey ? 64 : 16;
    if (e.key === "ArrowLeft") onWidth(clamp(width - step * dir));
    else if (e.key === "ArrowRight") onWidth(clamp(width + step * dir));
    else if (e.key === "Home") onWidth(min);
    else if (e.key === "End") onWidth(max);
    else return;
    e.preventDefault();
  };

  return (
    <div
      role="separator"
      aria-orientation="vertical"
      aria-label={`Resize ${label}`}
      aria-valuenow={width}
      aria-valuemin={min}
      aria-valuemax={max}
      tabIndex={0}
      title="Drag to resize · double-click to reset"
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={up}
      onDoubleClick={() => onWidth(initial)}
      onKeyDown={key}
      className={cn(
        "group absolute top-0 bottom-0 z-10 w-2 cursor-col-resize touch-none outline-none",
        side === "right" ? "-right-1" : "-left-1",
      )}
    >
      <span
        aria-hidden
        className="mx-auto block h-full w-0.5 bg-transparent transition-colors group-hover:bg-ring/40 group-focus-visible:bg-ring group-active:bg-ring"
      />
    </div>
  );
}
