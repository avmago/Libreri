import { toast } from "sonner";
import { commands, unwrap, type FeedItem, type Space } from "@/lib/ipc";
import { useTabs } from "@/lib/tabs";

/** Opens a book of the library in a tab. */
export async function openBook(bookId: string) {
  try {
    const book = await unwrap(commands.getBook(bookId));
    useTabs.getState().open({
      bookId,
      title: book.metadata.title || "Book",
      fileType: book.fileType,
    });
  } catch (e) {
    toast.error("Could not open the book", {
      description: e instanceof Error ? e.message : String(e),
    });
  }
}

/** Reads a download (PDF or article) in a tab, with the library's reader. */
export function openFeedDoc(item: FeedItem, space: Space) {
  if (!item.file) return;
  const pdf = item.file.toLowerCase().endsWith(".pdf");
  useTabs.getState().open({
    bookId: `feed:${item.id}`,
    title: item.title,
    fileType: pdf ? "pdf" : "md",
    feed: { space, id: item.id, file: item.file, source: item.source },
  });
}
