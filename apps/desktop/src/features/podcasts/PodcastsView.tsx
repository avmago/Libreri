import { useDeferredValue, useMemo, useState } from "react";
import { BarSkeleton, RowsSkeleton } from "@/components/Placeholders";
import { ask, save as pickSave } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  ArrowDown,
  ArrowUp,
  Check,
  CircleCheck,
  Clock,
  Download,
  ExternalLink,
  FileDown,
  FolderOpen,
  Inbox,
  Library,
  ListMinus,
  ListOrdered,
  ListPlus,
  Loader2,
  MoreHorizontal,
  Pause,
  Play,
  Plus,
  Podcast,
  RefreshCw,
  Search,
  Trash2,
  Unplug,
} from "lucide-react";
import { toast } from "sonner";
import { ResizablePanel } from "@/components/ResizablePanel";
import { SideOver, SideOverButton } from "@/components/SideOver";
import { useSideOver } from "@/lib/useSideOver";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  DropdownCheckItem,
  DropdownMenu,
  menuContent,
  menuItem,
  menuLabel,
  menuSeparator,
} from "@/components/ui/menu";
import {
  AddToLibraryDialog,
  openBook,
  useChangeFeed,
  useDeleteItems,
  useDownloadItem,
  useFeedItems,
  useFeedSettings,
  useForgetFile,
  useMarkAllRead,
  useRefreshFeeds,
  useRemoveFeed,
  whenLine,
} from "@/features/feeds";
import { usePermissions } from "@/features/profiles";
import {
  commands,
  unwrap,
  type FeedDto,
  type FeedItem,
  type FeedsDto,
  type ItemFilter,
} from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useMarkPlayed, usePodcasts, useQueueEpisodes, useSetQueue } from "./api";
import { FindPodcastsDialog, type FindTab } from "./FindPodcastsDialog";
import { heard, lengthLine, moveInQueue, RATES } from "./model";
import { Artwork } from "./PodcastPlayer";
import { usePlayer } from "./player";
import { usePodcastsView, type EpisodeShow, type PodcastPlace } from "./store";

const fail = (what: string) => (e: unknown) =>
  toast.error(what, { description: e instanceof Error ? e.message : String(e) });

const sentence = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

const LISTS: {
  id: Extract<PodcastPlace, { kind: "list" }>["id"];
  label: string;
  Icon: typeof Inbox;
}[] = [
  { id: "queue", label: "Up next", Icon: ListOrdered },
  { id: "new", label: "New episodes", Icon: Inbox },
  { id: "inProgress", label: "In progress", Icon: Clock },
  { id: "downloaded", label: "Downloaded", Icon: Download },
  { id: "all", label: "All episodes", Icon: Podcast },
];

const SHOWS: { id: EpisodeShow; label: string }[] = [
  { id: "all", label: "All" },
  { id: "unplayed", label: "Not played" },
  { id: "inProgress", label: "In progress" },
  { id: "downloaded", label: "Downloaded" },
];

const same = (a: PodcastPlace, b: PodcastPlace) => a.kind === b.kind && a.id === b.id;

/**
 * Podcasts (ADR 0028): the shows you follow on the left, with Up next and
 * the other lists; a show's episodes on the right. Episodes stream or are
 * downloaded, and play in the strip at the bottom of the window.
 */
