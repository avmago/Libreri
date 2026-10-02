import { useDeferredValue, useMemo, useState } from "react";
import { BarSkeleton, RowsSkeleton } from "@/components/Placeholders";
import { save as pickSave } from "@tauri-apps/plugin-dialog";
import {
  AlertCircle,
  BookOpen,
  BookPlus,
  Check,
  CheckCheck,
  ChevronDown,
  ChevronRight,
  Download,
  ExternalLink,
  FileDown,
  FileText,
  FolderInput,
  FolderOpen,
  FolderPlus,
  Folder as FolderIcon,
  Inbox,
  Library,
  Loader2,
  MoreHorizontal,
  Pencil,
  Plus,
  RefreshCw,
  Rss,
  Search,
  Trash2,
  Unplug,
  Share2,
  Link2,
  Copy,
  Quote,
  Mail,
} from "lucide-react";
import { toast } from "sonner";
import { ResizablePanel } from "@/components/ResizablePanel";
import { SideOver, SideOverButton } from "@/components/SideOver";
import { useSideOver } from "@/lib/useSideOver";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import {
  DropdownCheckItem,
  DropdownMenu,
  menuContent,
  menuItem,
  menuLabel,
  menuSeparator,
} from "@/components/ui/menu";
import { usePermissions } from "@/features/profiles";
import { commands, unwrap, type FeedDto, type FeedItem, type FeedsDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import {
  useAddFeedFolder,
  useChangeFeed,
  useChangeFeedFolder,
  useDeleteItems,
  useDownloadItem,
  useFeedItems,
  useFeedSettings,
  useFeedsOverview,
  useForgetFile,
  useMarkAllRead,
  useMarkRead,
  useRefreshFeeds,
  useRemoveFeed,
  useRemoveFeedFolder,
} from "../api";
import { announceLabel, authorsLine, feedTree, whenLine, type TreeFolder } from "../model";
import { useFeedsView, type FeedPlace, type FeedShow } from "../store";
import { AddFeedDialog, FolderSelect, type AddTab } from "./AddFeedDialog";
import { AddToLibraryDialog } from "./ItemDialogs";
import { openBook, openFeedDoc } from "../open";
import { shareBibtex, shareMailto, shareMarkdown, shareText, shareUrl } from "../share";

const fail = (what: string) => (e: unknown) =>
  toast.error(what, { description: e instanceof Error ? e.message : String(e) });

/** Rust's messages start lower-case, to read well inside other text. */
const sentence = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

const SHOWS: { id: FeedShow; label: string }[] = [
  { id: "all", label: "All" },
  { id: "unread", label: "New" },
  { id: "downloaded", label: "Downloaded" },
  { id: "library", label: "In library" },
];

type Ask =
  | { kind: "newFolder"; parent: string | null }
  | { kind: "renameFolder"; id: string; name: string }
  | { kind: "renameFeed"; id: string; name: string }
  | { kind: "moveFeed"; feed: FeedDto }
  | { kind: "moveFolder"; id: string; parent: string | null };

/**
 * Feeds (ADR 0027): subscriptions in folders on the left; what came in on
 * the right, each with Download and Delete, and Add to library once kept.
 */
export function FeedsView() {
  const { data } = useFeedsOverview();
  const [adding, setAdding] = useState<{ tab: AddTab; folder: string | null } | null>(null);
  const [ask, setAsk] = useState<Ask | null>(null);
  // A narrow window: folders and feeds slide over the items.
  const place = useFeedsView((s) => s.place);
  const side = useSideOver(800, place);

  if (!data) {
    return (
      <div className="flex h-full">
        <div className="w-64 shrink-0 border-r bg-sidebar p-3">
          <RowsSkeleton
            rows={8}
            label="Loading feeds…"
            className="[&>div>div]:border-0 [&>div>div]:px-1 [&>div>div]:py-1.5"
          />
        </div>
        <RowsSkeleton rows={7} label="Loading items…" className="flex-1" />
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      <Header data={data} onAdd={(tab) => setAdding({ tab, folder: null })} onAsk={setAsk} />
      {data.feeds.length === 0 ? (
        <Welcome onAdd={(tab) => setAdding({ tab, folder: null })} />
      ) : (
        <div className="relative flex min-h-0 flex-1">
          <SideOver roomy={side.roomy} open={side.open} onClose={() => side.setOpen(false)}>
            <Tree
              data={data}
              onAsk={setAsk}
              onAdd={(folder) => setAdding({ tab: "address", folder })}
            />
          </SideOver>
          <Items data={data} onFeeds={side.roomy ? undefined : () => side.setOpen(true)} />
        </div>
      )}
      {adding && (
        <AddFeedDialog
          open
          tab={adding.tab}
          folder={adding.folder}
          onClose={() => setAdding(null)}
        />
      )}
      {ask && <AskDialog ask={ask} data={data} onClose={() => setAsk(null)} />}
    </div>
  );
}

function Header({
  data,
  onAdd,
  onAsk,
}: {
  data: FeedsDto;
  onAdd: (tab: AddTab) => void;
  onAsk: (a: Ask) => void;
}) {
  const refresh = useRefreshFeeds();
  const settings = useFeedSettings();
  const busy = data.refreshing || refresh.isPending;
  const s = data.settings;
  const exportOpml = async () => {
    const path = await pickSave({
      title: "Export feeds",
      defaultPath: "Libreri feeds.opml",
      filters: [{ name: "OPML", extensions: ["opml"] }],
    });
    if (!path) return;
    unwrap(commands.feedsExportOpml("feeds", path)).then(
      () => toast.success("Feeds exported"),
      fail("Could not export the feeds"),
    );
  };
  return (
    <div className="flex h-14 shrink-0 items-center gap-3 border-b px-5">
      <Rss className="size-5 text-muted-foreground" aria-hidden />
      <div className="flex min-w-0 flex-1 flex-col">
        <h1 className="text-[15px] leading-tight font-semibold">Feeds</h1>
        <p className="truncate text-[12px] text-muted-foreground">
          {data.feeds.length === 1 ? "1 feed" : `${data.feeds.length} feeds`}
          {data.unread ? ` · ${data.unread} new` : ""}
          {data.downloaded ? ` · ${data.downloaded} downloaded` : ""}
          {busy ? " · Checking for new items…" : ""}
        </p>
      </div>
      {data.feeds.length > 0 && (
        <Button
          variant="outline"
          size="sm"
          disabled={busy}
          onClick={() => refresh.mutate(null)}
          title="Check every feed for new items"
        >
          <RefreshCw className={cn(busy && "animate-spin")} /> Refresh
        </Button>
      )}
      <Button size="sm" onClick={() => onAdd("address")}>
        <Plus /> Follow feeds
      </Button>
      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <Button variant="ghost" size="icon" aria-label="More">
            <MoreHorizontal />
          </Button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content align="end" sideOffset={6} className={cn(menuContent, "w-72")}>
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() => onAsk({ kind: "newFolder", parent: null })}
            >
              <FolderPlus /> New folder…
            </DropdownMenu.Item>
            <DropdownMenu.Item className={menuItem} onSelect={() => onAdd("arxiv")}>
              <Rss /> Follow arXiv categories…
            </DropdownMenu.Item>
            <DropdownMenu.Item className={menuItem} onSelect={() => onAdd("opml")}>
              <FolderInput /> Import from an OPML file…
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={menuItem}
              disabled={!data.feeds.length}
              onSelect={() => void exportOpml()}
            >
              <FileDown /> Export to an OPML file…
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() =>
                void unwrap(commands.feedsReveal(null)).catch(fail("Could not show the folder"))
              }
            >
              <FolderOpen /> Show the Feeds folder
            </DropdownMenu.Item>
            <DropdownMenu.Separator className={menuSeparator} />
            <DropdownMenu.Label className={menuLabel}>CHECK FOR NEW ITEMS</DropdownMenu.Label>
            {[
              [0, "Only when I press Refresh"],
              [30, "Every 30 minutes"],
              [60, "Every hour"],
              [180, "Every 3 hours"],
              [720, "Every 12 hours"],
            ].map(([m, label]) => (
              <DropdownCheckItem
                key={m}
                checked={(s.refreshMinutes ?? 60) === m}
                onCheckedChange={() =>
                  settings.mutate({ refreshMinutes: m as number, keepDays: s.keepDays ?? 30 })
                }
              >
                {label}
              </DropdownCheckItem>
            ))}
            <DropdownMenu.Separator className={menuSeparator} />
            <DropdownMenu.Label className={menuLabel}>
              KEEP ITEMS NOT DOWNLOADED FOR
            </DropdownMenu.Label>
            {[7, 30, 90, 365].map((d) => (
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
  );
}

function Welcome({ onAdd }: { onAdd: (tab: AddTab) => void }) {
  const ways: { tab: AddTab; title: string; text: string }[] = [
    {
      tab: "arxiv",
      title: "arXiv categories",
      text: "New papers in the fields you choose, every weekday.",
    },
    {
      tab: "address",
      title: "A feed or website",
      text: "Journals, preprint servers, newsletters and blogs.",
    },
    { tab: "suggested", title: "Suggested sources", text: "bioRxiv, Nature, PLOS, eLife…" },
    { tab: "opml", title: "From another feed reader", text: "Import your subscriptions (OPML)." },
  ];
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-5 p-8 text-center">
      <Inbox className="size-10 text-muted-foreground" aria-hidden />
      <div className="flex max-w-md flex-col gap-1.5">
        <h2 className="text-[17px] font-semibold">Follow papers, articles and newsletters</h2>
        <p className="text-muted-foreground">
          New items come in while Libreri is open. Download the ones you want, keep them in your
          Feeds folder, and add the best to your library.
        </p>
      </div>
      <div className="grid w-full max-w-xl grid-cols-2 gap-2.5">
        {ways.map((w) => (
          <button
            key={w.tab}
            type="button"
            onClick={() => onAdd(w.tab)}
            className="flex flex-col items-start gap-0.5 rounded-lg border p-3 text-left hover:bg-muted"
          >
            <span className="font-medium">{w.title}</span>
            <span className="text-[12.5px] text-muted-foreground">{w.text}</span>
          </button>
        ))}
      </div>
    </div>
  );
}

function samePlace(a: FeedPlace, b: FeedPlace) {
  return a.kind === b.kind && (a.kind === "all" || a.id === (b as { id: string }).id);
}

function Tree({
  data,
  onAsk,
  onAdd,
}: {
  data: FeedsDto;
  onAsk: (a: Ask) => void;
  onAdd: (folder: string | null) => void;
}) {
  const tree = useMemo(() => feedTree(data.folders, data.feeds), [data.folders, data.feeds]);
  const { place, setPlace } = useFeedsView();
  return (
    <ResizablePanel id="feeds.tree" initial={256} side="left" label="folders and feeds">
      <nav
        aria-label="Feeds"
        className="flex min-w-0 flex-1 flex-col gap-px overflow-auto border-r bg-sidebar p-2"
      >
        <Row
          active={samePlace(place, { kind: "all" })}
          onClick={() => setPlace({ kind: "all" })}
          icon={<Inbox className="size-4" />}
          label="All feeds"
          count={data.unread}
          depth={0}
        />
        <div className="mt-2 flex h-6 items-center justify-between pr-1 pl-2.5">
          <h2 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
            FOLDERS & FEEDS
          </h2>
          <button
            type="button"
            aria-label="New folder"
            title="New folder"
            onClick={() => onAsk({ kind: "newFolder", parent: null })}
            className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            <FolderPlus className="size-3.5" />
          </button>
        </div>
        {tree.folders.map((t) => (
          <FolderRow key={t.folder.id} t={t} depth={0} onAsk={onAsk} onAdd={onAdd} data={data} />
        ))}
        {tree.feeds.map((f) => (
          <FeedRow key={f.id} feed={f} depth={0} onAsk={onAsk} />
        ))}
      </nav>
    </ResizablePanel>
  );
}

function Row({
  active,
  onClick,
  icon,
  label,
  count,
  depth,
  toggle,
  menu,
  error,
  title,
}: {
  active: boolean;
  onClick: () => void;
  icon: React.ReactNode;
  label: string;
  count: number;
  depth: number;
  toggle?: React.ReactNode;
  menu?: React.ReactNode;
  error?: string | null;
  title?: string;
}) {
  return (
    <div
      className={cn(
        "group flex h-7 items-center rounded-md pr-1",
        active ? "bg-muted font-medium" : "hover:bg-muted/60",
      )}
      style={{ paddingLeft: 4 + depth * 14 }}
    >
      <span className="flex w-4 shrink-0 justify-center">{toggle}</span>
      <button
        type="button"
        onClick={onClick}
        aria-current={active ? "page" : undefined}
        title={title ?? label}
        className="flex h-full min-w-0 flex-1 items-center gap-2 pl-1 text-left"
      >
        <span className="shrink-0 text-muted-foreground [&_svg]:size-4">{icon}</span>
        <span className="flex-1 truncate">{label}</span>
        {error && (
          <AlertCircle
            className="size-3.5 shrink-0 text-destructive"
            aria-label="Could not be read"
          />
        )}
        <span className="text-[11px] text-muted-foreground tabular-nums group-hover:hidden">
          {count || ""}
        </span>
      </button>
      <span className="hidden group-focus-within:flex group-hover:flex">{menu}</span>
    </div>
  );
}

function RowMenu({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <button
          type="button"
          aria-label={label}
          className="rounded p-0.5 text-muted-foreground hover:bg-background hover:text-foreground"
        >
          <MoreHorizontal className="size-3.5" />
        </button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="start" sideOffset={4} className={cn(menuContent, "w-72")}>
          {children}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function FolderRow({
  t,
  depth,
  onAsk,
  onAdd,
  data,
}: {
  t: TreeFolder;
  depth: number;
  onAsk: (a: Ask) => void;
  onAdd: (folder: string | null) => void;
  data: FeedsDto;
}) {
  const { place, setPlace, closed, toggle } = useFeedsView();
  const change = useChangeFeedFolder();
  const remove = useRemoveFeedFolder();
  const allRead = useMarkAllRead();
  const refresh = useRefreshFeeds();
  const f = t.folder;
  const open = !closed[f.id];
  const feedIds = useMemo(() => {
    const out: string[] = [];
    const walk = (x: TreeFolder) => {
      out.push(...x.feeds.map((y) => y.id));
      x.folders.forEach(walk);
    };
    walk(t);
    return out;
  }, [t]);
  return (
    <>
      <Row
        active={samePlace(place, { kind: "folder", id: f.id })}
        onClick={() => setPlace({ kind: "folder", id: f.id })}
        icon={f.autoDownload ? <FolderDown /> : <FolderIcon />}
        title={f.autoDownload ? `${f.name} · new items download automatically` : f.name}
        label={f.name}
        count={t.unread}
        depth={depth}
        toggle={
          t.folders.length || t.feeds.length ? (
            <button
              type="button"
              aria-label={open ? `Close ${f.name}` : `Open ${f.name}`}
              aria-expanded={open}
              onClick={() => toggle(f.id)}
              className="text-muted-foreground"
            >
              {open ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
            </button>
          ) : null
        }
        menu={
          <RowMenu label={`${f.name} options`}>
            <DropdownCheckItem
              checked={!!f.autoDownload}
              onCheckedChange={(v) =>
                change.mutate({ id: f.id, change: { name: null, parent: null, autoDownload: v } })
              }
            >
              Download new items automatically
            </DropdownCheckItem>
            <DropdownMenu.Separator className={menuSeparator} />
            <DropdownMenu.Item className={menuItem} onSelect={() => onAdd(f.id)}>
              <Plus /> Follow a feed here…
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() => onAsk({ kind: "newFolder", parent: f.id })}
            >
              <FolderPlus /> New folder inside…
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={menuItem}
              disabled={!feedIds.length}
              onSelect={() => refresh.mutate(feedIds)}
            >
              <RefreshCw /> Check for new items
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() => allRead.mutate({ folder: f.id, feed: null })}
            >
              <CheckCheck /> Mark all as seen
            </DropdownMenu.Item>
            <DropdownMenu.Separator className={menuSeparator} />
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() => onAsk({ kind: "renameFolder", id: f.id, name: f.name })}
            >
              <Pencil /> Rename…
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={menuItem}
              disabled={data.folders.length < 2 && !f.parent}
              onSelect={() => onAsk({ kind: "moveFolder", id: f.id, parent: f.parent })}
            >
              <FolderInput /> Move to…
            </DropdownMenu.Item>
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() =>
                remove.mutate(f.id, {
                  onSuccess: () =>
                    toast(`Removed “${f.name}”`, {
                      description: "What was in it moved up a level.",
                    }),
                })
              }
            >
              <Trash2 /> Remove folder
            </DropdownMenu.Item>
          </RowMenu>
        }
      />
      {open &&
        t.folders.map((x) => (
          <FolderRow
            key={x.folder.id}
            t={x}
            depth={depth + 1}
            onAsk={onAsk}
            onAdd={onAdd}
            data={data}
          />
        ))}
      {open && t.feeds.map((x) => <FeedRow key={x.id} feed={x} depth={depth + 1} onAsk={onAsk} />)}
    </>
  );
}

/** A folder whose new items download by themselves. */
function FolderDown() {
  return <FolderInput />;
}

function FeedRow({
  feed: f,
  depth,
  onAsk,
}: {
  feed: FeedDto;
  depth: number;
  onAsk: (a: Ask) => void;
}) {
  const { place, setPlace } = useFeedsView();
  const change = useChangeFeed();
  const remove = useRemoveFeed();
  const allRead = useMarkAllRead();
  const refresh = useRefreshFeeds();
  const auto = f.autoDownload || f.autoFromFolder;
  return (
    <Row
      active={samePlace(place, { kind: "feed", id: f.id })}
      onClick={() => setPlace({ kind: "feed", id: f.id })}
      icon={auto ? <Download /> : <Rss />}
      label={f.title}
      title={[
        f.title,
        f.url,
        auto ? "New items download automatically" : "",
        f.error ? `Could not be read: ${f.error}` : "",
      ]
        .filter(Boolean)
        .join("\n")}
      count={f.unread}
      depth={depth}
      error={f.error}
      menu={
        <RowMenu label={`${f.title} options`}>
          <DropdownCheckItem
            checked={auto}
            onCheckedChange={(v) =>
              change.mutate({
                id: f.id,
                change: { title: null, folder: null, autoDownload: v, speed: null },
              })
            }
            className={cn(f.autoFromFolder && "opacity-60")}
          >
            Download new items automatically
          </DropdownCheckItem>
          {f.autoFromFolder && (
            <p className="px-8 pb-1 text-[11.5px] text-muted-foreground">
              Set by a folder it is in.
            </p>
          )}
          <DropdownMenu.Separator className={menuSeparator} />
          <DropdownMenu.Item className={menuItem} onSelect={() => refresh.mutate([f.id])}>
            <RefreshCw /> Check for new items
          </DropdownMenu.Item>
          <DropdownMenu.Item
            className={menuItem}
            onSelect={() => allRead.mutate({ folder: null, feed: f.id })}
          >
            <CheckCheck /> Mark all as seen
          </DropdownMenu.Item>
          {f.site && (
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() => void commands.openExternalUrl(f.site!)}
            >
              <ExternalLink /> Open the website
            </DropdownMenu.Item>
          )}
          <DropdownMenu.Item
            className={menuItem}
            onSelect={() =>
              void navigator.clipboard.writeText(f.url).then(() => toast("Feed address copied"))
            }
          >
            <Rss /> Copy the feed's address
          </DropdownMenu.Item>
          <DropdownMenu.Separator className={menuSeparator} />
          <DropdownMenu.Item
            className={menuItem}
            onSelect={() => onAsk({ kind: "renameFeed", id: f.id, name: f.title })}
          >
            <Pencil /> Rename…
          </DropdownMenu.Item>
          <DropdownMenu.Item
            className={menuItem}
            onSelect={() => onAsk({ kind: "moveFeed", feed: f })}
          >
            <FolderInput /> Move to folder…
          </DropdownMenu.Item>
          <DropdownMenu.Item
            className={menuItem}
            onSelect={() => {
              if (place.kind === "feed" && place.id === f.id) setPlace({ kind: "all" });
              remove.mutate(f.id, {
                onSuccess: () =>
                  toast(`Stopped following “${f.title}”`, {
                    description: "Its downloads stay in your Feeds folder.",
                  }),
              });
            }}
          >
            <Unplug /> Stop following
          </DropdownMenu.Item>
        </RowMenu>
      }
    />
  );
}

