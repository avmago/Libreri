import { create } from "zustand";
import type { BookView } from "./model";

/** Library dialogs that several places can open (menus, shortcuts, palette). */
export const useLibraryDialogs = create<{
  /** The books being edited together, or null when closed. */
  bulkEdit: BookView[] | null;
  saveCollection: boolean;
  openBulkEdit: (books: BookView[]) => void;
  closeBulkEdit: () => void;
  setSaveCollection: (open: boolean) => void;
}>((set) => ({
  bulkEdit: null,
  saveCollection: false,
  openBulkEdit: (books) => set({ bulkEdit: books.length ? books : null }),
  closeBulkEdit: () => set({ bulkEdit: null }),
  setSaveCollection: (saveCollection) => set({ saveCollection }),
}));
