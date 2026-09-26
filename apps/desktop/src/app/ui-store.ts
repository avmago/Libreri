import { create } from "zustand";

/** App-wide view state. Data from Rust lives in TanStack Query, not here. */
interface UiState {
  paletteOpen: boolean;
  sidebarCollapsed: boolean;
  setPaletteOpen: (open: boolean) => void;
  toggleSidebar: () => void;
}

export const useUi = create<UiState>((set) => ({
  paletteOpen: false,
  sidebarCollapsed: false,
  setPaletteOpen: (paletteOpen) => set({ paletteOpen }),
  toggleSidebar: () => set((s) => ({ sidebarCollapsed: !s.sidebarCollapsed })),
}));
