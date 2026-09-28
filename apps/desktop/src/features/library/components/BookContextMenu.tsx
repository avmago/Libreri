import { useState, type ReactNode } from "react";
import {
  BookOpen,
  Check,
  Columns2,
  Download,
  ExternalLink,
  FolderInput,
  FolderSearch,
  GitCompare,
  Globe,
  Heart,
  Info,
  ListChecks,
  Pencil,
  Quote,
  ScanText,
  Star,
  Trash2,
} from "lucide-react";
import { useDetailsDialog, useFillDetails } from "@/features/details";
import { usePortability } from "@/features/portability";
import { usePermissions } from "@/features/profiles";
import { useOcrDialog } from "@/features/search";
import {
  ContextMenu,
  MenuShortcut,
  menuContent,
  menuItem,
  menuSeparator,
} from "@/components/ui/menu";
import { keysLabel, platform, shortcutFor, type ActionId } from "@/lib/shortcuts";
import type { ReadingStatus } from "@/lib/ipc";
import { useTabs } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import { useFolders } from "../api";
import { useLibraryDialogs } from "../dialogs";
import { useBookActions } from "../hooks/useBookActions";
import { STATUS_LABEL, type BookView } from "../model";
import { useLibraryView } from "../store";
import { flattenFolders } from "./folderUtils";

const keys = (id: ActionId) => keysLabel(shortcutFor(id), platform);

const STATUSES: ReadingStatus[] = ["wantToRead", "reading", "finished", "abandoned", "none"];

