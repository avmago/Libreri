import { useLayoutEffect, useMemo, useRef, type MouseEvent, type PointerEvent } from "react";
import { platform } from "@/lib/shortcuts";
import { startDragOnMove, useDrag } from "../drag";
import type { BookView } from "../model";
import { useLibraryView } from "../store";
import { useBookActions } from "./useBookActions";

/**
 * Handlers for every book in the grid or list. The object never changes, so
 * memoised cards only re-render when their own book or selection changes.
 */
export interface ItemHandlers {
  click: (e: MouseEvent, id: string) => void;
  open: (book: BookView) => void;
  pointerDown: (book: BookView) => (e: PointerEvent) => void;
  getBooks: () => BookView[];
}

export function useItemHandlers(books: BookView[]): ItemHandlers {
  const { open } = useBookActions();
  const latest = useRef({ books, open });
  useLayoutEffect(() => {
    latest.current = { books, open };
  });

  return useMemo<ItemHandlers>(
    () => ({
      getBooks: () => latest.current.books,
      open: (book) => void latest.current.open(book),
      click: (e, id) => {
        if (useDrag.getState().justDropped) return;
        const toggle = platform === "mac" ? e.metaKey : e.ctrlKey;
        const ids = latest.current.books.map((b) => b.id);
        useLibraryView
          .getState()
          .select(id, e.shiftKey ? "range" : toggle ? "toggle" : "replace", ids);
      },
      pointerDown: (book) =>
        startDragOnMove(() => {
          const { selection, setSelection } = useLibraryView.getState();
          let ids = selection;
          if (!selection.includes(book.id)) {
            ids = [book.id];
            setSelection(ids);
          }
          const label =
            ids.length === 1
              ? (latest.current.books.find((b) => b.id === ids[0])?.metadata.title ?? "1 book")
              : `${ids.length} books`;
          return { kind: "books", ids, label };
        }),
    }),
    [],
  );
}
