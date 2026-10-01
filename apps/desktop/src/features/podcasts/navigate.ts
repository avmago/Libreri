import { useLibraryView } from "@/features/library";
import { useTabs } from "@/lib/tabs";
import { usePodcastsView, type PodcastPlace } from "./store";

/** Opens the Podcasts section (at a list or show). */
export function showPodcasts(place?: PodcastPlace) {
  useTabs.getState().activate(null);
  useLibraryView.getState().setNav({ kind: "podcasts" });
  if (place) usePodcastsView.getState().setPlace(place);
}