export function PodcastsView() {
  const { data } = usePodcasts();
  const [finding, setFinding] = useState<FindTab | null>(null);
  // A narrow window: the shows slide over the episodes.
  const place = usePodcastsView((s) => s.place);
  const side = useSideOver(800, place);

  if (!data) {
    return (
      <div className="flex h-full">
        <div className="w-64 shrink-0 border-r bg-sidebar p-3">
          <RowsSkeleton
            rows={8}
            picture="square"
            label="Loading shows…"
            className="[&>div>div]:border-0 [&>div>div]:px-1 [&>div>div]:py-1.5"
          />
        </div>
        <RowsSkeleton rows={6} picture="round" label="Loading episodes…" className="flex-1" />
      </div>
    );
  }
  return (
    <div className="relative flex h-full min-h-0">
      {data.feeds.length === 0 ? (
        <Welcome onFind={setFinding} />
      ) : (
        <>
          <SideOver roomy={side.roomy} open={side.open} onClose={() => side.setOpen(false)}>
            <Shows data={data} onFind={() => setFinding("search")} />
          </SideOver>
          <Episodes data={data} onShows={side.roomy ? undefined : () => side.setOpen(true)} />
        </>
      )}
      {finding && <FindPodcastsDialog open tab={finding} onClose={() => setFinding(null)} />}
    </div>
  );
}

function Welcome({ onFind }: { onFind: (tab: FindTab) => void }) {
  return (
    <div className="mx-auto flex max-w-md flex-1 flex-col items-center justify-center gap-4 p-8 text-center">
      <Podcast className="size-10 text-muted-foreground" aria-hidden />
      <h1 className="text-[17px] font-semibold">Listen while you read</h1>
      <p className="text-muted-foreground">
        Follow podcasts to see their new episodes here. Play them streaming or downloaded, keep your
        place in each one, and keep listening while you read a book.
      </p>
      <div className="flex flex-wrap justify-center gap-2">
        <Button onClick={() => onFind("search")}>
          <Search /> Find podcasts
        </Button>
        <Button variant="outline" onClick={() => onFind("address")}>
          <Plus /> Add by address
        </Button>
        <Button variant="outline" onClick={() => onFind("opml")}>
          Import from another app
        </Button>
      </div>
    </div>
  );
}

