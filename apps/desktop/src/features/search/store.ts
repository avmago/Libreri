import { create } from "zustand";

/** Background indexing, as reported by the app. */
export const useIndexing = create<{
  done: number;
  total: number;
  running: boolean;
  set: (p: { done: number; total: number; running: boolean }) => void;
}>((set) => ({
  done: 0,
  total: 0,
  running: false,
  set: (p) => set(p),
}));

/** The "Make searchable" dialog, opened from anywhere. */
export const useOcrDialog = create<{
  ids: string[] | null;
  open: (ids: string[]) => void;
  close: () => void;
}>((set) => ({
  ids: null,
  open: (ids) => set({ ids }),
  close: () => set({ ids: null }),
}));
