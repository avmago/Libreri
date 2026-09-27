/**
 * Which window this is. The first window ("main") owns the saved tabs;
 * windows opened for one book carry it in their address and save nothing.
 */
import type { FileType } from "@/lib/ipc";
import type { BookTab } from "@/lib/tabs";

const params = new URLSearchParams(window.location.search);

/** The book this window was opened for, if it is a book window. */
export const initialTab: BookTab | null = params.get("book")
  ? {
      bookId: params.get("book")!,
      title: params.get("title") ?? "Book",
      fileType: (params.get("type") ?? "pdf") as FileType,
    }
  : null;

export const isMainWindow = initialTab === null && !params.has("blank");

/** Address for a new window showing one book, or an empty library view. */
export function windowUrl(tab: BookTab | null): string {
  const p = new URLSearchParams();
  if (tab) {
    p.set("book", tab.bookId);
    p.set("title", tab.title);
    p.set("type", tab.fileType);
  } else {
    p.set("blank", "1");
  }
  return `index.html?${p.toString()}`;
}
