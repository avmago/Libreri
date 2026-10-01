import { useEffect } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import {
  commands,
  events,
  unwrap,
  type FeedChange,
  type FeedSettings,
  type FolderChange,
  type ItemFilter,
  type Space,
} from "@/lib/ipc";

export const feedsKey = ["feeds"] as const;

export function useFeedsOverview(enabled = true, space: Space = "feeds") {
  return useQuery({
    queryKey: [...feedsKey, space, "overview"],
    queryFn: () => unwrap(commands.feedsOverview(space)),
    enabled,
  });
}

export function useFeedItems(filter: ItemFilter, space: Space = "feeds") {
  return useQuery({
    queryKey: [...feedsKey, space, "items", filter],
    queryFn: () => unwrap(commands.feedItems(space, filter)),
    placeholderData: (prev) => prev,
  });
}

export function useArxivCategories(enabled: boolean) {
  return useQuery({
    queryKey: [...feedsKey, "arxiv"],
    queryFn: () => commands.arxivCategories(),
    staleTime: Infinity,
    enabled,
  });
}

export function useSuggestedFeeds(enabled: boolean) {
  return useQuery({
    queryKey: [...feedsKey, "suggested"],
    queryFn: () => commands.suggestedFeeds(),
    staleTime: Infinity,
    enabled,
  });
}

function fail(what: string) {
  return (e: unknown) =>
    toast.error(what, { description: e instanceof Error ? e.message : String(e) });
}

/** A mutation that refreshes the feeds lists when it is done. */
function useFeedsMutation<A, R>(fn: (args: A) => Promise<R>, error: string) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onError: fail(error),
    onSettled: () => void qc.invalidateQueries({ queryKey: feedsKey }),
  });
}

/** Looks for new items (in the feeds given, or all). */
export function refreshFeeds(ids: string[] | null, quiet = false, space: Space = "feeds") {
  return unwrap(commands.feedsRefresh(space, ids)).then(
    (r) => {
      if (quiet || r.busy) return r;
      if (r.failed && !r.newItems)
        toast.error("Some feeds could not be read", {
          description: "Their addresses are marked in the list.",
        });
      else if (r.newItems)
        toast.success(r.newItems === 1 ? "1 new item" : `${r.newItems} new items`);
      else toast("Nothing new");
      return r;
    },
    (e) => {
      if (!quiet) fail("Could not check the feeds")(e);
      throw e;
    },
  );
}

export function useRefreshFeeds(space: Space = "feeds") {
  return useFeedsMutation(
    (ids: string[] | null) => refreshFeeds(ids, false, space),
    "Could not check for new items",
  );
}

export function useAddFeed(space: Space = "feeds") {
  return useFeedsMutation(
    (a: { url: string; title: string; folder: string | null; autoDownload: boolean }) =>
      unwrap(commands.feedAdd(space, a.url, a.title, a.folder, a.autoDownload)).then((id) => {
        void refreshFeeds([id], true, space);
        return id;
      }),
    "Could not follow the feed",
  );
}

export function useAddArxiv() {
  return useFeedsMutation(
    (a: { codes: string[]; search: string | null; autoDownload: boolean }) =>
      unwrap(commands.feedsAddArxiv(a.codes, a.search, a.autoDownload)).then((ids) => {
        if (ids.length) void refreshFeeds(ids, true);
        return ids;
      }),
    "Could not follow arXiv",
  );
}

export function useChangeFeed(space: Space = "feeds") {
  return useFeedsMutation(
    (a: { id: string; change: FeedChange }) => unwrap(commands.feedChange(space, a.id, a.change)),
    "Could not change the feed",
  );
}

export function useRemoveFeed(space: Space = "feeds") {
  return useFeedsMutation(
    (id: string) => unwrap(commands.feedRemove(space, id)),
    "Could not remove the feed",
  );
}

export function useAddFeedFolder(space: Space = "feeds") {
  return useFeedsMutation(
    (a: { name: string; parent: string | null }) =>
      unwrap(commands.feedFolderAdd(space, a.name, a.parent)),
    "Could not add the folder",
  );
}

export function useChangeFeedFolder(space: Space = "feeds") {
  return useFeedsMutation(
    (a: { id: string; change: FolderChange }) =>
      unwrap(commands.feedFolderChange(space, a.id, a.change)),
    "Could not change the folder",
  );
}

export function useRemoveFeedFolder(space: Space = "feeds") {
  return useFeedsMutation(
    (id: string) => unwrap(commands.feedFolderRemove(space, id)),
    "Could not remove the folder",
  );
}

export function useFeedSettings(space: Space = "feeds") {
  return useFeedsMutation(
    (s: FeedSettings) => unwrap(commands.feedsSettingsSet(space, s)),
    "Could not save the settings",
  );
}

export function useDownloadItem(space: Space = "feeds") {
  return useFeedsMutation(
    (id: string) => unwrap(commands.feedItemDownload(space, id)),
    "Could not download it",
  );
}

export function useDeleteItems(space: Space = "feeds") {
  return useFeedsMutation(
    (ids: string[]) => unwrap(commands.feedItemsDelete(space, ids)),
    "Could not delete it",
  );
}

export function useForgetFile(space: Space = "feeds") {
  return useFeedsMutation(
    (id: string) => unwrap(commands.feedItemForgetFile(space, id)),
    "Could not delete the download",
  );
}

export function useMarkRead(space: Space = "feeds") {
  return useFeedsMutation(
    (a: { ids: string[]; read: boolean }) => unwrap(commands.feedItemsRead(space, a.ids, a.read)),
    "Could not mark it",
  );
}

export function useMarkAllRead(space: Space = "feeds") {
  return useFeedsMutation(
    (a: { folder: string | null; feed: string | null }) =>
      unwrap(commands.feedAllRead(space, a.folder, a.feed)),
    "Could not mark them read",
  );
}

export function useAddToLibrary(space: Space = "feeds") {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (a: { id: string; folder: string }) =>
      unwrap(commands.feedItemToLibrary(space, a.id, a.folder)),
    onError: fail("Could not add it to the library"),
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: feedsKey });
      void qc.invalidateQueries({ queryKey: ["lib"] });
    },
  });
}

/**
 * Keeps the feeds lists current (they change in the background), and
 * looks for new items when the library opens and then every so often
 * while Libreri is open, as Feeds settings say.
 */
export function useFeedsBackground(active: boolean, space: Space = "feeds") {
  const qc = useQueryClient();
  const { data } = useFeedsOverview(active, space);
  const minutes = data?.settings.refreshMinutes ?? 60;
  const hasFeeds = (data?.feeds.length ?? 0) > 0;

  useEffect(() => {
    if (!active) return;
    let off: (() => void) | undefined;
    let gone = false;
    void events.feedsChanged
      .listen(() => void qc.invalidateQueries({ queryKey: feedsKey }))
      .then((f) => (gone ? f() : (off = f)));
    return () => {
      gone = true;
      off?.();
    };
  }, [active, qc]);

  useEffect(() => {
    if (!active || !hasFeeds) return;
    // Soon after opening, then on the schedule.
    const first = setTimeout(() => void refreshFeeds(null, true, space).catch(() => {}), 8000);
    const every =
      minutes > 0
        ? setInterval(() => void refreshFeeds(null, true, space).catch(() => {}), minutes * 60_000)
        : undefined;
    return () => {
      clearTimeout(first);
      if (every) clearInterval(every);
    };
  }, [active, hasFeeds, minutes, space]);
}