function Items({ data, onFeeds }: { data: FeedsDto; onFeeds?: () => void }) {
  const { place, show, setShow, topic, setTopic, search, setSearch, kept } = useFeedsView();
  const deferred = useDeferredValue(search);
  const filter = {
    folder: place.kind === "folder" ? place.id : null,
    feed: place.kind === "feed" ? place.id : null,
    show,
    topic,
    search: deferred.trim() || null,
    keep: kept,
  };
  const { data: list, isPending } = useFeedItems(filter);
  const allRead = useMarkAllRead();
  const [adding, setAdding] = useState<FeedItem | null>(null);
  const { editLibrary } = usePermissions();
  const downloading = useMemo(() => new Set(data.downloading), [data.downloading]);
  const feed = place.kind === "feed" ? data.feeds.find((f) => f.id === place.id) : undefined;
  const folder = place.kind === "folder" ? data.folders.find((f) => f.id === place.id) : undefined;
  const heading = feed?.title ?? folder?.name ?? "All feeds";
  const items = list?.items ?? [];

  return (
    <section aria-label={heading} className="flex min-w-0 flex-1 flex-col">
      {onFeeds && <SideOverButton label="Folders and feeds" onOpen={onFeeds} />}
      <div className="flex flex-col gap-2 border-b px-5 py-3">
        <div className="flex items-center gap-3">
          <h2 className="min-w-0 flex-1 truncate text-[15px] font-semibold">{heading}</h2>
          <div className="relative">
            <Search className="pointer-events-none absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
            <Input
              aria-label="Find in these items"
              placeholder="Find title, author, abstract…"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="h-8 w-64 pl-7"
            />
          </div>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => allRead.mutate({ folder: filter.folder, feed: filter.feed })}
            title="Mark everything here as seen"
          >
            <CheckCheck /> Mark all seen
          </Button>
        </div>
        {feed?.error && (
          <p className="flex items-center gap-1.5 text-[12.5px] text-destructive">
            <AlertCircle className="size-3.5" /> This feed could not be read last time: {feed.error}
            .
          </p>
        )}
        <div className="flex flex-wrap items-center gap-1.5">
          <div role="tablist" aria-label="Show" className="flex gap-0.5 rounded-md bg-muted p-0.5">
            {SHOWS.map((s) => (
              <button
                key={s.id}
                type="button"
                role="tab"
                aria-selected={show === s.id}
                onClick={() => setShow(s.id)}
                className={cn(
                  "rounded px-2 py-0.5 text-[12px] text-muted-foreground",
                  show === s.id && "bg-background font-medium text-foreground shadow-sm",
                )}
              >
                {s.label}
              </button>
            ))}
          </div>
          {(list?.topics.length ?? 0) > 1 && (
            <NativeSelect
              aria-label="Topic"
              value={topic ?? ""}
              onChange={(e) => setTopic(e.target.value || null)}
              className="h-7 w-auto max-w-72 text-[12px]"
            >
              <option value="">All topics</option>
              {list!.topics.map((t) => (
                <option key={t.code} value={t.code}>
                  {t.name ? `${t.name} (${t.code})` : t.code} · {t.count}
                </option>
              ))}
            </NativeSelect>
          )}
          <span className="ml-auto text-[12px] text-muted-foreground">
            {list
              ? list.total > items.length
                ? `${items.length} of ${list.total}`
                : list.total === 1
                  ? "1 item"
                  : `${list.total} items`
              : ""}
          </span>
        </div>
      </div>
      <div className="min-h-0 flex-1 overflow-auto">
        {isPending && !list && <RowsSkeleton rows={7} label="Loading items…" />}
        {list && items.length === 0 && (
          <div className="flex flex-col items-center gap-2 p-10 text-center text-muted-foreground">
            <Inbox className="size-7" aria-hidden />
            <p>
              {search.trim()
                ? "Nothing matches."
                : show === "unread"
                  ? "Nothing new. Refresh to check again."
                  : show === "downloaded"
                    ? "Nothing downloaded here yet."
                    : show === "library"
                      ? "Nothing from here was added to the library yet."
                      : data.refreshing
                        ? "Checking for new items…"
                        : "No items yet. New ones come in when the feed has them."}
            </p>
          </div>
        )}
        <ul className="flex flex-col">
          {items.map((it) => (
            <ItemRow
              key={it.id}
              item={it}
              downloading={downloading.has(it.id)}
              canAdd={editLibrary}
              onPreview={(it) => openFeedDoc(it, "feeds")}
              onAdd={setAdding}
              showSource={place.kind !== "feed"}
            />
          ))}
        </ul>
      </div>
      <AddToLibraryDialog item={adding} onClose={() => setAdding(null)} />
    </section>
  );
}

