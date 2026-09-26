import { useEffect } from "react";
import { createPortal } from "react-dom";
import { BookCopy, Folder } from "lucide-react";
import { dropHandler, useDrag, type DragItem } from "../drag";

/** Renders the drag preview and handles drops. Mount once. */
export function DragLayer({ onDrop }: { onDrop: (item: DragItem, folder: string) => void }) {
  useEffect(() => {
    dropHandler.current = onDrop;
    return () => {
      if (dropHandler.current === onDrop) dropHandler.current = null;
    };
  }, [onDrop]);
  const { item, x, y, target } = useDrag();
  if (!item) return null;
  const Icon = item.kind === "folder" ? Folder : BookCopy;
  return createPortal(
    <div
      className="pointer-events-none fixed z-[100] flex max-w-64 items-center gap-2 rounded-md border bg-popover px-2.5 py-1.5 text-[12px] font-medium shadow-lg"
      style={{ left: x + 12, top: y + 12 }}
    >
      <Icon className="size-4 shrink-0 text-muted-foreground" aria-hidden />
      <span className="truncate">{item.label}</span>
      {target !== null && (
        <span className="shrink-0 text-muted-foreground">
          → {target === "" ? "Books" : target.split("/").pop()}
        </span>
      )}
    </div>,
    document.body,
  );
}
