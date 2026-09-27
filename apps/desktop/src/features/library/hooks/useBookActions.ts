import { useCallback } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { commands, unwrap, type ReadingStatus } from "@/lib/ipc";
import { useProfilePrefs } from "@/features/profiles";
import { useTabs } from "@/lib/tabs";
import { canRead } from "@/readers";
import { useMoveBooks, useSetBookState, useTrashBooks } from "../api";
import type { BookView } from "../model";
import { useLibraryView } from "../store";

function errorText(e: unknown) {
  return e instanceof Error ? e.message : String(e);
}

/** Actions on one or more books, shared by menus, the details panel and shortcuts. */
export function useBookActions() {
  const trash = useTrashBooks();
  const move = useMoveBooks();
  const setState = useSetBookState();
  const setSelection = useLibraryView((s) => s.setSelection);

  const openElsewhere = useCallback(async (book: BookView) => {
    try {
      await unwrap(commands.openBookExternally(book.id));
    } catch (e) {
      toast.error("Could not open the book", { description: errorText(e) });
    }
  }, []);

  /** Opens the book in a reader tab, or in another app for formats Libreri cannot show yet. */
  const open = useCallback(
    async (book: BookView) => {
      if (canRead(book.fileType) && !book.missing) {
        useTabs.getState().open({
          bookId: book.id,
          title: book.metadata.title,
          fileType: book.fileType,
        });
        return;
      }
      await openElsewhere(book);
    },
    [openElsewhere],
  );

  const reveal = useCallback(async (book: BookView) => {
    try {
      await unwrap(commands.revealBook(book.id));
    } catch (e) {
      toast.error("Could not show the file", { description: errorText(e) });
    }
  }, []);

  const moveToTrash = useCallback(
    async (books: BookView[]) => {
      if (books.length === 0) return;
      const what = books.length === 1 ? `“${books[0]?.metadata.title}”` : `${books.length} books`;
      const confirm = useProfilePrefs.getState().prefs.library.confirmTrash;
      const ok =
        !confirm ||
        (await ask(
          `The ${books.length === 1 ? "file goes" : "files go"} to the system Trash. If you restore ${
            books.length === 1 ? "it" : "them"
          } from there, ${books.length === 1 ? "its" : "their"} details come back too.`,
          { title: `Move ${what} to the Trash?`, kind: "warning", okLabel: "Move to Trash" },
        ));
      if (!ok) return;
      try {
        const n = await trash.mutateAsync(books.map((b) => b.id));
        setSelection([]);
        toast.success(n === 1 ? "Moved 1 book to the Trash" : `Moved ${n} books to the Trash`);
      } catch (e) {
        toast.error("Could not move to the Trash", { description: errorText(e) });
      }
    },
    [trash, setSelection],
  );

  const moveTo = useCallback(
    async (books: BookView[], folder: string) => {
      try {
        const n = await move.mutateAsync({ ids: books.map((b) => b.id), folder });
        const where = folder === "" ? "Books" : folder.split("/").pop();
        if (n > 0) toast.success(`Moved ${n === 1 ? "1 book" : `${n} books`} to ${where}`);
      } catch (e) {
        toast.error("Could not move", { description: errorText(e) });
      }
    },
    [move],
  );

  const setStatus = useCallback(
    (books: BookView[], status: ReadingStatus) => {
      for (const b of books) setState.mutate({ id: b.id, user: { ...b.user, status } });
    },
    [setState],
  );

  const toggleFavorite = useCallback(
    (books: BookView[]) => {
      const favorite = !books.every((b) => b.user.favorite);
      for (const b of books) setState.mutate({ id: b.id, user: { ...b.user, favorite } });
    },
    [setState],
  );

  const setRating = useCallback(
    (books: BookView | BookView[], rating: number) => {
      for (const b of Array.isArray(books) ? books : [books]) {
        setState.mutate({ id: b.id, user: { ...b.user, rating } });
      }
    },
    [setState],
  );

  /** Opens the book beside the book being read (split view). */
  const openBeside = useCallback((book: BookView) => {
    if (!canRead(book.fileType) || book.missing) return;
    useTabs.getState().openBeside({
      bookId: book.id,
      title: book.metadata.title,
      fileType: book.fileType,
    });
  }, []);

  return {
    open,
    openBeside,
    openElsewhere,
    reveal,
    moveToTrash,
    moveTo,
    setStatus,
    toggleFavorite,
    setRating,
  };
}