function ItemRow({
  item: it,
  downloading,
  canAdd,
  onPreview,
  onAdd,
  showSource,
}: {
  item: FeedItem;
  downloading: boolean;
  canAdd: boolean;
  onPreview: (it: FeedItem) => void;
  onAdd: (it: FeedItem) => void;
  showSource: boolean;
}) {
  const [open, setOpen] = useState(false);
  const download = useDownloadItem();
  const remove = useDeleteItems();
  const forget = useForgetFile();
  const markRead = useMarkRead();
  const keep = useFeedsView((s) => s.keep);
  const busy = downloading || download.isPending;
  const unread = !it.read;
  const note = announceLabel(it.announce);
  const pdf = it.file?.toLowerCase().endsWith(".pdf") ?? !!it.pdf;
  const expand = () => {
    setOpen((o) => !o);
    if (unread) {
      // Stays in New while it is being read.
      keep(it.id);
      markRead.mutate({ ids: [it.id], read: true });
    }
  };
  return (
    <li className="group flex gap-3 border-b px-5 py-3 hover:bg-muted/40">
      <span className="mt-1.5 flex w-2 shrink-0 justify-center">
        {unread && <span className="size-2 rounded-full bg-primary" aria-label="New" />}
      </span>
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <button
          type="button"
          onClick={expand}
          aria-expanded={open}
          className={cn("text-left leading-snug", unread ? "font-semibold" : "font-medium")}
        >
          {it.title}
        </button>
        <p className="flex flex-wrap items-center gap-x-1.5 text-[12px] text-muted-foreground">
          {showSource && <span className="font-medium text-foreground/80">{it.source}</span>}
          {showSource && it.authors.length > 0 && <span aria-hidden>·</span>}
          {it.authors.length > 0 && <span>{authorsLine(it.authors)}</span>}
          {(it.published || it.foundAt) && <span aria-hidden>·</span>}
          <span>{whenLine(it.published ?? it.foundAt)}</span>
          {note && (
            <span className="rounded bg-muted px-1.5 py-px text-[11px] font-medium">{note}</span>
          )}
          {it.topics.slice(0, 4).map((t) => (
            <span key={t} className="rounded border px-1.5 py-px font-mono text-[10.5px]">
              {t}
            </span>
          ))}
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
        {open && (
          <div className="flex flex-wrap gap-2 pt-1">
            {it.link && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => void commands.openExternalUrl(it.link!)}
              >
                <ExternalLink /> Open the page
              </Button>
            )}
            {it.doi && (
              <Button
                variant="outline"
                size="sm"
                onClick={() => void commands.openExternalUrl(`https://doi.org/${it.doi}`)}
              >
                <ExternalLink /> DOI {it.doi}
              </Button>
            )}
            {it.file && (
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  void unwrap(commands.feedsReveal(it.file)).catch(fail("Could not show it"))
                }
              >
                <FolderOpen /> Show in folder
              </Button>
            )}
            {it.file && (
              <Button variant="ghost" size="sm" onClick={() => forget.mutate(it.id)}>
                <Trash2 /> Delete the download only
              </Button>
            )}
            <Button
              variant="ghost"
              size="sm"
              onClick={() => markRead.mutate({ ids: [it.id], read: !it.read })}
            >
              {it.read ? "Mark as new" : "Mark as seen"}
            </Button>
          </div>
        )}
      </div>
      <div className="flex shrink-0 items-start gap-1">
        {it.book ? (
          <Button variant="outline" size="sm" onClick={() => void openBook(it.book!)}>
            <Library /> In library · Open
          </Button>
        ) : it.file ? (
          <>
            <Button variant="outline" size="sm" onClick={() => onPreview(it)}>
              {pdf ? <BookOpen /> : <FileText />} Read
            </Button>
            {canAdd && (
              <Button size="sm" onClick={() => onAdd(it)}>
                <BookPlus /> Add to library
              </Button>
            )}
          </>
        ) : (
          <Button
            variant="outline"
            size="sm"
            disabled={busy}
            onClick={() =>
              download.mutate(it.id, {
                onSuccess: () => toast.success("Downloaded", { description: it.title }),
              })
            }
            title={it.pdf ? "Download the PDF" : "Keep a readable copy of the article (Markdown)"}
          >
            {busy ? <Loader2 className="animate-spin" /> : <Download />}
            {busy ? "Downloading…" : it.pdf ? "Download PDF" : "Download"}
          </Button>
        )}
        <ShareMenu it={it} />
        <Button
          variant="ghost"
          size="icon"
          aria-label={`Delete “${it.title}”`}
          title={it.file ? "Delete (and its download)" : "Delete"}
          disabled={busy}
          onClick={() => remove.mutate([it.id])}
        >
          <Trash2 />
        </Button>
      </div>
    </li>
  );
}

