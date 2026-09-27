import { create } from "zustand";
import type { SettingsSection } from "@/features/settings";

/** App-wide view state. Data from Rust lives in TanStack Query, not here. */
interface UiState {
  paletteOpen: boolean;
  sidebarCollapsed: boolean;
  /** Settings is showing, at this section. */
  settings: SettingsSection | null;
  shortcutsOpen: boolean;
  setPaletteOpen: (open: boolean) => void;
  toggleSidebar: () => void;
  setSidebarCollapsed: (collapsed: boolean) => void;
  openSettings: (section?: SettingsSection) => void;
  closeSettings: () => void;
  setShortcutsOpen: (open: boolean) => void;
}

export const useUi = create<UiState>((set) => ({
  paletteOpen: false,
  sidebarCollapsed: false,
  settings: null,
  shortcutsOpen: false,
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
  setSidebarCollapsed: (sidebarCollapsed) => set({ sidebarCollapsed }),
  openSettings: (section) => set((s) => ({ settings: section ?? s.settings ?? "general" })),
  closeSettings: () => set({ settings: null }),
  setShortcutsOpen: (shortcutsOpen) => set({ shortcutsOpen }),
}));