/** The menu itself; only mounted while open, so closed menus cost nothing. */
function MenuItems({ book, targets }: { book: BookView; targets: BookView[] }) {
  const actions = useBookActions();
  const { editLibrary } = usePermissions();
  const openBulk = useLibraryDialogs((s) => s.openBulkEdit);
  const openFinder = useDetailsDialog((s) => s.open);
  const fillDetails = useFillDetails();
  const openCitation = usePortability((s) => s.openCitation);
  const openExport = usePortability((s) => s.openExport);
  const hasOpenBook = useTabs((s) => s.tabs.length > 0);
  const setDetailsOpen = useLibraryView((s) => s.setDetailsOpen);
  const { data: folders = [] } = useFolders();
  const single = targets.length === 1;
  const scans = targets.filter((b) => b.fileType === "pdf" || b.fileType === "djvu");
  const allFavourite = targets.every((b) => b.user.favorite);

  return (
    <>
      {single && (
        <>
          <ContextMenu.Item className={menuItem} onSelect={() => void actions.open(book)}>
            <BookOpen /> Open <MenuShortcut>{keys("books.open")}</MenuShortcut>
          </ContextMenu.Item>
          {hasOpenBook && (
            <ContextMenu.Item className={menuItem} onSelect={() => actions.openBeside(book)}>
              <Columns2 /> Open beside the current book
            </ContextMenu.Item>
          )}
          <ContextMenu.Item className={menuItem} onSelect={() => void actions.openElsewhere(book)}>
            <ExternalLink /> Open in another app
          </ContextMenu.Item>
          <ContextMenu.Item className={menuItem} onSelect={() => void actions.reveal(book)}>
            <FolderSearch /> Show in file manager
            <MenuShortcut>{keys("books.reveal")}</MenuShortcut>
          </ContextMenu.Item>
          <ContextMenu.Item className={menuItem} onSelect={() => setDetailsOpen(true)}>
            <Info /> Details <MenuShortcut>{keys("details.toggle")}</MenuShortcut>
          </ContextMenu.Item>
          <ContextMenu.Separator className={menuSeparator} />
        </>
      )}
      {targets.length === 2 && scans.length === 2 && (
        <>
          <ContextMenu.Item
            className={menuItem}
            onSelect={() => {
              const [a, b] = targets as [BookView, BookView];
              useTabs.getState().open({
                bookId: a.id,
                title: a.metadata.title ?? "Book",
                fileType: a.fileType,
                compare: { a: { kind: "book", id: a.id }, b: { kind: "book", id: b.id } },
              });
            }}
          >
            <GitCompare /> Compare these two
          </ContextMenu.Item>
          <ContextMenu.Separator className={menuSeparator} />
        </>
      )}
      <ContextMenu.Sub>
        <ContextMenu.SubTrigger className={menuItem}>
          <ListChecks /> Reading status
        </ContextMenu.SubTrigger>
        <ContextMenu.Portal>
          <ContextMenu.SubContent className={menuContent}>
            {STATUSES.map((s) => (
              <ContextMenu.Item
                key={s}
                className={menuItem}
                onSelect={() => actions.setStatus(targets, s)}
              >
                <span className="flex size-4 items-center justify-center">
                  {targets.every((b) => b.user.status === s) && <Check />}
                </span>
                {STATUS_LABEL[s]}
              </ContextMenu.Item>
            ))}
          </ContextMenu.SubContent>
        </ContextMenu.Portal>
      </ContextMenu.Sub>
      <ContextMenu.Sub>
        <ContextMenu.SubTrigger className={menuItem}>
          <Star /> Rating
        </ContextMenu.SubTrigger>
        <ContextMenu.Portal>
          <ContextMenu.SubContent className={menuContent}>
            {[5, 4, 3, 2, 1, 0].map((n) => (
              <ContextMenu.Item
                key={n}
                className={menuItem}
                onSelect={() => actions.setRating(targets, n)}
              >
                <span className="flex size-4 items-center justify-center">
                  {targets.every((b) => b.user.rating === n) && <Check />}
                </span>
                {n === 0 ? "No rating" : `${n} star${n > 1 ? "s" : ""}`}
                <MenuShortcut>{keys(`books.rate${n}` as ActionId)}</MenuShortcut>
              </ContextMenu.Item>
            ))}
          </ContextMenu.SubContent>
        </ContextMenu.Portal>
      </ContextMenu.Sub>
      <ContextMenu.Item className={menuItem} onSelect={() => actions.toggleFavorite(targets)}>
        <Heart className={cn(allFavourite && "fill-current")} />
        {allFavourite ? "Remove from Favourites" : "Add to Favourites"}
        <MenuShortcut>{keys("books.favorite")}</MenuShortcut>
      </ContextMenu.Item>
      <ContextMenu.Item
        className={menuItem}
        onSelect={() => openCitation(targets.map((b) => b.id))}
      >
        <Quote /> {single ? "Cite…" : `Cite ${targets.length} books…`}
        <MenuShortcut>{keys("books.cite")}</MenuShortcut>
      </ContextMenu.Item>
      {editLibrary && (
        <ContextMenu.Item
          className={menuItem}
          onSelect={() => openExport(targets.map((b) => b.id))}
        >
          <Download /> {single ? "Export…" : `Export ${targets.length} books…`}
          <MenuShortcut>{keys("library.export")}</MenuShortcut>
        </ContextMenu.Item>
      )}
      {editLibrary && (
        <>
          <ContextMenu.Item className={menuItem} onSelect={() => openBulk(targets)}>
            <Pencil />{" "}
            {single ? "Edit details together…" : `Edit ${targets.length} books together…`}
            <MenuShortcut>{keys("books.bulkEdit")}</MenuShortcut>
          </ContextMenu.Item>
          {single ? (
            <ContextMenu.Item className={menuItem} onSelect={() => openFinder(book.id)}>
              <Globe /> Find details online…
              <MenuShortcut>{keys("details.find")}</MenuShortcut>
            </ContextMenu.Item>
          ) : (
            <ContextMenu.Item
              className={menuItem}
              onSelect={() => fillDetails(targets.map((b) => b.id))}
            >
              <Globe /> Fill in missing details online
              <MenuShortcut>{keys("details.fill")}</MenuShortcut>
            </ContextMenu.Item>
          )}
          {scans.length > 0 && (
            <ContextMenu.Item
              className={menuItem}
              onSelect={() => useOcrDialog.getState().open(scans.map((b) => b.id))}
            >
              <ScanText />{" "}
              {scans.length === 1
                ? "Make searchable (OCR)…"
                : `Make ${scans.length} books searchable…`}
            </ContextMenu.Item>
          )}
          <ContextMenu.Sub>
            <ContextMenu.SubTrigger className={menuItem}>
              <FolderInput /> Move to
            </ContextMenu.SubTrigger>
            <ContextMenu.Portal>
              <ContextMenu.SubContent className={cn(menuContent, "max-h-80 overflow-y-auto")}>
                <ContextMenu.Item
                  className={menuItem}
                  onSelect={() => void actions.moveTo(targets, "")}
                >
                  Books (top level)
                </ContextMenu.Item>
                {flattenFolders(folders).map(({ folder, depth }) => (
                  <ContextMenu.Item
                    key={folder.path}
                    className={menuItem}
                    style={{ paddingLeft: 8 + depth * 14 }}
                    onSelect={() => void actions.moveTo(targets, folder.path)}
                  >
                    {folder.name}
                  </ContextMenu.Item>
                ))}
              </ContextMenu.SubContent>
            </ContextMenu.Portal>
          </ContextMenu.Sub>
          <ContextMenu.Separator className={menuSeparator} />
          <ContextMenu.Item
            className={cn(menuItem, "text-destructive [&_svg]:!text-destructive")}
            onSelect={() => void actions.moveToTrash(targets)}
          >
            <Trash2 /> {single ? "Move to Trash" : `Move ${targets.length} books to Trash`}
            <MenuShortcut>
              {keys(platform === "mac" ? "books.trashMac" : "books.trash")}
            </MenuShortcut>
          </ContextMenu.Item>
        </>
      )}
    </>
  );
}

/**
 * Right-click menu for a book. Acts on the whole selection when the book is
 * part of it, otherwise selects just this book first.
 */
export function BookContextMenu({
  book,
  getBooks,
  children,
}: {
  book: BookView;
  /** The books on screen, read only when the menu opens. */
  getBooks: () => BookView[];
  children: ReactNode;
}) {
  const [targets, setTargets] = useState<BookView[] | null>(null);
  return (
    <ContextMenu.Root
      onOpenChange={(open) => {
        if (!open) return setTargets(null);
        const { selection, setSelection } = useLibraryView.getState();
        if (selection.includes(book.id)) {
          setTargets(getBooks().filter((b) => selection.includes(b.id)));
        } else {
          setSelection([book.id]);
          setTargets([book]);
        }
      }}
    >
      <ContextMenu.Trigger asChild>{children}</ContextMenu.Trigger>
      <ContextMenu.Portal>
        <ContextMenu.Content className={menuContent}>
          {targets && <MenuItems book={book} targets={targets} />}
        </ContextMenu.Content>
      </ContextMenu.Portal>
    </ContextMenu.Root>
  );
}
