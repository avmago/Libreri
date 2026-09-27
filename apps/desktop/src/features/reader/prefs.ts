import { create } from "zustand";
import type { PageThemeId, PdfDarkMode } from "@/readers";

/** How pages look. Saved with the session (per library and profile). */
export interface ReaderPrefs {
  theme: PageThemeId;
  /** Use the Night page theme while the app is dark. */
  followApp: boolean;
  pdfMode: PdfDarkMode;
}

interface PrefsState extends ReaderPrefs {
  set: (change: Partial<ReaderPrefs>) => void;
}

export const DEFAULT_PREFS: ReaderPrefs = {
  theme: "original",
  followApp: true,
  pdfMode: "recolour",
};

export const useReaderPrefs = create<PrefsState>((set) => ({
  ...DEFAULT_PREFS,
  set: (change) => set(change),
}));
