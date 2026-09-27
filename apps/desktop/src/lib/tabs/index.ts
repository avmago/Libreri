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

/** Two books side by side. */
export interface Split {
  left: string;
  right: string;
}

interface TabsState {
  tabs: BookTab[];
  /** `null` = the library tab. */
  active: string | null;
  split: Split | null;
  /** The book shown most recently (for "open beside" from the library). */
  lastBook: string | null;
  /** Recently closed tabs, newest last (for "Reopen closed tab"). */
  closed: BookTab[];
  open: (tab: BookTab) => void;
  close: (bookId: string) => void;
  reopen: () => void;
  activate: (bookId: string | null) => void;
  cycle: (step: 1 | -1) => void;
  rename: (bookId: string, title: string, fileType?: FileType) => void;
  restore: (tabs: BookTab[], active: string | null, split?: Split | null) => void;
  clearJump: (bookId: string) => void;
  /** Splits the active book with the previous one, or ends the split. */
  toggleSplit: () => boolean;
  /** Moves the keyboard to the other side of the split. */
  swapSplitFocus: () => void;
  /** Opens `bookId` beside the active book. */
  openBeside: (tab: BookTab) => void;
  /** Removes a tab without remembering it (it moved to another window). */
  detach: (bookId: string) => void;
}

function withoutSplitOf(split: Split | null, bookId: string): Split | null {
  return split && (split.left === bookId || split.right === bookId) ? null : split;
}

export const useTabs = create<TabsState>((set, get) => ({
  tabs: [],
  active: null,
  split: null,
  lastBook: null,
  closed: [],
  open: (tab) =>
    set((s) => {
      const existing = s.tabs.find((t) => t.bookId === tab.bookId);
      if (existing) {
        return {
          active: tab.bookId,
          lastBook: tab.bookId,
          tabs: tab.jumpTo
            ? s.tabs.map((t) => (t.bookId === tab.bookId ? { ...t, jumpTo: tab.jumpTo } : t))
            : s.tabs,
        };
      }
      return { tabs: [...s.tabs, tab], active: tab.bookId, lastBook: tab.bookId };
    }),
  close: (bookId) =>
    set((s) => {
      const i = s.tabs.findIndex((t) => t.bookId === bookId);
      if (i < 0) return {};
      const tabs = s.tabs.filter((t) => t.bookId !== bookId);
      const partner =
        s.split &&
        (s.split.left === bookId ? s.split.right : s.split.right === bookId ? s.split.left : null);
      const active =
        s.active !== bookId
          ? s.active
          : (partner ?? tabs[Math.min(i, tabs.length - 1)]?.bookId ?? null);
      const closedTab: BookTab = { ...s.tabs[i]!, jumpTo: undefined };
      return {
        tabs,
        active,
        split: withoutSplitOf(s.split, bookId),
        closed: [...s.closed.filter((t) => t.bookId !== bookId), closedTab].slice(-20),
      };
    }),
  reopen: () => {
    const last = get().closed[get().closed.length - 1];
    if (!last) return;
    set((s) => ({ closed: s.closed.slice(0, -1) }));
    get().open(last);
  },
  activate: (active) => set((s) => ({ active, lastBook: active ?? s.lastBook })),
  cycle: (step) =>
    set((s) => {
      const order: (string | null)[] = [null, ...s.tabs.map((t) => t.bookId)];
      const i = order.indexOf(s.active);
      return { active: order[(i + step + order.length) % order.length] ?? null };
    }),
  rename: (bookId, title, fileType) =>
    set((s) => ({
      tabs: s.tabs.map((t) =>
        t.bookId === bookId ? { ...t, title, fileType: fileType ?? t.fileType } : t,
      ),
    })),
  restore: (tabs, active, split = null) => {
    const has = (id: string | null | undefined) => !!id && tabs.some((t) => t.bookId === id);
    set({
      tabs,
      active: has(active) ? active : null,
      split:
        split && has(split.left) && has(split.right) && split.left !== split.right ? split : null,
      closed: [],
    });
  },
  clearJump: (bookId) =>
    set((s) => ({
      tabs: s.tabs.map((t) => (t.bookId === bookId ? { ...t, jumpTo: undefined } : t)),
    })),
  toggleSplit: () => {
    const s = get();
    if (s.split) {
      set({ split: null });
      return true;
    }
    if (!s.active || s.tabs.length < 2) return false;
    const i = s.tabs.findIndex((t) => t.bookId === s.active);
    const other = s.tabs[i > 0 ? i - 1 : i + 1];
    if (!other) return false;
    set({ split: { left: s.active, right: other.bookId } });
    return true;
  },
  swapSplitFocus: () => {
    const { split, active } = get();
    if (!split) return;
    set({ active: active === split.left ? split.right : split.left });
  },
  openBeside: (tab) => {
    const s = get();
    const base = s.active ?? (s.tabs.some((t) => t.bookId === s.lastBook) ? s.lastBook : null);
    get().open(tab);
    if (base && base !== tab.bookId) set({ split: { left: base, right: tab.bookId } });
  },
  detach: (bookId) =>
    set((s) => {
      const tabs = s.tabs.filter((t) => t.bookId !== bookId);
      return {
        tabs,
        active: s.active === bookId ? (tabs[tabs.length - 1]?.bookId ?? null) : s.active,
        split: withoutSplitOf(s.split, bookId),
      };
    }),
}));
