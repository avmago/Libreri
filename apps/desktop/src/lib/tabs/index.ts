/**
 * Open tabs: the library plus one tab per open book. Lives in `lib` because
 * both the library (which opens books) and the reader (which shows them)
 * use it.
 */
import { create } from "zustand";
import type { FileType } from "@/lib/ipc";

export interface BookTab {
  /** One tab per book, so the book id is the tab id. */
  bookId: string;
  title: string;
  fileType: FileType;
  /** Where to go when the tab opens (an annotation id), used once. */
  jumpTo?: string;
}

interface TabsState {
  tabs: BookTab[];
  /** `null` = the library tab. */
  active: string | null;
  open: (tab: BookTab) => void;
  close: (bookId: string) => void;
  activate: (bookId: string | null) => void;
  cycle: (step: 1 | -1) => void;
  rename: (bookId: string, title: string) => void;
  restore: (tabs: BookTab[], active: string | null) => void;
  clearJump: (bookId: string) => void;
}

export const useTabs = create<TabsState>((set) => ({
  tabs: [],
  active: null,
  open: (tab) =>
    set((s) => {
      const existing = s.tabs.find((t) => t.bookId === tab.bookId);
      if (existing) {
        return {
          active: tab.bookId,
          tabs: tab.jumpTo
            ? s.tabs.map((t) => (t.bookId === tab.bookId ? { ...t, jumpTo: tab.jumpTo } : t))
            : s.tabs,
        };
      }
      return { tabs: [...s.tabs, tab], active: tab.bookId };
    }),
  close: (bookId) =>
    set((s) => {
      const i = s.tabs.findIndex((t) => t.bookId === bookId);
      if (i < 0) return {};
      const tabs = s.tabs.filter((t) => t.bookId !== bookId);
      const active =
        s.active !== bookId ? s.active : (tabs[Math.min(i, tabs.length - 1)]?.bookId ?? null);
      return { tabs, active };
    }),
  activate: (active) => set({ active }),
  cycle: (step) =>
    set((s) => {
      const order: (string | null)[] = [null, ...s.tabs.map((t) => t.bookId)];
      const i = order.indexOf(s.active);
      return { active: order[(i + step + order.length) % order.length] ?? null };
    }),
  rename: (bookId, title) =>
    set((s) => ({ tabs: s.tabs.map((t) => (t.bookId === bookId ? { ...t, title } : t)) })),
  restore: (tabs, active) =>
    set({ tabs, active: active && tabs.some((t) => t.bookId === active) ? active : null }),
  clearJump: (bookId) =>
    set((s) => ({
      tabs: s.tabs.map((t) => (t.bookId === bookId ? { ...t, jumpTo: undefined } : t)),
    })),
}));