function Shows({ data, onFind }: { data: FeedsDto; onFind: () => void }) {
  const { place, setPlace } = usePodcastsView();
  const refresh = useRefreshFeeds("podcasts");
  const settings = useFeedSettings("podcasts");
  const busy = data.refreshing || refresh.isPending;
  const s = data.settings;
  const shows = useMemo(
    () => [...data.feeds].sort((a, b) => a.title.localeCompare(b.title)),
    [data.feeds],
  );
  const counts: Record<string, number> = {
    queue: data.queue.length,
    new: data.unread,
    inProgress: data.inProgress,
    downloaded: data.downloaded,
    all: 0,
  };
  const exportOpml = async () => {
    const path = await pickSave({
      title: "Export podcasts",
      defaultPath: "Libreri podcasts.opml",
      filters: [{ name: "OPML", extensions: ["opml"] }],
    });
    if (!path) return;
    unwrap(commands.feedsExportOpml("podcasts", path)).then(
      () => toast.success("Podcasts exported"),
      fail("Could not export the podcasts"),
    );
  };
  return (
    <ResizablePanel id="podcasts.shows" initial={264} side="left" label="shows">
      <nav
        aria-label="Podcasts"
        className="flex min-w-0 flex-1 flex-col overflow-hidden border-r bg-sidebar"
      >
        <div className="flex items-center gap-1 px-3 pt-3 pb-2">
          <h1 className="flex-1 truncate text-[15px] font-semibold">Podcasts</h1>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Check for new episodes"
            title={busy ? "Checking…" : "Check every show for new episodes"}
            disabled={busy}
            onClick={() => refresh.mutate(null)}
          >
            <RefreshCw className={cn(busy && "animate-spin")} />
          </Button>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <Button variant="ghost" size="icon" aria-label="More">
                <MoreHorizontal />
              </Button>
            </DropdownMenu.Trigger>
            <DropdownMenu.Portal>
              <DropdownMenu.Content
                align="start"
                sideOffset={6}
                className={cn(menuContent, "w-72")}
              >
                <DropdownMenu.Item className={menuItem} onSelect={() => void exportOpml()}>
                  <FileDown /> Export to an OPML file…
                </DropdownMenu.Item>
                <DropdownMenu.Item
                  className={menuItem}
                  onSelect={() =>
                    void unwrap(commands.feedsReveal(null)).catch(fail("Could not show the folder"))
                  }
                >
                  <FolderOpen /> Show the downloads folder
                </DropdownMenu.Item>
                <DropdownMenu.Separator className={menuSeparator} />
                <DropdownMenu.Label className={menuLabel}>
                  CHECK FOR NEW EPISODES
                </DropdownMenu.Label>
                {(
                  [
                    [0, "Only when I press Refresh"],
                    [60, "Every hour"],
                    [180, "Every 3 hours"],
                    [720, "Every 12 hours"],
                  ] as const
                ).map(([m, label]) => (
                  <DropdownCheckItem
                    key={m}
                    checked={(s.refreshMinutes ?? 60) === m}
                    onCheckedChange={() =>
                      settings.mutate({ refreshMinutes: m, keepDays: s.keepDays ?? 30 })
                    }
                  >
                    {label}
                  </DropdownCheckItem>
                ))}
                <DropdownMenu.Separator className={menuSeparator} />
                <DropdownMenu.Label className={menuLabel}>
                  KEEP EPISODES NOT DOWNLOADED FOR
                </DropdownMenu.Label>
                {[30, 90, 365].map((d) => (
                  <DropdownCheckItem
                    key={d}
                    checked={(s.keepDays ?? 30) === d}
                    onCheckedChange={() =>
                      settings.mutate({ refreshMinutes: s.refreshMinutes ?? 60, keepDays: d })
                    }
                  >
                    {d === 365 ? "A year" : `${d} days`}
                  </DropdownCheckItem>
                ))}
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        </div>
        <div className="px-3 pb-2">
          <Button size="sm" className="w-full" onClick={onFind}>
            <Plus /> Find podcasts
          </Button>
        </div>
        <div className="flex min-h-0 flex-1 flex-col gap-px overflow-auto px-2 pb-2">
          {LISTS.map(({ id, label, Icon }) => (
            <PlaceRow
              key={id}
              active={same(place, { kind: "list", id })}
              onClick={() => setPlace({ kind: "list", id })}
              icon={<Icon className="size-4 text-muted-foreground" />}
              label={label}
              count={counts[id] ?? 0}
            />
          ))}
          <div className="px-2 pt-3 pb-1 text-[11px] font-medium tracking-wide text-muted-foreground">
            SHOWS
          </div>
          {shows.map((f) => (
            <PlaceRow
              key={f.id}
              active={same(place, { kind: "show", id: f.id })}
              onClick={() => setPlace({ kind: "show", id: f.id })}
              icon={<Artwork src={f.artwork} title={f.title} className="size-6 text-[9px]" />}
              label={f.title}
              count={f.unread}
              error={f.error}
            />
          ))}
        </div>
      </nav>
    </ResizablePanel>
  );
}

function PlaceRow({
  active,
  onClick,
  icon,
  label,
  count,
  error,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
  count: number;
  error?: string | null;
}) {
  return (
    <button
      type="button"
      aria-current={active ? "page" : undefined}
      onClick={onClick}
      title={error ? `${label} — ${sentence(error)}` : label}
      className={cn(
        "flex min-h-8 items-center gap-2.5 rounded-md px-2 py-1 text-left",
        active ? "bg-muted font-medium" : "hover:bg-muted/60",
      )}
    >
      {icon}
      <span className="min-w-0 flex-1 truncate">{label}</span>
      {error && <AlertCircle className="size-3.5 shrink-0 text-destructive" aria-label="Problem" />}
      <span className="text-[11px] text-muted-foreground">{count || ""}</span>
    </button>
  );
}

