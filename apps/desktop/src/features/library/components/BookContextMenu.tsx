import { useState, type ReactNode } from "react";
import {
  BookOpen,
  Check,
  ExternalLink,
  FolderInput,
  FolderSearch,
  Heart,
  Info,
  ListChecks,
  Trash2,
} from "lucide-react";
import {
  ContextMenu,
  MenuShortcut,
  menuContent,
  menuItem,
  menuSeparator,
} from "@/components/ui/menu";
import { DEFAULT_SHORTCUTS, displayKeys, platform } from "@/lib/shortcuts";
import type { ReadingStatus } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useFolders } from "../api";
import { useBookActions } from "../hooks/useBookActions";
import { STATUS_LABEL, type BookView } from "../model";
import { useLibraryView } from "../store";
import { flattenFolders } from "./folderUtils";

const keys = (id: keyof typeof DEFAULT_SHORTCUTS) =>
  displayKeys(DEFAULT_SHORTCUTS[id], platform).join(platform === "mac" ? "" : "+");

const STATUSES: ReadingStatus[] = ["wantToRead", "reading", "finished", "abandoned", "none"];

/** The menu itself; only mounted while open, so closed menus cost nothing. */
function MenuItems({ book, targets }: { book: BookView; targets: BookView[] }) {
  const actions = useBookActions();
  const setDetailsOpen = useLibraryView((s) => s.setDetailsOpen);
  const { data: folders = [] } = useFolders();
  const single = targets.length === 1;
  const allFavourite = targets.every((b) => b.user.favorite);

  return (
    <>
      {single && (
        <>
          <ContextMenu.Item className={menuItem} onSelect={() => void actions.open(book)}>
            <BookOpen /> Open <MenuShortcut>{keys("books.open")}</MenuShortcut>
          </ContextMenu.Item>
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
      <ContextMenu.Item className={menuItem} onSelect={() => actions.toggleFavorite(targets)}>
        <Heart className={cn(allFavourite && "fill-current")} />
        {allFavourite ? "Remove from Favourites" : "Add to Favourites"}
        <MenuShortcut>{keys("books.favorite")}</MenuShortcut>
      </ContextMenu.Item>
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
        <MenuShortcut>{keys(platform === "mac" ? "books.trashMac" : "books.trash")}</MenuShortcut>
      </ContextMenu.Item>
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
