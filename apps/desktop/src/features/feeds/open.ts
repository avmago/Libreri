import { toast } from "sonner";
import { commands, unwrap } from "@/lib/ipc";
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