function Episodes({ data, onShows }: { data: FeedsDto; onShows?: () => void }) {
  const { place, show, setShow, search, setSearch, kept } = usePodcastsView();
  const q = useDeferredValue(search.trim());
  const feeds = useMemo(() => new Map(data.feeds.map((f) => [f.id, f])), [data.feeds]);
  const feed = place.kind === "show" ? (feeds.get(place.id) ?? null) : null;
  const queue = place.kind === "list" && place.id === "queue";
  const filter: ItemFilter = useMemo(() => {
    if (place.kind === "show")
      return { folder: null, feed: place.id, show, topic: null, search: q || null, keep: kept };
    const byList = {
      new: "unread",
      inProgress: "inProgress",
      downloaded: "downloaded",
      all: "all",
      queue: "all",
    } as const;
    return {
      folder: null,
      feed: null,
      show: byList[place.id],
      topic: null,
      search: q || null,
      keep: kept,
    };
  }, [place, show, q, kept]);
  const items = useFeedItems(filter, "podcasts");
  const queued = useQueueEpisodes(queue ? data.queue : []);
  const list: FeedItem[] = queue
    ? data.queue.length
      ? (queued.data ?? [])
      : []
    : (items.data?.items ?? []);
  const [adding, setAdding] = useState<FeedItem | null>(null);
  const markAll = useMarkAllRead("podcasts");

  if (place.kind === "show" && !feed) {
    return (
      <div className="flex flex-1 items-center justify-center text-muted-foreground">
        Choose a show on the left.
      </div>
    );
  }

  const title = feed?.title ?? LISTS.find((l) => l.id === place.id)?.label ?? "";
  const loading = queue ? queued.isLoading : items.isLoading;
  return (
    <main className="flex min-w-0 flex-1 flex-col">
      {onShows && <SideOverButton label="Shows and lists" onOpen={onShows} />}
      {feed ? (
        <ShowHeader feed={feed} />
      ) : (
        <div className="flex h-14 shrink-0 items-center gap-3 border-b px-5">
          <h2 className="flex-1 truncate text-[15px] font-semibold">{title}</h2>
          {place.id === "new" && data.unread > 0 && (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => markAll.mutate({ folder: null, feed: null })}
            >
              <Check /> Mark all as seen
            </Button>
          )}
        </div>
      )}
      {!queue && (
        <div className="flex shrink-0 flex-wrap items-center gap-2 border-b px-5 py-2">
          {feed && (
            <div role="tablist" aria-label="Show" className="flex gap-1">
              {SHOWS.map((s) => (
                <button
                  key={s.id}
                  type="button"
                  role="tab"
                  aria-selected={show === s.id}
                  onClick={() => setShow(s.id)}
                  className={cn(
                    "rounded-md px-2.5 py-1 text-[12.5px] font-medium text-muted-foreground",
                    show === s.id ? "bg-muted text-foreground" : "hover:text-foreground",
                  )}
                >
                  {s.label}
                </button>
              ))}
            </div>
          )}
          <div className="relative ml-auto w-64">
            <Search className="absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              aria-label="Find in episodes"
              placeholder="Find in episodes"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="h-8 pl-8"
            />
          </div>
        </div>
      )}
      <div className="min-h-0 flex-1 overflow-auto">
        {loading ? (
          <RowsSkeleton rows={6} picture="round" label="Loading episodes…" />
        ) : list.length === 0 ? (
          <p className="px-5 py-10 text-center text-muted-foreground">
            {queue
              ? "Nothing up next. Add episodes with “Up next”, and they play one after another."
              : q
                ? "No episodes match."
                : feed
                  ? "No episodes here."
                  : place.id === "new"
                    ? "No new episodes."
                    : "No episodes here."}
          </p>
        ) : (
          <ul aria-label="Episodes">
            {list.map((it, i) => (
              <EpisodeRow
                key={it.id}
                item={it}
                feed={feeds.get(it.feed) ?? null}
                showSource={!feed}
                queue={data.queue}
                queueIndex={queue ? i : null}
                downloading={data.downloading.includes(it.id)}
                onAdd={setAdding}
              />
            ))}
          </ul>
        )}
        {!queue && items.data && items.data.total > list.length && (
          <p className="px-5 py-3 text-center text-[12px] text-muted-foreground">
            Showing {list.length} of {items.data.total}. Search to find older episodes.
          </p>
        )}
      </div>
      <AddToLibraryDialog space="podcasts" item={adding} onClose={() => setAdding(null)} />
    </main>
  );
}

