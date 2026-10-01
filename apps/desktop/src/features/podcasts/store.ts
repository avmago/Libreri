import { create } from "zustand";

/** A list (Up next, New, Downloaded, In progress, Everything) or one show. */
export type PodcastPlace =
  | { kind: "list"; id: "queue" | "new" | "downloaded" | "inProgress" | "all" }
  | { kind: "show"; id: string };

export type EpisodeShow = "all" | "unplayed" | "downloaded" | "inProgress";

interface PodcastsViewState {
  place: PodcastPlace;
  show: EpisodeShow;
  search: string;
  /** Episodes played from this list: still shown under New until it changes. */
  kept: string[];
  keep: (id: string) => void;
  setPlace: (place: PodcastPlace) => void;
  setShow: (show: EpisodeShow) => void;
  setSearch: (search: string) => void;
}

export const usePodcastsView = create<PodcastsViewState>((set) => ({
  place: { kind: "list", id: "new" },
  show: "all",
  search: "",
  kept: [],
  keep: (id) => set((s) => (s.kept.includes(id) ? s : { kept: [...s.kept, id] })),
  setPlace: (place) => set({ place, show: "all", search: "", kept: [] }),
  setShow: (show) => set({ show, kept: [] }),
  setSearch: (search) => set({ search }),
}));
