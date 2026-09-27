import { create } from "zustand";

/** The Find details dialog, opened from menus, shortcuts and the palette. */
export const useDetailsDialog = create<{
  /** The book being looked up, or null when closed. */
  bookId: string | null;
  open: (bookId: string) => void;
  close: () => void;
}>((set) => ({
  bookId: null,
  open: (bookId) => set({ bookId }),
  close: () => set({ bookId: null }),
}));
