import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { commands, unwrap } from "@/lib/ipc";
import { feedsKey, useFeedsOverview } from "@/features/feeds";

const fail = (what: string) => (e: unknown) =>
  toast.error(what, { description: e instanceof Error ? e.message : String(e) });

export const podcastsKey = [...feedsKey, "podcasts"] as const;

export function usePodcasts(enabled = true) {
  return useFeedsOverview(enabled, "podcasts");
}

/** New episodes, for the sidebar. */
export function usePodcastsBadge(): number {
  return usePodcasts().data?.unread ?? 0;
}

/** The episodes in Up next, in order. */
export function useQueueEpisodes(queue: string[]) {
  return useQuery({
    queryKey: [...podcastsKey, "queue", queue],
    queryFn: () => unwrap(commands.podcastEpisodes(queue)),
    enabled: queue.length > 0,
    placeholderData: (prev) => prev,
  });
}

export function useSetQueue() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (ids: string[]) => unwrap(commands.podcastQueueSet(ids)),
    onError: fail("Could not change Up next"),
    onSettled: () => void qc.invalidateQueries({ queryKey: feedsKey }),
  });
}

export function useMarkPlayed() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: { ids: string[]; played: boolean }) =>
      unwrap(commands.podcastPlayed(a.ids, a.played)),
    onError: fail("Could not mark the episode"),
    onSettled: () => void qc.invalidateQueries({ queryKey: feedsKey }),
  });
}

/** Whether a Podcast Index key is saved on this computer. */
export function useIndexStatus() {
  return useQuery({
    queryKey: ["podcast-index"],
    queryFn: () => commands.podcastIndexStatus(),
    staleTime: Infinity,
  });
}

export function useSetIndexKey() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: { key: string; secret: string }) =>
      unwrap(commands.podcastIndexSet(a.key, a.secret)),
    onSuccess: (dto) => qc.setQueryData(["podcast-index"], dto),
  });
}

export function useSearchShows(query: string, index: boolean) {
  const q = query.trim();
  return useQuery({
    queryKey: ["podcast-search", index, q],
    queryFn: () => unwrap(commands.podcastSearch(q, index)),
    enabled: q.length > 1,
    staleTime: 10 * 60_000,
    retry: false,
  });
}

export function useTrending(category: string | null, enabled: boolean) {
  return useQuery({
    queryKey: ["podcast-trending", category],
    queryFn: () => unwrap(commands.podcastTrending(category)),
    enabled,
    staleTime: 30 * 60_000,
    retry: false,
  });
}

export function useCategories(enabled: boolean) {
  return useQuery({
    queryKey: ["podcast-categories"],
    queryFn: () => unwrap(commands.podcastCategories()),
    enabled,
    staleTime: Infinity,
    retry: false,
  });
}