function ShowHeader({ feed }: { feed: FeedDto }) {
  const change = useChangeFeed("podcasts");
  const remove = useRemoveFeed("podcasts");
  const refresh = useRefreshFeeds("podcasts");
  const setPlace = usePodcastsView((s) => s.setPlace);
  const unfollow = async () => {
    const ok = await ask(
      `New episodes stop coming in. Episodes you downloaded or added to your library are kept.`,
      { title: `Unfollow “${feed.title}”?`, okLabel: "Unfollow" },
    );
    if (ok)
      remove.mutate(feed.id, {
        onSuccess: () => setPlace({ kind: "list", id: "new" }),
      });
  };
  const set = (c: Partial<{ autoDownload: boolean; speed: number }>) =>
    change.mutate({
      id: feed.id,
      change: {
        title: null,
        folder: null,
        autoDownload: c.autoDownload ?? null,
        speed: c.speed ?? null,
      },
    });
  return (
    <div className="flex shrink-0 gap-4 border-b px-5 py-4">
      <Artwork src={feed.artwork} title={feed.title} className="size-24 text-[22px]" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <h2 className="line-clamp-2 text-[17px] leading-tight font-semibold" title={feed.title}>
          {feed.title}
        </h2>
        <p className="truncate text-[12.5px] text-muted-foreground">
          {[feed.author, feed.total === 1 ? "1 episode" : `${feed.total} episodes`]
            .filter(Boolean)
            .join(" · ")}
          {feed.checkedAt ? ` · checked ${whenLine(feed.checkedAt).toLowerCase()}` : ""}
        </p>
        {feed.error && (
          <p className="flex items-center gap-1.5 text-[12px] text-destructive">
            <AlertCircle className="size-3.5 shrink-0" /> {sentence(feed.error)}
          </p>
        )}
        <div className="mt-1 flex flex-wrap items-center gap-2">
          <Button variant="outline" size="sm" onClick={() => void unfollow()}>
            <Check /> Following
          </Button>
          <label className="flex items-center gap-1.5 text-[12.5px]">
            <input
              type="checkbox"
              checked={feed.autoDownload}
              onChange={(e) => set({ autoDownload: e.target.checked })}
            />
            Download new episodes
          </label>
          <label className="flex items-center gap-1.5 text-[12.5px]">
            <span className="text-muted-foreground">Speed</span>
            <select
              aria-label="Speed for this show"
              value={feed.speed ?? 0}
              onChange={(e) => set({ speed: Number(e.target.value) })}
              className="h-7 rounded-md border bg-background px-1.5"
            >
              <option value={0}>Usual</option>
              {RATES.map((r) => (
                <option key={r} value={r}>
                  {r}×
                </option>
              ))}
            </select>
          </label>
          <DropdownMenu.Root>
            <DropdownMenu.Trigger asChild>
              <Button variant="ghost" size="icon" aria-label={`${feed.title} options`}>
                <MoreHorizontal />
              </Button>
            </DropdownMenu.Trigger>
            <DropdownMenu.Portal>
              <DropdownMenu.Content align="start" sideOffset={6} className={menuContent}>
                <DropdownMenu.Item className={menuItem} onSelect={() => refresh.mutate([feed.id])}>
                  <RefreshCw /> Check for new episodes
                </DropdownMenu.Item>
                <DropdownMenu.Item
                  className={menuItem}
                  onSelect={() => void navigator.clipboard.writeText(feed.url)}
                >
                  <FileDown /> Copy the RSS address
                </DropdownMenu.Item>
                <DropdownMenu.Separator className={menuSeparator} />
                <DropdownMenu.Item className={menuItem} onSelect={() => void unfollow()}>
                  <Unplug /> Unfollow…
                </DropdownMenu.Item>
              </DropdownMenu.Content>
            </DropdownMenu.Portal>
          </DropdownMenu.Root>
        </div>
      </div>
    </div>
  );
}

