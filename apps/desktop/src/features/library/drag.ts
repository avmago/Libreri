/**
 * Dragging books and folders inside the window.
 *
 * Built on pointer events rather than HTML5 drag-and-drop, because the
 * webview reserves native drag-and-drop for files dropped from the desktop
 * (and on Windows the two cannot be used together). Drop targets are any
 * element with `data-drop-folder="<path>"`.
 */
import type { PointerEvent as ReactPointerEvent } from "react";
import { create } from "zustand";

export type DragItem =
  { kind: "books"; ids: string[]; label: string } | { kind: "folder"; path: string; label: string };

interface DragState {
  item: DragItem | null;
  x: number;
  y: number;
  /** Folder path under the pointer, if it accepts the item. */
  target: string | null;
  /** Set right after a drop so the click that follows is ignored. */
  justDropped: boolean;
}

export const useDrag = create<DragState>(() => ({
  item: null,
  x: 0,
  y: 0,
  target: null,
  justDropped: false,
}));

const THRESHOLD = 5;

function targetAt(x: number, y: number, item: DragItem): string | null {
  const el = document.elementFromPoint(x, y)?.closest<HTMLElement>("[data-drop-folder]");
  if (!el) return null;
  const path = el.dataset.dropFolder ?? "";
  if (item.kind === "folder") {
    const parent = item.path.includes("/") ? item.path.slice(0, item.path.lastIndexOf("/")) : "";
    // Not into itself, its own subfolders, or where it already is.
    if (path === item.path || path.startsWith(`${item.path}/`) || path === parent) return null;
  }
  return path;
}

/**
 * Returns a pointer-down handler that starts a drag once the pointer moves a
 * few pixels. `getItem` is called at that moment (so it can read the current
 * selection).
 */
export function startDragOnMove(getItem: () => DragItem | null) {
  return (e: ReactPointerEvent) => {
    if (e.button !== 0) return;
    const startX = e.clientX;
    const startY = e.clientY;
    let started = false;

    const move = (ev: PointerEvent) => {
      if (!started) {
        if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < THRESHOLD) return;
        const item = getItem();
        if (!item) return cleanup();
        started = true;
        useDrag.setState({ item, justDropped: false });
        document.body.style.cursor = "grabbing";
      }
      const { item } = useDrag.getState();
      if (!item) return;
      useDrag.setState({
        x: ev.clientX,
        y: ev.clientY,
        target: targetAt(ev.clientX, ev.clientY, item),
      });
    };
    const up = () => {
      if (started) {
        const { item, target } = useDrag.getState();
        if (item && target !== null) dropHandler.current?.(item, target);
        useDrag.setState({ item: null, target: null, justDropped: true });
        setTimeout(() => useDrag.setState({ justDropped: false }), 0);
      }
      cleanup();
    };
    const key = (ev: KeyboardEvent) => {
      if (ev.key === "Escape") {
        useDrag.setState({ item: null, target: null });
        cleanup();
      }
    };
    function cleanup() {
      document.body.style.cursor = "";
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      window.removeEventListener("keydown", key, true);
    }
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
    window.addEventListener("keydown", key, true);
  };
}

/** Set by the mounted `DragLayer`. */
export const dropHandler: { current: ((item: DragItem, folder: string) => void) | null } = {
  current: null,
};