function AskDialog({ ask, data, onClose }: { ask: Ask; data: FeedsDto; onClose: () => void }) {
  const addFolder = useAddFeedFolder();
  const changeFolder = useChangeFeedFolder();
  const changeFeed = useChangeFeed();
  const [name, setName] = useState(
    ask.kind === "renameFolder" || ask.kind === "renameFeed" ? ask.name : "",
  );
  const [target, setTarget] = useState(
    ask.kind === "moveFeed"
      ? (ask.feed.folder ?? "")
      : ask.kind === "moveFolder"
        ? (ask.parent ?? "")
        : "",
  );
  const exclude = useMemo(() => {
    if (ask.kind !== "moveFolder") return undefined;
    // A folder cannot go inside itself.
    const out = new Set([ask.id]);
    let grew = true;
    while (grew) {
      grew = false;
      for (const f of data.folders)
        if (f.parent && out.has(f.parent) && !out.has(f.id)) {
          out.add(f.id);
          grew = true;
        }
    }
    return out;
  }, [ask, data.folders]);
  const title =
    ask.kind === "newFolder"
      ? "New folder"
      : ask.kind === "moveFeed"
        ? `Move “${ask.feed.title}”`
        : ask.kind === "moveFolder"
          ? "Move folder"
          : "Rename";
  const moving = ask.kind === "moveFeed" || ask.kind === "moveFolder";
  const submit = () => {
    const done = { onSuccess: onClose };
    switch (ask.kind) {
      case "newFolder":
        return addFolder.mutate({ name, parent: ask.parent }, done);
      case "renameFolder":
        return changeFolder.mutate(
          { id: ask.id, change: { name, parent: null, autoDownload: null } },
          done,
        );
      case "renameFeed":
        return changeFeed.mutate(
          { id: ask.id, change: { title: name, folder: null, autoDownload: null, speed: null } },
          done,
        );
      case "moveFeed":
        return changeFeed.mutate(
          {
            id: ask.feed.id,
            change: { title: null, folder: target, autoDownload: null, speed: null },
          },
          done,
        );
      case "moveFolder":
        return changeFolder.mutate(
          { id: ask.id, change: { name: null, parent: target, autoDownload: null } },
          done,
        );
    }
  };
  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={title}
      description={
        ask.kind === "newFolder"
          ? "Folders sort feeds into categories and subcategories. Downloads are kept in folders of the same names."
          : undefined
      }
      className="w-[420px]"
    >
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          if (moving || name.trim()) submit();
        }}
      >
        {moving ? (
          <FolderSelect
            folders={data.folders}
            value={target}
            onChange={setTarget}
            topLabel="Top level"
            exclude={exclude}
          />
        ) : (
          <Input
            autoFocus
            aria-label="Name"
            value={name}
            onChange={(e) => setName(e.target.value)}
          />
        )}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button type="submit" disabled={!moving && !name.trim()}>
            <Check /> {ask.kind === "newFolder" ? "Add" : moving ? "Move" : "Rename"}
          </Button>
        </div>
      </form>
    </Dialog>
  );
}

