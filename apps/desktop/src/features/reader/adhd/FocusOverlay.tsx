import { useEffect, useRef, useState } from "react";
import type { LineBox } from "@/readers";

/** Where the pointer is, relative to the overlay, and the line under it. */
interface Place {
  y: number;
  line: LineBox | null;
}

/**
 * ADHD reading over the page: a highlight on the line under the pointer,
 * and a mask that dims the page except a strip around it. It lets the
 * pointer through, so text can still be selected.
 */
export function FocusOverlay({
  line,
  mask,
  maskHeight,
  findLine,
  onSink,
  refresh,
}: {
  line: boolean;
  mask: boolean;
  /** Height of the strip left clear by the mask, in pixels. */
  maskHeight: number;
  /** The line at a point in the window (null where there is no text). */
  findLine: (x: number, y: number) => LineBox | null;
  /** Moves over pages in frames (EPUB), which the page area does not see. */
  onSink: (look: ((x: number, y: number) => void) | null) => void;
  /** Looks again when this changes (a page was turned). */
  refresh: unknown;
}) {
  const root = useRef<HTMLDivElement>(null);
  const [place, setPlace] = useState<Place | null>(null);
  const find = useRef(findLine);
  useEffect(() => {
    find.current = findLine;
  }, [findLine]);

  // Looks at the pointer again (null until the overlay is on the page).
  const again = useRef<(() => void) | null>(null);

  useEffect(() => {
    const area = root.current?.parentElement;
    if (!area) return;
    let last: { x: number; y: number } | null = null;
    let frame = 0;
    const look = (x: number, y: number) => {
      last = { x, y };
      if (frame) return;
      frame = requestAnimationFrame(() => {
        frame = 0;
        const el = root.current;
        if (!el || !last) return;
        const r = el.getBoundingClientRect();
        const box = find.current(last.x, last.y);
        setPlace({
          y: last.y - r.top,
          line: box
            ? {
                top: box.top - r.top,
                bottom: box.bottom - r.top,
                left: Math.max(0, box.left - r.left),
                right: Math.min(r.width, box.right - r.left),
              }
            : null,
        });
      });
    };
    const lookAgain = () => {
      if (last) look(last.x, last.y);
    };
    const move = (e: PointerEvent) => look(e.clientX, e.clientY);
    const leave = () => setPlace((p) => (p ? { ...p, line: null } : p));
    area.addEventListener("pointermove", move);
    area.addEventListener("pointerleave", leave);
    // The page scrolled under a pointer that did not move.
    area.addEventListener("scroll", lookAgain, true);
    area.addEventListener("wheel", lookAgain, { passive: true });
    again.current = lookAgain;
    onSink(look);
    return () => {
      area.removeEventListener("pointermove", move);
      area.removeEventListener("pointerleave", leave);
      area.removeEventListener("scroll", lookAgain, true);
      area.removeEventListener("wheel", lookAgain);
      again.current = null;
      onSink(null);
      cancelAnimationFrame(frame);
    };
  }, [onSink]);

  useEffect(() => {
    // After the page has been drawn.
    const id = setTimeout(() => again.current?.(), 60);
    return () => clearTimeout(id);
  }, [refresh]);

  const box = place?.line ?? null;
  const center = box ? (box.top + box.bottom) / 2 : (place?.y ?? null);
  const half = maskHeight / 2;
  // Before the pointer has moved: a third of the way down.
  const edge = (d: number) => (center === null ? `calc(33% + ${d}px)` : `${center + d}px`);

  return (
    <div
      ref={root}
      aria-hidden
      data-testid="adhd-overlay"
      className="pointer-events-none absolute inset-0 z-10 overflow-hidden"
    >
      {line && box && (
        <div
          data-testid="adhd-line"
          className="absolute rounded-sm transition-[top,height,left,width] duration-75 ease-out"
          style={{
            top: box.top - 3,
            height: box.bottom - box.top + 6,
            left: Math.max(0, box.left - 8),
            width: box.right - box.left + 16,
            background: "rgba(250, 204, 21, 0.3)",
          }}
        />
      )}
      {mask && (
        <>
          <div
            data-testid="adhd-mask"
            className="absolute inset-x-0 top-0 bg-black/55 transition-[height] duration-75 ease-out"
            style={{ height: `max(0px, ${edge(-half)})` }}
          />
          <div
            className="absolute inset-x-0 bottom-0 bg-black/55 transition-[top] duration-75 ease-out"
            style={{ top: edge(half) }}
          />
        </>
      )}
    </div>
  );
}
