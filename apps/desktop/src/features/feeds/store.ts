import { create } from "zustand";

/** What the list shows: everything, a folder (and what is inside it) or one feed. */
export type FeedPlace =
  { kind: "all" } | { kind: "folder"; id: string } | { kind: "feed"; id: string };

export type FeedShow = "all" | "unread" | "downloaded" | "library";

interface FeedsViewState {
  place: FeedPlace;
  show: FeedShow;
  topic: string | null;
  search: string;
  /** Folders shown closed. */
  closed: Record<string, boolean>;
  setPlace: (place: FeedPlace) => void;
  setShow: (show: FeedShow) => void;
  setTopic: (topic: string | null) => void;
  setSearch: (search: string) => void;
  toggle: (folder: string) => void;
}

export const useFeedsView = create<FeedsViewState>((set) => ({
  place: { kind: "all" },
  show: "all",
  topic: null,
  search: "",
  closed: {},
  setPlace: (place) => set({ place, topic: null }),
  setShow: (show) => set({ show }),
  setTopic: (topic) => set({ topic }),
  setSearch: (search) => set({ search }),
  toggle: (folder) => set((s) => ({ closed: { ...s.closed, [folder]: !s.closed[folder] } })),
}));
