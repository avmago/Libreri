import { memo } from "react";
import { ArrowDown, ArrowUp, Folder, Heart } from "lucide-react";
import type { FolderDto, SortKey } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { authorsText, FILE_TYPE_LABEL, formatDate, formatSize, type BookView } from "../model";
import { useLibraryView } from "../store";
import { useDrag } from "../drag";
import { BookContextMenu } from "./BookContextMenu";
import { BookCover } from "./BookCover";
import { useItemHandlers, type ItemHandlers } from "../hooks/useBookInteractions";

const COLUMNS: { key: SortKey | null; label: string; className: string }[] = [
  { key: "title", label: "Title", className: "min-w-0 flex-[3]" },
  { key: "author", label: "Author", className: "min-w-0 flex-[2]" },
  { key: null, label: "Type", className: "w-16" },
  { key: "year", label: "Year", className: "w-14 text-right" },
  { key: "pages", label: "Pages", className: "w-14 text-right" },
  { key: "size", label: "Size", className: "w-18 text-right" },
  { key: "added", label: "Added", className: "w-28 text-right" },
];

function FolderRow({ folder, onOpen }: { folder: FolderDto; onOpen: () => void }) {
  const over = useDrag((s) => s.item !== null && s.target === folder.path);
  return (
    <div
      data-drop-folder={folder.path}
      onClick={onOpen}
      className={cn(
        "flex h-10 cursor-default items-center gap-3 border-b px-6 hover:bg-muted/60",
        over && "bg-muted ring-2 ring-primary ring-inset",
      )}
    >
      <Folder className="size-4 text-muted-foreground" aria-hidden />
      <span className="flex-1 font-medium">{folder.name}</span>
      <span className="text-muted-foreground">{folder.totalCount}</span>
    </div>
  );
}

const BookRow = memo(function BookRow({
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
          "flex h-12 cursor-default items-center gap-3 border-b px-6 outline-none [content-visibility:auto] [contain-intrinsic-size:auto_48px]",
          selected ? "bg-muted" : "hover:bg-muted/50",
          b.missing && "text-muted-foreground",
        )}
      >
        <div className="w-7">
          <BookCover book={b} className="rounded-[2px] shadow-none" />
        </div>
        <span className="flex min-w-0 flex-[3] flex-col">
          <span className="flex items-center gap-1.5 truncate font-medium">
            <span className="truncate">{b.metadata.title}</span>
            {b.user.favorite && (
              <Heart className="size-3 shrink-0 fill-current" aria-label="Favourite" />
            )}
          </span>
          {b.metadata.subtitle && (
            <span className="truncate text-[11.5px] text-muted-foreground">
              {b.metadata.subtitle}
            </span>
          )}
        </span>
        <span className="min-w-0 flex-[2] truncate text-muted-foreground">
          {b.metadata.authors.length ? authorsText(b.metadata.authors, 3) : "—"}
        </span>
        <span className="w-16 font-mono text-[11px] text-muted-foreground">
          {FILE_TYPE_LABEL[b.fileType]}
        </span>
        <span className="w-14 text-right text-muted-foreground tabular-nums">
          {b.metadata.year ?? ""}
        </span>
        <span className="w-14 text-right text-muted-foreground tabular-nums">
          {b.metadata.pages ?? ""}
        </span>
        <span className="w-18 text-right text-muted-foreground tabular-nums">
          {formatSize(b.fileSize)}
        </span>
        <span className="w-28 text-right text-muted-foreground">{formatDate(b.addedAt)}</span>
      </div>
    </BookContextMenu>
  );
});

export function BookList({
  books,
  folders,
  onOpenFolder,
}: {
  books: BookView[];
  folders: FolderDto[];
  onOpenFolder: (path: string) => void;
}) {
  const { selection, sort, descending, setSort } = useLibraryView();
  const handlers = useItemHandlers(books);
  const SortIcon = descending ? ArrowDown : ArrowUp;

  return (
    <div className="flex flex-col pb-10">
      <div className="sticky top-0 z-10 flex h-8 items-center gap-3 border-b bg-background px-6 text-[11.5px] font-medium text-muted-foreground">
        <span className="w-7" />
        {COLUMNS.map((c) => (
          <button
            key={c.label}
            type="button"
            disabled={!c.key}
            onClick={() => c.key && setSort(c.key)}
            className={cn(
              "flex items-center gap-1 truncate hover:text-foreground disabled:hover:text-muted-foreground",
              c.className.includes("text-right") && "justify-end",
              c.className,
              sort === c.key && "text-foreground",
            )}
          >
            {c.label}
            {sort === c.key && <SortIcon className="size-3" aria-hidden />}
          </button>
        ))}
      </div>
      {folders.map((f) => (
        <FolderRow key={f.path} folder={f} onOpen={() => onOpenFolder(f.path)} />
      ))}
      <div role="listbox" aria-multiselectable aria-label="Books">
        {books.map((b) => (
          <BookRow key={b.id} book={b} selected={selection.includes(b.id)} handlers={handlers} />
        ))}
      </div>
    </div>
  );
}
