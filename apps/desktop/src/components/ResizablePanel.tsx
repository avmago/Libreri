import { type ReactNode } from "react";
import { create } from "zustand";
import { cn } from "@/lib/utils";
import { ResizeHandle } from "./ResizeHandle";

const KEY = "libreri.panelWidths";

function load(): Record<string, number> {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}") as unknown;
    return v && typeof v === "object" ? (v as Record<string, number>) : {};
  } catch {
    return {};
  }
}

/** Widths of resizable columns by id, kept on this computer. */
const usePanelWidths = create<{
  widths: Record<string, number>;
  set: (id: string, width: number) => void;
}>((set, get) => ({
  widths: load(),
  set: (id, width) => {
    const widths = { ...get().widths, [id]: width };
    try {
      localStorage.setItem(KEY, JSON.stringify(widths));
    } catch {
      /* kept for this session only */
    }
    set({ widths });
  },
}));

/**
 * A side column the user can make wider or narrower by dragging its inner
 * edge (or with the arrow keys on the handle); double-click resets it.
 * The column's content fills it: give it `min-w-0 flex-1`.
 */
export function ResizablePanel({
  id,
  initial,
  min = 180,
  max = 640,
  side,
  label,
  className,
  children,
}: {
  /** Remembers the width ("library.sidebar"). */
  id: string;
  initial: number;
  min?: number;
  max?: number;
  /** Where the column sits: its handle is on the other edge. */
  side: "left" | "right";
  /** What is resized, for screen readers. */
  label: string;
  className?: string;
  children: ReactNode;
}) {
  const saved = usePanelWidths((s) => s.widths[id]);
  const setWidth = usePanelWidths((s) => s.set);
  const width = Math.min(max, Math.max(min, saved ?? initial));
  return (
    <div className={cn("relative flex shrink-0", className)} style={{ width }}>
      {children}
      <ResizeHandle
        width={width}
        onWidth={(w) => setWidth(id, w)}
        min={min}
        max={max}
        initial={initial}
        label={label}
        side={side === "left" ? "right" : "left"}
      />
    </div>
  );
}
