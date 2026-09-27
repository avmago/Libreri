import { create } from "zustand";

/** Dialogs of export, citations, archive import and the health check. */
export const usePortability = create<{
  /** Export dialog: `null` = closed; `bookIds` null = the whole library. */
  exporting: { bookIds: string[] | null } | null;
  /** Books whose citations are shown. */
  citing: string[] | null;
  /** The archive being imported. */
  importing: string | null;
  health: boolean;
  /** Bumped to ask the app to show the "Missing files" list. */
  showMissingRequest: number;
  showMissing: () => void;
  openExport: (bookIds: string[] | null) => void;
  openCitation: (ids: string[]) => void;
  openImport: (path: string) => void;
  setHealth: (open: boolean) => void;
  close: () => void;
}>((set) => ({
  exporting: null,
  citing: null,
  importing: null,
  health: false,
  showMissingRequest: 0,
  showMissing: () =>
    set((s) => ({
      showMissingRequest: s.showMissingRequest + 1,
      exporting: null,
      citing: null,
      importing: null,
      health: false,
    })),
  openExport: (bookIds) => set({ exporting: { bookIds } }),
  openCitation: (ids) => set({ citing: ids.length ? ids : null }),
  openImport: (importing) => set({ importing }),
  setHealth: (health) => set({ health }),
  close: () => set({ exporting: null, citing: null, importing: null, health: false }),
}));