/** Share: copy the paper's address (or as text, Markdown, BibTeX), or
 * email it. */
function ShareMenu({ it }: { it: FeedItem }) {
  const url = shareUrl(it);
  const copy = (text: string, what: string) =>
    void navigator.clipboard.writeText(text).then(
      () => toast(`${what} copied`, { description: it.title }),
      () => toast.error("Could not copy"),
    );
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button variant="ghost" size="icon" aria-label={`Share “${it.title}”`} title="Share">
          <Share2 />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className={menuContent} align="end" sideOffset={4}>
          {url && (
            <div className="max-w-72 truncate px-2 pt-1.5 pb-1 text-[11.5px] text-muted-foreground">
              {url}
            </div>
          )}
          <DropdownMenu.Item
            className={menuItem}
            disabled={!url}
            onSelect={() => url && copy(url, "Link")}
          >
            <Link2 /> Copy link
          </DropdownMenu.Item>
          <DropdownMenu.Item
            className={menuItem}
            onSelect={() => copy(shareText(it), "Title and link")}
          >
            <Copy /> Copy title, authors and link
          </DropdownMenu.Item>
          <DropdownMenu.Item
            className={menuItem}
            onSelect={() => copy(shareMarkdown(it), "Markdown link")}
          >
            <FileText /> Copy as Markdown link
          </DropdownMenu.Item>
          {(it.arxivId || it.doi) && (
            <DropdownMenu.Item
              className={menuItem}
              onSelect={() => copy(shareBibtex(it), "BibTeX")}
            >
              <Quote /> Copy BibTeX citation
            </DropdownMenu.Item>
          )}
          <DropdownMenu.Separator className={menuSeparator} />
          <DropdownMenu.Item
            className={menuItem}
            onSelect={() => void commands.openExternalUrl(shareMailto(it))}
          >
            <Mail /> Email…
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
