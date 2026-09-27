import { memo } from "react";
import { Folder, Heart } from "lucide-react";
import type { FolderDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { startDragOnMove, useDrag } from "../drag";
import { useItemHandlers, type ItemHandlers } from "../hooks/useBookInteractions";
import { authorsText, type BookView } from "../model";
import { useLibraryView } from "../store";
import { BookContextMenu } from "./BookContextMenu";
import { BookCover } from "./BookCover";

export function FolderTile({ folder, onOpen }: { folder: FolderDto; onOpen: () => void }) {
  const over = useDrag((s) => s.item !== null && s.target === folder.path);
  return (
    <button
      type="button"
      data-drop-folder={folder.path}
      onClick={() => !useDrag.getState().justDropped && onOpen()}
      onPointerDown={startDragOnMove(() => ({
        kind: "folder",
        path: folder.path,
        label: folder.name,
      }))}
      className={cn(
        "group flex flex-col gap-2 rounded-lg p-1.5 text-left outline-none focus-visible:ring-2 focus-visible:ring-ring",
        over && "bg-muted ring-2 ring-primary",
      )}
    >
      <div className="flex aspect-[2/3] w-full flex-col items-center justify-center gap-2 rounded-[3px] border border-dashed bg-sidebar text-muted-foreground group-hover:bg-muted">
        <Folder className="size-9" strokeWidth={1.4} aria-hidden />
        <span className="text-[11px]">
          {folder.totalCount} {folder.totalCount === 1 ? "book" : "books"}
        </span>
      </div>
      <span className="line-clamp-2 text-[12.5px] leading-snug font-medium">{folder.name}</span>
    </button>
  );
}

const BookCard = memo(function BookCard({
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
        data-book-id={b.id}
        tabIndex={-1}
        onClick={(e) => handlers.click(e, b.id)}
        onDoubleClick={() => handlers.open(b)}
        onPointerDown={handlers.pointerDown(b)}
        className={cn(
          "flex cursor-default flex-col gap-2 rounded-lg p-1.5 outline-none [content-visibility:auto] [contain-intrinsic-size:auto_260px]",
          selected ? "bg-muted ring-2 ring-primary" : "hover:bg-muted/60",
        )}
      >
        <BookCover book={b} />
        <div className="flex min-w-0 flex-col gap-0.5 px-0.5">
          <span
            className="line-clamp-2 text-[12.5px] leading-snug font-medium"
            title={b.metadata.title}
          >
            {b.metadata.title}
          </span>
          <span className="flex items-center gap-1 text-[11.5px] text-muted-foreground">
            <span className="truncate">{authorsText(b.metadata.authors, 1)}</span>
            {b.user.favorite && (
              <Heart className="size-3 shrink-0 fill-current" aria-label="Favourite" />
            )}
          </span>
        </div>
      </div>
    </BookContextMenu>
  );
});

export function BookGrid({
  books,
  folders,
  onOpenFolder,
}: {
  books: BookView[];
  folders: FolderDto[];
  onOpenFolder: (path: string) => void;
}) {
  const selection = useLibraryView((s) => s.selection);
  const handlers = useItemHandlers(books);

  return (
    <div
      role="listbox"
      data-shortcuts
      aria-multiselectable
      aria-label="Books"
      data-book-grid
      className="grid grid-cols-[repeat(auto-fill,minmax(132px,1fr))] gap-x-5 gap-y-6 px-6 pt-4 pb-10"
    >
      {folders.map((f) => (
        <FolderTile key={f.path} folder={f} onOpen={() => onOpenFolder(f.path)} />
      ))}
      {books.map((b) => (
        <BookCard key={b.id} book={b} selected={selection.includes(b.id)} handlers={handlers} />
      ))}
    </div>
  );
}
