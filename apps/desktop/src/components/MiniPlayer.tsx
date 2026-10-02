import { useRef, useState, type PointerEvent } from "react";
import { ChevronsLeft, ChevronsRight, Loader2, Pause, Play } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useFloatingBar } from "@/lib/floating";
import { cn } from "@/lib/utils";

/**
 * The floating player folded into a small rounded box at the bottom left
 * or right: play / pause, and a button to open it again. Drag it to the
 * other side.
 */
export function MiniPlayer({
  label,
  playing,
  busy,
  onToggle,
  fixed,
  className,
  picture,
}: {
  /** What is playing, for screen readers and the tooltip. */
  label: string;
  playing: boolean;
  busy?: boolean;
  onToggle: () => void;
  /** Placed on the window (else on the page it is in). */
  fixed?: boolean;
  className?: string;
  /** A small picture behind the buttons (a podcast's artwork). */
  picture?: React.ReactNode;
}) {
  const { side, setSide, setCollapsed } = useFloatingBar();
  const box = useRef<HTMLDivElement>(null);
  const drag = useRef<{ x: number; moved: boolean } | null>(null);
  const [dx, setDx] = useState(0);

  const down = (e: PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 || (e.target as HTMLElement).closest("button")) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    drag.current = { x: e.clientX, moved: false };
  };
  const move = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    if (!d) return;
    const delta = e.clientX - d.x;
    if (Math.abs(delta) > 4) d.moved = true;
    setDx(delta);
  };
  const up = (e: PointerEvent<HTMLDivElement>) => {
    const d = drag.current;
    drag.current = null;
    setDx(0);
    if (!d?.moved) return;
    const area = (fixed ? null : box.current?.offsetParent?.getBoundingClientRect()) ?? {
      left: 0,
      width: window.innerWidth,
    };
    setSide(e.clientX < area.left + area.width / 2 ? "left" : "right");
  };

  const Expand = side === "left" ? ChevronsRight : ChevronsLeft;
  return (
    <div
      ref={box}
      role="group"
      aria-label={`Player (folded): ${label}`}
      title={`${label}\nDrag to move it to the other side`}
      onPointerDown={down}
      onPointerMove={move}
      onPointerUp={up}
      onPointerCancel={() => {
        drag.current = null;
        setDx(0);
      }}
      style={dx ? { transform: `translateX(${dx}px)` } : undefined}
      className={cn(
        fixed ? "fixed" : "absolute",
        "bottom-5 z-30 flex size-[68px] cursor-grab touch-none flex-col items-center justify-center gap-0.5 overflow-hidden rounded-2xl border bg-popover/95 text-popover-foreground shadow-xl backdrop-blur select-none active:cursor-grabbing",
        side === "left" ? "left-5" : "right-5",
        !dx && "transition-[left,right]",
        className,
      )}
    >
      {picture && <div className="pointer-events-none absolute inset-0 opacity-25">{picture}</div>}
      <Button
        size="icon"
        className="relative size-9 rounded-full"
        aria-label={playing ? "Pause" : "Play"}
        onClick={onToggle}
      >
        {busy ? <Loader2 className="animate-spin" /> : playing ? <Pause /> : <Play />}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        className="relative h-5 w-9 rounded-md"
        aria-label="Open the player"
        title="Open the player"
        onClick={() => setCollapsed(false)}
      >
        <Expand />
      </Button>
    </div>
  );
}
