import { memo, useState } from "react";
import { useGridSize, useWindowRows } from "../hooks/useWindowRows";
import type { FolderDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useItemHandlers, type ItemHandlers } from "../hooks/useBookInteractions";
import { authorsText, type BookView } from "../model";
import { useSelectedIds } from "../store";
import { FolderTile } from "./BookGrid";
import { BookContextMenu } from "./BookContextMenu";
import { BookCover } from "./BookCover";

/** One row: covers standing on a shelf board. */
const ROW = 236;

const ShelfBook = memo(function ShelfBook({
  book: b,
  selected,
  handlers,
}: {
  book: BookView;
  selected: boolean;
  handlers: ItemHandlers;
}) {
  return (
    <BookContextMenu book={b} getBooks={handlers.getBooks}>
      <div
        role="option"
        aria-selected={selected}
        aria-label={`${b.metadata.title}, ${authorsText(b.metadata.authors, 2)}`}
        title={`${b.metadata.title}\n${authorsText(b.metadata.authors, 2)}`}
        data-book-id={b.id}
        tabIndex={-1}
        onClick={(e) => handlers.click(e, b.id)}
        onDoubleClick={() => handlers.open(b)}
        onPointerDown={handlers.pointerDown(b)}
        className={cn(
          // Off-screen books are not laid out or painted (big libraries).
          "flex h-full cursor-default items-end justify-center pb-[22px] outline-none transition-transform [content-visibility:auto] [contain-intrinsic-size:auto_236px]",
          selected ? "-translate-y-2" : "hover:-translate-y-1",
        )}
      >
        <BookCover
          book={b}
          className={cn(
            "w-[128px]",
            selected && "ring-2 ring-primary ring-offset-2 ring-offset-background",
          )}
        />
      </div>
    </BookContextMenu>
  );
});

/**
 * Board 9: books standing on shelves. Each row of the grid sits on a board
 * drawn by the background, so any window width fills whole shelves.
 */
export function BookShelf({
  books,
  folders,
  onOpenFolder,
}: {
  books: BookView[];
  folders: FolderDto[];
  onOpenFolder: (path: string) => void;
}) {
  const selected = useSelectedIds();
  const handlers = useItemHandlers(books);
  const [shelves, setShelves] = useState<HTMLDivElement | null>(null);
  const { cols } = useGridSize(shelves, 150, 0);
  const w = useWindowRows(shelves, books.length, cols, ROW);
  return (
    <div className="px-6 pt-2 pb-10">
      {folders.length > 0 && (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(112px,1fr))] gap-x-5 gap-y-4 pb-6">
          {folders.map((f) => (
            <FolderTile key={f.path} folder={f} onOpen={() => onOpenFolder(f.path)} />
          ))}
        </div>
      )}
      <div
        role="listbox"
        data-shortcuts
        aria-multiselectable
        aria-label="Books"
        data-book-grid
        ref={setShelves}
        className="lb-shelves grid grid-cols-[repeat(auto-fill,minmax(150px,1fr))]"
        style={{
          gridAutoRows: ROW,
          ...(w.on ? { paddingTop: w.before, paddingBottom: w.after } : {}),
        }}
      >
        {books.slice(w.start, w.end).map((b) => (
          <ShelfBook key={b.id} book={b} selected={selected.has(b.id)} handlers={handlers} />
        ))}
      </div>
    </div>
  );
}
