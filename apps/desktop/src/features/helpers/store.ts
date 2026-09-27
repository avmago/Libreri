import { create } from "zustand";
import type { Helper } from "@/lib/ipc";

/** The install dialog for a helper program, opened from anywhere. */
export const useHelperDialog = create<{
  helper: Helper | null;
  /** Called once the helper is installed (e.g. to reopen a book). */
  onInstalled: (() => void) | null;
  open: (helper: Helper, onInstalled?: () => void) => void;
  close: () => void;
}>((set) => ({
  helper: null,
  onInstalled: null,
  open: (helper, onInstalled) => set({ helper, onInstalled: onInstalled ?? null }),
  close: () => set({ helper: null, onInstalled: null }),
}));
