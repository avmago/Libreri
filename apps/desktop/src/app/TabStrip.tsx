import { useRef } from "react";
import { BookOpen, Columns2, FileText, Library, X } from "lucide-react";
import { useTabs } from "@/lib/tabs";
import { cn } from "@/lib/utils";

const tabClass = (active: boolean) =>
  cn(
    "group flex h-8 min-w-0 shrink items-center gap-2 rounded-t-lg border px-3 text-[13px]",
    active
      ? "-mb-px border-b-background bg-background font-medium"
      : "border-transparent text-muted-foreground hover:bg-muted/60 hover:text-foreground",
  );

/**
 * The library tab plus one tab per open book (Acrobat-style). Dragging a
 * book tab out of the window opens it in a window of its own.
 */
export function TabStrip({ onMoveToWindow }: { onMoveToWindow: (bookId: string) => void }) {
  const { tabs, active, activate, close, split } = useTabs();
  const drag = useRef<{ id: string; x: number; y: number; moved: boolean } | null>(null);

  return (
    <div role="tablist" aria-label="Open books" className="flex min-w-0 flex-1 items-end gap-0.5">
      <button
        role="tab"
        type="button"
        aria-selected={active === null}
        onClick={() => activate(null)}
        className={cn(tabClass(active === null), "shrink-0")}
      >
        <Library className="size-4 shrink-0" aria-hidden /> Library
      </button>
      {tabs.map((t) => {
        const isActive = active === t.bookId;
        const inSplit = split?.left === t.bookId || split?.right === t.bookId;
        const Icon = t.fileType === "md" || t.fileType === "txt" ? FileText : BookOpen;
        return (
          <div
            key={t.bookId}
            role="tab"
            tabIndex={0}
            aria-selected={isActive}
            title={`${t.title}\nDrag out of the window to open it in its own window`}
            onClick={() => !drag.current?.moved && activate(t.bookId)}
            onKeyDown={(e) => e.key === "Enter" && activate(t.bookId)}
            onAuxClick={(e) => e.button === 1 && close(t.bookId)}
            onPointerDown={(e) => {
              if (e.button !== 0) return;
              drag.current = { id: t.bookId, x: e.clientX, y: e.clientY, moved: false };
              e.currentTarget.setPointerCapture(e.pointerId);
            }}
            onPointerMove={(e) => {
              const d = drag.current;
              if (d && Math.hypot(e.clientX - d.x, e.clientY - d.y) > 8) d.moved = true;
            }}
            onPointerUp={(e) => {
              const d = drag.current;
              setTimeout(() => (drag.current = null), 0);
              if (!d?.moved) return;
              const outside =
                e.clientX < 0 ||
                e.clientY < 0 ||
                e.clientX > window.innerWidth ||
                e.clientY > window.innerHeight;
              if (outside) onMoveToWindow(d.id);
            }}
            className={cn(tabClass(isActive), "max-w-56 cursor-default pr-1.5 select-none")}
          >
            {inSplit ? (
              <Columns2 className="size-3.5 shrink-0" aria-label="In split view" />
            ) : (
              <Icon className="size-3.5 shrink-0" aria-hidden />
            )}
            <span className="truncate">{t.title}</span>
            <button
              type="button"
              aria-label={`Close ${t.title}`}
              onPointerDown={(e) => e.stopPropagation()}
              onClick={(e) => {
                e.stopPropagation();
                close(t.bookId);
              }}
              className={cn(
                "shrink-0 rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground",
                !isActive && "opacity-0 group-hover:opacity-100 focus-visible:opacity-100",
              )}
            >
              <X className="size-3.5" />
            </button>
          </div>
        );
      })}
    </div>
  );
}