function EpisodeRow({
  item: it,
  feed,
  showSource,
  queue,
  queueIndex,
  downloading,
  onAdd,
}: {
  item: FeedItem;
  feed: FeedDto | null;
  showSource: boolean;
  queue: string[];
  /** Its place in Up next, when that list is shown. */
  queueIndex: number | null;
  downloading: boolean;
  onAdd: (it: FeedItem) => void;
}) {
  const [open, setOpen] = useState(false);
  const { editLibrary } = usePermissions();
  const current = usePlayer((s) => s.episode?.id === it.id);
  const playing = usePlayer((s) => s.episode?.id === it.id && s.playing);
  const play = usePlayer((s) => s.play);
  const keep = usePodcastsView((s) => s.keep);
  const download = useDownloadItem("podcasts");
  const forget = useForgetFile("podcasts");
  const remove = useDeleteItems("podcasts");
  const played = useMarkPlayed();
  const setQueue = useSetQueue();
  const busy = downloading || download.isPending;
  const inQueue = queue.includes(it.id);
  const fraction = heard(it);
  const unread = !it.read;
  const length = lengthLine(it);
  return (
    <li
      className={cn(
        "group flex gap-3 border-b px-5 py-3 hover:bg-muted/40",
        current && "bg-muted/50",
      )}
    >
      <Button
        variant={current ? "default" : "outline"}
        size="icon"
        className="mt-0.5 size-9 shrink-0 rounded-full"
        aria-label={playing ? `Pause “${it.title}”` : `Play “${it.title}”`}
        disabled={!it.audio && !it.file}
        onClick={() => {
          keep(it.id);
          play(it, feed);
        }}
      >
        {playing ? <Pause /> : <Play />}
      </Button>
      {showSource && (
        <Artwork src={feed?.artwork} title={it.source} className="mt-0.5 size-9 text-[11px]" />
      )}
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <button
          type="button"
          onClick={() => setOpen((o) => !o)}
          aria-expanded={open}
          className={cn(
            "flex items-start gap-2 text-left leading-snug",
            unread ? "font-semibold" : "font-medium",
            it.played && "text-muted-foreground",
          )}
        >
          {unread && (
            <span className="mt-1.5 size-2 shrink-0 rounded-full bg-primary" aria-label="New" />
          )}
          <span>{it.title}</span>
        </button>
        <p className="flex flex-wrap items-center gap-x-1.5 text-[12px] text-muted-foreground">
          {showSource && <span className="font-medium text-foreground/80">{it.source}</span>}
          {showSource && <span aria-hidden>·</span>}
          <span>{whenLine(it.published ?? it.foundAt)}</span>
          {length && <span aria-hidden>·</span>}
          {length && <span>{length}</span>}
          {fraction !== null && fraction < 1 && (
            <span
              className="inline-block h-1 w-14 overflow-hidden rounded-full bg-muted"
              role="progressbar"
              aria-label="Heard"
              aria-valuenow={Math.round(fraction * 100)}
            >
              <span
                className="block h-full bg-foreground/60"
                style={{ width: `${fraction * 100}%` }}
              />
            </span>
          )}
          {it.file && (
            <span className="flex items-center gap-1">
              · <CircleCheck className="size-3" /> Downloaded
            </span>
          )}
          {it.transcript && (
            <span className="rounded border px-1.5 py-px text-[10.5px]">Transcript</span>
          )}
          {it.chapters && (
            <span className="rounded border px-1.5 py-px text-[10.5px]">Chapters</span>
          )}
        </p>
        {it.summary && (
          <p
            className={cn(
              "text-[12.5px] leading-relaxed whitespace-pre-line text-muted-foreground",
              !open && "line-clamp-2",
            )}
          >
            {it.summary}
          </p>
        )}
        {busy && <BarSkeleton label="Downloading…" className="max-w-sm pt-1" />}
        {it.downloadError && !it.file && (
          <p className="flex items-center gap-1.5 text-[12px] text-destructive">
            <AlertCircle className="size-3.5 shrink-0" /> {sentence(it.downloadError)}
          </p>
        )}
      </div>
      <div className="flex shrink-0 items-start gap-1">
        {queueIndex !== null ? (
          <>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Move up"
              disabled={queueIndex === 0}
              onClick={() => setQueue.mutate(moveInQueue(queue, it.id, -1))}
            >
              <ArrowUp />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Move down"
              disabled={queueIndex === queue.length - 1}
              onClick={() => setQueue.mutate(moveInQueue(queue, it.id, 1))}
            >
              <ArrowDown />
            </Button>
          </>
        ) : (
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              setQueue.mutate(inQueue ? queue.filter((q) => q !== it.id) : [...queue, it.id])
            }
            title={inQueue ? "Take it out of Up next" : "Play it after what is playing"}
          >
            {inQueue ? <ListMinus /> : <ListPlus />} {inQueue ? "Queued" : "Up next"}
          </Button>
        )}
        {it.book ? (
          <Button variant="outline" size="sm" onClick={() => void openBook(it.book!)}>
            <Library /> In library
          </Button>
        ) : !it.file && it.audio ? (
          <Button
            variant="outline"
            size="icon"
            aria-label={busy ? "Downloading" : "Download"}
            title={busy ? "Downloading…" : "Download to listen offline"}
            disabled={busy}
            onClick={() =>
              download.mutate(it.id, {
                onSuccess: () => toast.success("Downloaded", { description: it.title }),
              })
            }
          >
            {busy ? <Loader2 className="animate-spin" /> : <Download />}
          </Button>
        ) : null}
        <DropdownMenu.Root>
          <DropdownMenu.Trigger asChild>
            <Button variant="ghost" size="icon" aria-label={`More for “${it.title}”`}>
              <MoreHorizontal />
            </Button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Portal>
            <DropdownMenu.Content align="end" sideOffset={6} className={menuContent}>
              <DropdownMenu.Item
                className={menuItem}
                onSelect={() => played.mutate({ ids: [it.id], played: !it.played })}
              >
                <CircleCheck /> {it.played ? "Mark as not played" : "Mark as played"}
              </DropdownMenu.Item>
              {queueIndex !== null && (
                <DropdownMenu.Item
                  className={menuItem}
                  onSelect={() => setQueue.mutate(queue.filter((q) => q !== it.id))}
                >
                  <ListMinus /> Take out of Up next
                </DropdownMenu.Item>
              )}
              {it.link && (
                <DropdownMenu.Item
                  className={menuItem}
                  onSelect={() => void commands.openExternalUrl(it.link!)}
                >
                  <ExternalLink /> Open the episode's page
                </DropdownMenu.Item>
              )}
              {it.file && !it.book && editLibrary && (
                <DropdownMenu.Item className={menuItem} onSelect={() => onAdd(it)}>
                  <Library /> Add to library as an audiobook…
                </DropdownMenu.Item>
              )}
              {it.file && (
                <DropdownMenu.Item
                  className={menuItem}
                  onSelect={() =>
                    void unwrap(commands.feedsReveal(it.file)).catch(fail("Could not show it"))
                  }
                >
                  <FolderOpen /> Show in folder
                </DropdownMenu.Item>
              )}
              {it.file && !it.book && (
                <DropdownMenu.Item className={menuItem} onSelect={() => forget.mutate(it.id)}>
                  <Trash2 /> Delete the download only
                </DropdownMenu.Item>
              )}
              <DropdownMenu.Separator className={menuSeparator} />
              <DropdownMenu.Item
                className={menuItem}
                disabled={busy || current}
                onSelect={() => remove.mutate([it.id])}
              >
                <Trash2 /> Delete the episode
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
      </div>
    </li>
  );
}
