import { useMemo, useState } from "react";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { Check, FileUp, Loader2, Rss, Search } from "lucide-react";
import { toast } from "sonner";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { commands, unwrap, type FeedFolder, type FeedPreviewDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import {
  feedsKey,
  refreshFeeds,
  useAddArxiv,
  useAddFeed,
  useArxivCategories,
  useFeedsOverview,
  useSuggestedFeeds,
} from "../api";
import { flatFeedFolders } from "../model";

export type AddTab = "address" | "arxiv" | "suggested" | "opml";

const TABS: { id: AddTab; label: string }[] = [
  { id: "address", label: "Address" },
  { id: "arxiv", label: "arXiv" },
  { id: "suggested", label: "Suggested" },
  { id: "opml", label: "OPML file" },
];

/** Follow feeds: by address, arXiv categories, suggested sources or an OPML file. */
export function AddFeedDialog({
  open,
  tab: initialTab,
  folder: initialFolder,
  onClose,
}: {
  open: boolean;
  tab: AddTab;
  /** The folder new feeds go in. */
  folder: string | null;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<AddTab>(initialTab);
  const { data } = useFeedsOverview();
  const folders = data?.folders ?? [];
  const [folder, setFolder] = useState<string>(initialFolder ?? "");
  const [auto, setAuto] = useState(false);
  const followed = useMemo(() => new Set(data?.feeds.map((f) => f.url)), [data?.feeds]);

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Follow feeds"
      description="New papers, articles and newsletters come in while Libreri is open. Download the ones you want, and add them to your library."
      className="w-[640px]"
    >
      <div role="tablist" aria-label="Where from" className="flex gap-1 rounded-lg bg-muted p-1">
        {TABS.map((t) => (
          <button
            key={t.id}
            type="button"
            role="tab"
            aria-selected={tab === t.id}
            onClick={() => setTab(t.id)}
            className={cn(
              "flex-1 rounded-md py-1 text-[12.5px] font-medium text-muted-foreground",
              tab === t.id && "bg-background text-foreground shadow-sm",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      {tab === "address" && (
        <AddressTab folder={folder || null} auto={auto} followed={followed} onDone={onClose} />
      )}
      {tab === "arxiv" && <ArxivTab followed={followed} auto={auto} onDone={onClose} />}
      {tab === "suggested" && (
        <SuggestedTab folder={folder || null} auto={auto} followed={followed} />
      )}
      {tab === "opml" && <OpmlTab folder={folder || null} onDone={onClose} />}

      <div className="flex flex-wrap items-center gap-x-4 gap-y-2 border-t pt-3 text-[12.5px]">
        {tab !== "arxiv" && (
          <label className="flex items-center gap-2">
            <span className="text-muted-foreground">Into</span>
            <FolderSelect folders={folders} value={folder} onChange={setFolder} />
          </label>
        )}
        {tab !== "opml" && (
          <label className="flex items-center gap-2">
            <input type="checkbox" checked={auto} onChange={(e) => setAuto(e.target.checked)} />
            Download new items automatically
          </label>
        )}
      </div>
    </Dialog>
  );
}

export function FolderSelect({
  folders,
  value,
  onChange,
  topLabel = "No folder",
  exclude,
}: {
  folders: FeedFolder[];
  value: string;
  onChange: (v: string) => void;
  topLabel?: string;
  /** Folders that cannot be chosen (a folder and what is inside it). */
  exclude?: Set<string>;
}) {
  return (
    <NativeSelect value={value} onChange={(e) => onChange(e.target.value)} className="w-56">
      <option value="">{topLabel}</option>
      {flatFeedFolders(folders)
        .filter(({ folder }) => !exclude?.has(folder.id))
        .map(({ folder, depth }) => (
          <option key={folder.id} value={folder.id}>
            {"  ".repeat(depth)}
            {folder.name}
          </option>
        ))}
    </NativeSelect>
  );
}

function AddressTab({
  folder,
  auto,
  followed,
  onDone,
}: {
  folder: string | null;
  auto: boolean;
  followed: Set<string>;
  onDone: () => void;
}) {
  const [address, setAddress] = useState("");
  const [finding, setFinding] = useState(false);
  const [found, setFound] = useState<FeedPreviewDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [title, setTitle] = useState("");
  const add = useAddFeed();

  const find = async () => {
    setFinding(true);
    setError(null);
    setFound(null);
    try {
      const f = await unwrap(commands.feedFind(address));
      setFound(f);
      setTitle(f.title);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setFinding(false);
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <form
        className="flex gap-2"
        onSubmit={(e) => {
          e.preventDefault();
          if (address.trim()) void find();
        }}
      >
        <Input
          autoFocus
          aria-label="Feed or site address"
          placeholder="Paste a feed or website address (arxiv.org/…, a blog, a Substack…)"
          value={address}
          onChange={(e) => setAddress(e.target.value)}
          className="flex-1"
        />
        <Button type="submit" variant="outline" disabled={!address.trim() || finding}>
          {finding ? <Loader2 className="animate-spin" /> : <Search />} Find
        </Button>
      </form>
      {error && <p className="text-[12.5px] text-destructive">{error}</p>}
      {!found && !error && (
        <p className="text-[12.5px] text-muted-foreground">
          A site's own address is enough when its pages name their feed. PubMed, journals and
          newsletters offer feed addresses too (look for RSS).
        </p>
      )}
      {found && (
        <div className="flex flex-col gap-2 rounded-lg border p-3">
          <div className="flex items-center gap-2">
            <Rss className="size-4 text-muted-foreground" aria-hidden />
            <Input
              aria-label="Title"
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              className="h-8 flex-1 font-medium"
            />
          </div>
          <p className="truncate text-[12px] text-muted-foreground" title={found.url}>
            {found.url} · {found.count === 1 ? "1 item now" : `${found.count} items now`}
          </p>
          {found.sample.length > 0 && (
            <ul className="list-disc pl-5 text-[12.5px] text-muted-foreground">
              {found.sample.map((s, i) => (
                <li key={i} className="truncate">
                  {s}
                </li>
              ))}
            </ul>
          )}
          <div className="flex justify-end">
            {found.followed || followed.has(found.url) ? (
              <span className="text-[12.5px] text-muted-foreground">
                You already follow this feed.
              </span>
            ) : (
              <Button
                size="sm"
                disabled={add.isPending}
                onClick={() =>
                  add.mutate(
                    { url: found.url, title, folder, autoDownload: auto },
                    {
                      onSuccess: () => {
                        toast.success(`Following “${title || found.title}”`);
                        onDone();
                      },
                    },
                  )
                }
              >
                {add.isPending ? <Loader2 className="animate-spin" /> : <Check />} Follow
              </Button>
            )}
          </div>
        </div>
      )}
    </div>
  );
}

function ArxivTab({
  followed,
  auto,
  onDone,
}: {
  followed: Set<string>;
  auto: boolean;
  onDone: () => void;
}) {
  const { data: groups = [], isPending } = useArxivCategories(true);
  const [filter, setFilter] = useState("");
  const [picked, setPicked] = useState<Set<string>>(new Set());
  const [search, setSearch] = useState("");
  const add = useAddArxiv();
  const isFollowed = (code: string) => followed.has(`https://rss.arxiv.org/rss/${code}`);
  const words = filter.trim().toLowerCase();
  const shown = groups
    .map((g) => ({
      ...g,
      categories: g.categories.filter(
        (c) =>
          !words ||
          c.code.toLowerCase().includes(words) ||
          c.name.toLowerCase().includes(words) ||
          g.name.toLowerCase().includes(words),
      ),
    }))
    .filter((g) => g.categories.length > 0);
  const flip = (code: string) =>
    setPicked((p) => {
      const n = new Set(p);
      if (n.has(code)) n.delete(code);
      else n.add(code);
      return n;
    });
  const count = picked.size + (search.trim() ? 1 : 0);

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <Input
        autoFocus
        aria-label="Find a category"
        placeholder="Find a category (machine learning, math.PR, quantum…)"
        value={filter}
        onChange={(e) => setFilter(e.target.value)}
      />
      <div className="max-h-72 overflow-auto rounded-lg border">
        {isPending && (
          <p className="flex items-center gap-2 p-3 text-muted-foreground">
            <Loader2 className="size-4 animate-spin" /> Loading…
          </p>
        )}
        {shown.map((g) => (
          <fieldset key={g.name} className="border-b px-3 py-2 last:border-b-0">
            <legend className="sr-only">{g.name}</legend>
            <div className="pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
              {g.name.toUpperCase()}
            </div>
            <div className="grid grid-cols-2 gap-x-3">
              {g.categories.map((c) => {
                const already = isFollowed(c.code);
                return (
                  <label
                    key={c.code}
                    className={cn(
                      "flex min-w-0 items-center gap-2 rounded px-1 py-0.5 text-[12.5px] hover:bg-muted",
                      already && "opacity-60",
                    )}
                    title={already ? "Already followed" : `${c.name} (${c.code})`}
                  >
                    <input
                      type="checkbox"
                      checked={already || picked.has(c.code)}
                      disabled={already}
                      onChange={() => flip(c.code)}
                    />
                    <span className="truncate">{c.name}</span>
                    <span className="ml-auto shrink-0 font-mono text-[11px] text-muted-foreground">
                      {c.code}
                    </span>
                  </label>
                );
              })}
            </div>
          </fieldset>
        ))}
        {!isPending && !shown.length && (
          <p className="p-3 text-muted-foreground">No category matches.</p>
        )}
      </div>
      <label className="flex flex-col gap-1">
        <span className="text-[11.5px] font-medium text-muted-foreground">
          And/or follow a search (newest papers that match)
        </span>
        <Input
          placeholder="diffusion models · au:Tao AND cat:math.NT"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
        />
      </label>
      <p className="text-[12px] text-muted-foreground">
        Each category goes in <b>arXiv › its group</b>, and searches in <b>arXiv › Searches</b>.
        arXiv announces new papers once a day, on weekdays.
      </p>
      <div className="flex justify-end">
        <Button
          disabled={!count || add.isPending}
          onClick={() =>
            add.mutate(
              { codes: [...picked], search: search.trim() || null, autoDownload: auto },
              {
                onSuccess: (ids) => {
                  toast.success(
                    ids.length === 1
                      ? "Following 1 arXiv feed"
                      : `Following ${ids.length} arXiv feeds`,
                  );
                  onDone();
                },
              },
            )
          }
        >
          {add.isPending ? <Loader2 className="animate-spin" /> : <Check />}
          {count === 1 ? "Follow 1 feed" : `Follow ${count || ""} feeds`}
        </Button>
      </div>
    </div>
  );
}

function SuggestedTab({
  folder,
  auto,
  followed,
}: {
  folder: string | null;
  auto: boolean;
  followed: Set<string>;
}) {
  const { data: list = [] } = useSuggestedFeeds(true);
  const add = useAddFeed();
  const [adding, setAdding] = useState<string | null>(null);
  const groups = [...new Set(list.map((s) => s.group))];
  return (
    <div className="flex max-h-96 flex-col gap-3 overflow-auto">
      <p className="text-[12.5px] text-muted-foreground">
        For arXiv, choose categories in the arXiv tab. Any other source with a feed can be added by
        its address.
      </p>
      {groups.map((g) => (
        <div key={g} className="flex flex-col gap-1">
          <div className="text-[11px] font-semibold tracking-wide text-muted-foreground">
            {g.toUpperCase()}
          </div>
          {list
            .filter((s) => s.group === g)
            .map((s) => {
              const already = followed.has(s.url);
              return (
                <div key={s.url} className="flex items-center gap-3 rounded-md border px-3 py-2">
                  <span className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate font-medium">{s.title}</span>
                    <span className="truncate text-[12px] text-muted-foreground">{s.about}</span>
                  </span>
                  {already ? (
                    <span className="flex items-center gap-1 text-[12px] text-muted-foreground">
                      <Check className="size-3.5" /> Following
                    </span>
                  ) : (
                    <Button
                      size="sm"
                      variant="outline"
                      disabled={adding === s.url}
                      onClick={() => {
                        setAdding(s.url);
                        add.mutate(
                          { url: s.url, title: s.title, folder, autoDownload: auto },
                          {
                            onSuccess: () => toast.success(`Following “${s.title}”`),
                            onSettled: () => setAdding(null),
                          },
                        );
                      }}
                    >
                      {adding === s.url ? <Loader2 className="animate-spin" /> : <Rss />} Follow
                    </Button>
                  )}
                </div>
              );
            })}
        </div>
      ))}
    </div>
  );
}

function OpmlTab({ folder, onDone }: { folder: string | null; onDone: () => void }) {
  const qc = useQueryClient();
  const [busy, setBusy] = useState(false);
  const choose = async () => {
    const path = await pickFile({
      multiple: false,
      title: "Import feeds from an OPML file",
      filters: [{ name: "OPML", extensions: ["opml", "xml"] }],
    });
    if (typeof path !== "string") return;
    setBusy(true);
    try {
      const [added, skipped] = await unwrap(commands.feedsImportOpml(path, folder));
      void qc.invalidateQueries({ queryKey: feedsKey });
      toast.success(added === 1 ? "Following 1 feed" : `Following ${added} feeds`, {
        description: skipped ? `${skipped} already followed.` : undefined,
      });
      if (added) void refreshFeeds(null, true);
      onDone();
    } catch (e) {
      toast.error("Could not import the file", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col items-start gap-3">
      <p className="text-[12.5px] text-muted-foreground">
        Feed readers (Feedly, Inoreader, NetNewsWire, Thunderbird, Zotero…) export your
        subscriptions as an OPML file. Their folders become folders here.
      </p>
      <Button onClick={() => void choose()} disabled={busy}>
        {busy ? <Loader2 className="animate-spin" /> : <FileUp />} Choose an OPML file…
      </Button>
    </div>
  );
}
