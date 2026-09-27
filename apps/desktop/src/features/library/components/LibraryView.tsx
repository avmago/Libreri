import { useCallback, useDeferredValue, useEffect, useMemo, useRef } from "react";
import {
  ArrowDownUp,
  BookOpen,
  BookmarkPlus,
  LibraryBig,
  ChevronDown,
  ChevronRight,
  Download,
  FileUp,
  FolderUp,
  LayoutGrid,
  List,
  ListFilter,
  PanelRight,
  RefreshCw,
  Search,
  Upload,
  SearchX,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import {
  DropdownCheckItem,
  DropdownMenu,
  MenuShortcut,
  menuContent,
  menuItem,
  menuLabel,
  menuSeparator,
} from "@/components/ui/menu";
import { commands, type SortKey } from "@/lib/ipc";
import { keysLabel, platform, shortcutFor, useShortcut, type ActionId } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";
import { usePermissions } from "@/features/profiles";
import { useBooks, useFacets, useFolders, useMoveFolder } from "../api";
import { useDetailsDialog, useFillDetails } from "@/features/details";
import { pickArchiveToImport, usePortability } from "@/features/portability";
import { useLibraryDialogs } from "../dialogs";
import type { DragItem } from "../drag";
import { DragLayer } from "./DragLayer";
import { pickFilesToImport, pickFolderToImport } from "../import";
import { useBookActions } from "../hooks/useBookActions";
import { usePdfCovers } from "../hooks/usePdfCovers";
import { CONTENT_TYPE_LABEL, FILE_TYPE_LABEL, SORT_LABEL, type BookView } from "../model";
import { buildQuery, navTitle, useLibraryView } from "../store";
import { BookGrid } from "./BookGrid";
import { BookList } from "./BookList";
import { BookShelf } from "./BookShelf";
import { DetailsPanel } from "./DetailsPanel";
import { findFolder } from "./folderUtils";

const keys = (id: ActionId) => keysLabel(shortcutFor(id), platform);

function Breadcrumbs({ path }: { path: string }) {
  const setNav = useLibraryView((s) => s.setNav);
  const parts = path.split("/");
  return (
    <nav
      aria-label="Folder path"
      className="flex min-w-0 items-center gap-1 text-xl font-semibold tracking-tight"
    >
      <button
        type="button"
        className="text-muted-foreground hover:text-foreground"
        onClick={() => setNav({ kind: "folder", path: "" })}
      >
        Books
      </button>
      {parts.map((p, i) => {
        const last = i === parts.length - 1;
        return (
          <span key={i} className="flex min-w-0 items-center gap-1">
            <ChevronRight className="size-4 shrink-0 text-muted-foreground" aria-hidden />
            {last ? (
              <span className="truncate">{p}</span>
            ) : (
              <button
                type="button"
                className="truncate text-muted-foreground hover:text-foreground"
                onClick={() => setNav({ kind: "folder", path: parts.slice(0, i + 1).join("/") })}
              >
                {p}
              </button>
            )}
          </span>
        );
      })}
    </nav>
  );
}

function FilterMenu() {
  const { data: facets } = useFacets();
  const view = useLibraryView();
  const active = view.fileTypes.length + view.contentTypes.length + view.tags.length;
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button variant="outline" size="sm" aria-label="Filter">
          <ListFilter /> Filter
          {active > 0 && (
            <span className="rounded bg-primary px-1 text-[10px] leading-4 text-primary-foreground">
              {active}
            </span>
          )}
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          sideOffset={6}
          className={cn(menuContent, "max-h-[70vh] w-60 overflow-y-auto")}
        >
          <DropdownMenu.Label className={menuLabel}>FILE TYPE</DropdownMenu.Label>
          {(facets?.fileTypes ?? []).map(({ value, count }) => (
            <DropdownCheckItem
              key={value}
              checked={view.fileTypes.includes(value)}
              onCheckedChange={() => view.toggleFileType(value)}
            >
              <span className="flex-1">{FILE_TYPE_LABEL[value]}</span>
              <span className="text-[11px] text-muted-foreground">{count}</span>
            </DropdownCheckItem>
          ))}
          <DropdownMenu.Separator className={menuSeparator} />
          <DropdownMenu.Label className={menuLabel}>CONTENT TYPE</DropdownMenu.Label>
          {(facets?.contentTypes ?? []).map(({ value, count }) => (
            <DropdownCheckItem
              key={value}
              checked={view.contentTypes.includes(value)}
              onCheckedChange={() => view.toggleContentType(value)}
            >
              <span className="flex-1">{CONTENT_TYPE_LABEL[value]}</span>
              <span className="text-[11px] text-muted-foreground">{count}</span>
            </DropdownCheckItem>
          ))}
          {(facets?.tags.length ?? 0) > 0 && (
            <>
              <DropdownMenu.Separator className={menuSeparator} />
              <DropdownMenu.Label className={menuLabel}>TAGS</DropdownMenu.Label>
              {facets!.tags.slice(0, 40).map(({ value, count }) => (
                <DropdownCheckItem
                  key={value}
                  checked={view.tags.includes(value)}
                  onCheckedChange={() => view.toggleTag(value)}
                >
                  <span className="flex-1 truncate">{value}</span>
                  <span className="text-[11px] text-muted-foreground">{count}</span>
                </DropdownCheckItem>
              ))}
            </>
          )}
          {active > 0 && (
            <>
              <DropdownMenu.Separator className={menuSeparator} />
              <DropdownMenu.Item className={menuItem} onSelect={view.clearFilters}>
                <X /> Clear filters
              </DropdownMenu.Item>
            </>
          )}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function SortMenu() {
  const { sort, descending, setSort, setDescending, nav, includeSubfolders, setIncludeSubfolders } =
    useLibraryView();
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button variant="outline" size="sm" aria-label="Sort">
          <ArrowDownUp /> {SORT_LABEL[sort]}
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" sideOffset={6} className={cn(menuContent, "w-52")}>
          <DropdownMenu.Label className={menuLabel}>SORT BY</DropdownMenu.Label>
          {(Object.keys(SORT_LABEL) as SortKey[]).map((k) => (
            <DropdownCheckItem
              key={k}
              checked={sort === k}
              onCheckedChange={() => sort !== k && setSort(k)}
            >
              {SORT_LABEL[k]}
            </DropdownCheckItem>
          ))}
          <DropdownMenu.Separator className={menuSeparator} />
          <DropdownCheckItem checked={!descending} onCheckedChange={() => setDescending(false)}>
            Ascending
          </DropdownCheckItem>
          <DropdownCheckItem checked={descending} onCheckedChange={() => setDescending(true)}>
            Descending
          </DropdownCheckItem>
          {nav.kind === "folder" && (
            <>
              <DropdownMenu.Separator className={menuSeparator} />
              <DropdownCheckItem checked={includeSubfolders} onCheckedChange={setIncludeSubfolders}>
                Include subfolders
              </DropdownCheckItem>
            </>
          )}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function ImportMenu() {
  const { kind } = usePermissions();
  const openExport = usePortability((s) => s.openExport);
  return (
    <div className="flex">
      <Button size="sm" className="rounded-r-none" onClick={() => void pickFilesToImport()}>
        <FileUp /> Import
      </Button>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <Button
            size="sm"
            className="rounded-l-none border-l border-primary-foreground/20 px-1.5"
            aria-label="More import options"
          >
            <ChevronDown />
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content align="end" sideOffset={6} className={menuContent}>
            <DropdownMenu.Item className={menuItem} onSelect={() => void pickFilesToImport()}>
              <FileUp /> Import files… <MenuShortcut>{keys("library.import")}</MenuShortcut>
            </DropdownMenu.Item>
            <DropdownMenu.Item className={menuItem} onSelect={() => void pickFolderToImport()}>
              <FolderUp /> Import a folder…{" "}
              <MenuShortcut>{keys("library.importFolder")}</MenuShortcut>
            </DropdownMenu.Item>
            <DropdownMenu.Separator className={menuSeparator} />
            <DropdownMenu.Item className={menuItem} onSelect={() => void commands.rescanLibrary()}>
              <RefreshCw /> Check folder for changes{" "}
              <MenuShortcut>{keys("library.refresh")}</MenuShortcut>
            </DropdownMenu.Item>
            <DropdownMenu.Separator className={menuSeparator} />
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() => {
                const sel = useLibraryView.getState().selection;
                openExport(sel.length ? sel : null);
              }}
            >
              <Download /> Export… <MenuShortcut>{keys("library.export")}</MenuShortcut>
            </DropdownMenu.Item>
            {kind === "owner" && (
              <DropdownMenu.Item className={menuItem} onSelect={() => void pickArchiveToImport()}>
                <Upload /> Import a Libreri archive…
              </DropdownMenu.Item>
            )}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>
    </div>
  );
}

function ActiveFilters() {
  const view = useLibraryView();
  const chips = [
    ...view.fileTypes.map((t) => ({
      key: `f-${t}`,
      label: FILE_TYPE_LABEL[t],
      remove: () => view.toggleFileType(t),
    })),
    ...view.contentTypes.map((t) => ({
      key: `c-${t}`,
      label: CONTENT_TYPE_LABEL[t],
      remove: () => view.toggleContentType(t),
    })),
    ...view.tags.map((t) => ({ key: `t-${t}`, label: `#${t}`, remove: () => view.toggleTag(t) })),
  ];
  if (!chips.length) return null;
  return (
    <div className="flex flex-wrap items-center gap-1.5 px-6 pb-3">
      {chips.map((c) => (
        <span
          key={c.key}
          className="flex items-center gap-1 rounded-full border bg-sidebar py-0.5 pr-1 pl-2.5 text-[12px]"
        >
          {c.label}
          <button
            type="button"
            aria-label={`Remove filter ${c.label}`}
            onClick={c.remove}
            className="rounded-full p-0.5 hover:bg-muted"
          >
            <X className="size-3" />
          </button>
        </span>
      ))}
      <button
        type="button"
        onClick={view.clearFilters}
        className="px-1 text-[12px] text-muted-foreground hover:text-foreground"
      >
        Clear all
      </button>
    </div>
  );
}

function EmptyState({ filtered, totalBooks }: { filtered: boolean; totalBooks: number }) {
  const clearFilters = useLibraryView((s) => s.clearFilters);
  const { editLibrary } = usePermissions();
  if (filtered) {
    return (
      <div className="flex flex-1 flex-col items-center justify-center gap-3 py-20 text-center">
        <SearchX className="size-8 text-muted-foreground" aria-hidden />
        <p className="text-muted-foreground">No books match.</p>
        <Button variant="outline" size="sm" onClick={clearFilters}>
          Clear search and filters
        </Button>
      </div>
    );
  }
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 py-20 text-center">
      <div className="flex size-12 items-center justify-center rounded-xl bg-muted">
        <BookOpen className="size-6 text-muted-foreground" aria-hidden />
      </div>
      <h2 className="text-base font-semibold">
        {totalBooks === 0 ? "No books yet" : "Nothing here yet"}
      </h2>
      {editLibrary ? (
        <>
          <p className="max-w-sm text-muted-foreground">
            Drop books or whole folders onto this window, or import them. PDF, EPUB, Markdown,
            comics, DjVu, FB2, MOBI, text files and audiobooks are all welcome.
          </p>
          <div className="flex gap-2">
            <Button onClick={() => void pickFilesToImport()}>
              <FileUp /> Import files…
            </Button>
            <Button variant="outline" onClick={() => void pickFolderToImport()}>
              <FolderUp /> Import a folder…
            </Button>
          </div>
        </>
      ) : (
        <p className="max-w-sm text-muted-foreground">Ask the library's owner to add books here.</p>
      )}
    </div>
  );
}

/** Moves the selection with the arrow keys, in grid or list order. */
function useArrowKeys(books: BookView[], columns: () => number) {
  const move = (delta: (cols: number) => number) => () => {
    if (!books.length) return;
    const { selection, setSelection } = useLibraryView.getState();
    const current = books.findIndex((b) => b.id === selection[selection.length - 1]);
    const next =
      current < 0 ? 0 : Math.min(books.length - 1, Math.max(0, current + delta(columns())));
    const id = books[next]?.id;
    if (!id) return;
    setSelection([id]);
    document.querySelector(`[data-book-id="${id}"]`)?.scrollIntoView({ block: "nearest" });
  };
  useShortcut(
    "books.next",
    move(() => 1),
  );
  useShortcut(
    "books.previous",
    move(() => -1),
  );
  useShortcut(
    "books.down",
    move((c) => c),
  );
  useShortcut(
    "books.up",
    move((c) => -c),
  );
  useShortcut(
    "books.first",
    move(() => -books.length),
  );
  useShortcut(
    "books.last",
    move(() => books.length),
  );
}

export function LibraryView() {
  const view = useLibraryView();
  // Typing stays smooth: the list follows the search box a moment later.
  const search = useDeferredValue(view.search);
  const query = useMemo(
    () => buildQuery({ ...view, search }),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [
      view.nav,
      search,
      view.fileTypes,
      view.contentTypes,
      view.tags,
      view.sort,
      view.descending,
      view.includeSubfolders,
    ],
  );
  const { data: books = [], isFetching } = useBooks(query);
  const { data: facets } = useFacets();
  const { data: folderTree = [] } = useFolders();
  const actions = useBookActions();
  const moveFolder = useMoveFolder();
  const searchRef = useRef<HTMLInputElement>(null);
  const { editLibrary } = usePermissions();
  const dialogs = useLibraryDialogs();
  const openFinder = useDetailsDialog((s) => s.open);
  const fillDetails = useFillDetails();
  const openExport = usePortability((s) => s.openExport);
  const openCitation = usePortability((s) => s.openCitation);
  usePdfCovers(books);

  const selected = books.filter((b) => view.selection.includes(b.id));
  const childFolders =
    view.nav.kind === "folder" && !query.includeSubfolders && !query.search
      ? view.nav.path === ""
        ? folderTree
        : (findFolder(folderTree, view.nav.path)?.children ?? [])
      : [];
  const filtered =
    Boolean(query.search) ||
    view.fileTypes.length + view.contentTypes.length + view.tags.length > 0;

  // Keep the selection to books that are still shown.
  useEffect(() => {
    const ids = new Set(books.map((b) => b.id));
    const kept = view.selection.filter((id) => ids.has(id));
    if (kept.length !== view.selection.length) view.setSelection(kept);
  }, [books]); // eslint-disable-line react-hooks/exhaustive-deps

  useShortcut("library.search", () => searchRef.current?.focus());
  useShortcut("library.import", () => editLibrary && void pickFilesToImport());
  useShortcut("library.importFolder", () => editLibrary && void pickFolderToImport());
  useShortcut("library.refresh", () => void commands.rescanLibrary());
  useShortcut("view.grid", () => view.setView("grid"));
  useShortcut("view.list", () => view.setView("list"));
  useShortcut("view.shelf", () => view.setView("shelf"));
  useShortcut("filters.clear", view.clearFilters);
  useShortcut("collection.save", () => dialogs.setSaveCollection(true));
  useShortcut("sort.title", () => view.setSort("title"));
  useShortcut("sort.author", () => view.setSort("author"));
  useShortcut("sort.added", () => view.setSort("added"));
  useShortcut("sort.lastOpened", () => view.setSort("lastOpened"));
  useShortcut("books.bulkEdit", () => editLibrary && dialogs.openBulkEdit(selected));
  useShortcut("books.markWantToRead", () => actions.setStatus(selected, "wantToRead"));
  useShortcut("books.markReading", () => actions.setStatus(selected, "reading"));
  useShortcut("books.markFinished", () => actions.setStatus(selected, "finished"));
  useShortcut("books.markNone", () => actions.setStatus(selected, "none"));
  useShortcut("books.rate0", () => actions.setRating(selected, 0));
  useShortcut("books.rate1", () => actions.setRating(selected, 1));
  useShortcut("books.rate2", () => actions.setRating(selected, 2));
  useShortcut("books.rate3", () => actions.setRating(selected, 3));
  useShortcut("books.rate4", () => actions.setRating(selected, 4));
  useShortcut("books.rate5", () => actions.setRating(selected, 5));
  useShortcut("details.toggle", view.toggleDetails);
  useShortcut("details.find", () => {
    if (editLibrary && selected.length === 1) openFinder(selected[0]!.id);
  });
  useShortcut("details.fill", () => editLibrary && fillDetails(selected.map((b) => b.id)));
  useShortcut(
    "library.export",
    () => editLibrary && openExport(selected.length ? selected.map((b) => b.id) : null),
  );
  useShortcut("books.cite", () => selected.length && openCitation(selected.map((b) => b.id)));
  useShortcut("books.selectAll", () => view.setSelection(books.map((b) => b.id)));
  useShortcut("books.clearSelection", () => view.setSelection([]));
  const only = selected.length === 1 ? selected[0] : undefined;
  useShortcut("books.open", () => only && void actions.open(only));
  useShortcut("books.reveal", () => only && void actions.reveal(only));
  useShortcut("books.trash", () => editLibrary && void actions.moveToTrash(selected));
  useShortcut("books.trashMac", () => editLibrary && void actions.moveToTrash(selected));
  useShortcut("books.favorite", () => selected.length && actions.toggleFavorite(selected));
  useArrowKeys(books, () => {
    if (view.view === "list") return 1;
    const grid = document.querySelector("[data-book-grid]");
    return grid ? getComputedStyle(grid).gridTemplateColumns.split(" ").length : 1;
  });

  const onDrop = useCallback(
    (item: DragItem, folder: string) => {
      if (item.kind === "books") {
        void actions.moveTo(
          books.filter((b) => item.ids.includes(b.id)),
          folder,
        );
      } else {
        moveFolder.mutate(
          { path: item.path, parent: folder },
          { onError: (e) => toast.error("Could not move the folder", { description: String(e) }) },
        );
      }
    },
    [actions, books, moveFolder],
  );

  const Content = view.view === "grid" ? BookGrid : view.view === "shelf" ? BookShelf : BookList;
  const openFolder = (path: string) => view.setNav({ kind: "folder", path });
  const count = books.length;

  return (
    <div className="flex h-full min-w-0">
      <div className="flex min-w-0 flex-1 flex-col">
        <header className="flex flex-col gap-3 px-6 pt-4 pb-3">
          <div className="flex items-center gap-3">
            <div className="flex min-w-0 flex-1 items-baseline gap-3">
              {view.nav.kind === "folder" && view.nav.path !== "" ? (
                <Breadcrumbs path={view.nav.path} />
              ) : (
                <h1 className="truncate text-xl font-semibold tracking-tight">
                  {navTitle(view.nav)}
                </h1>
              )}
              <span className="shrink-0 text-muted-foreground">
                {childFolders.length > 0 &&
                  `${childFolders.length} ${childFolders.length === 1 ? "folder" : "folders"} · `}
                {count} {count === 1 ? "book" : "books"}
                {isFetching && (
                  <RefreshCw className="ml-2 inline size-3 animate-spin" aria-label="Loading" />
                )}
              </span>
            </div>
            {selected.length > 1 && editLibrary && (
              <Button variant="outline" size="sm" onClick={() => dialogs.openBulkEdit(selected)}>
                Edit {selected.length} together…
              </Button>
            )}
            {editLibrary && <ImportMenu />}
            <Button
              variant="ghost"
              size="icon"
              aria-label="Show or hide details"
              aria-pressed={view.detailsOpen}
              title={`Details (${keys("details.toggle")})`}
              onClick={view.toggleDetails}
            >
              <PanelRight />
            </Button>
          </div>
          <div className="flex items-center gap-2">
            <div className="relative w-72 min-w-40 shrink">
              <Search
                className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                aria-hidden
              />
              <input
                ref={searchRef}
                type="search"
                value={view.search}
                onChange={(e) => view.setSearch(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Escape") {
                    view.setSearch("");
                    e.currentTarget.blur();
                  }
                }}
                placeholder="Title, author, tag, ISBN…"
                aria-label="Search books"
                className="h-8 w-full rounded-md border border-input bg-background pr-14 pl-8 text-[13px] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/20"
              />
              <Kbd
                action="library.search"
                className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2"
              />
            </div>
            <div className="flex-1" />
            {(filtered || view.nav.kind === "tag" || view.nav.kind === "category") && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => dialogs.setSaveCollection(true)}
                title={`Save as a smart collection (${keys("collection.save")})`}
              >
                <BookmarkPlus /> Save search
              </Button>
            )}
            <FilterMenu />
            <SortMenu />
            <div className="flex rounded-md border p-0.5" role="group" aria-label="View">
              <Button
                variant="ghost"
                size="icon"
                className={cn("size-7", view.view === "grid" && "bg-muted")}
                aria-label="Grid"
                aria-pressed={view.view === "grid"}
                title={`Grid (${keys("view.grid")})`}
                onClick={() => view.setView("grid")}
              >
                <LayoutGrid />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className={cn("size-7", view.view === "list" && "bg-muted")}
                aria-label="List"
                aria-pressed={view.view === "list"}
                title={`List (${keys("view.list")})`}
                onClick={() => view.setView("list")}
              >
                <List />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className={cn("size-7", view.view === "shelf" && "bg-muted")}
                aria-label="Shelves"
                aria-pressed={view.view === "shelf"}
                title={`Shelves (${keys("view.shelf")})`}
                onClick={() => view.setView("shelf")}
              >
                <LibraryBig />
              </Button>
            </div>
          </div>
        </header>
        <ActiveFilters />
        <div
          className="flex min-h-0 flex-1 flex-col overflow-y-auto border-t"
          data-drop-folder={view.nav.kind === "folder" ? view.nav.path : undefined}
          onClick={(e) => {
            if (e.target === e.currentTarget) view.setSelection([]);
          }}
        >
          {count === 0 && childFolders.length === 0 ? (
            <EmptyState filtered={filtered} totalBooks={facets?.total ?? 0} />
          ) : (
            <Content books={books} folders={childFolders} onOpenFolder={openFolder} />
          )}
        </div>
      </div>
      {view.detailsOpen && <DetailsPanel books={books} />}
      <DragLayer onDrop={onDrop} />
    </div>
  );
}
