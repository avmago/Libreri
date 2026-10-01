import { useEffect, useMemo, useState } from "react";
import { RowsSkeleton } from "@/components/Placeholders";
import { Check, Loader2, Plus, Search, Settings2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import { AddressTab, OpmlTab, useAddFeed } from "@/features/feeds";
import type { Show } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { useCategories, useIndexStatus, usePodcasts, useSearchShows, useTrending } from "./api";
import { openPodcastSettings } from "./opener";
import { Artwork } from "./PodcastPlayer";

export type FindTab = "search" | "popular" | "address" | "opml";

const TABS: { id: FindTab; label: string }[] = [
  { id: "search", label: "Search" },
  { id: "popular", label: "Popular" },
  { id: "address", label: "Address" },
  { id: "opml", label: "OPML file" },
];

/** Waits until typing stops. */
function useSettled(value: string, ms = 400): string {
  const [v, setV] = useState(value);
  useEffect(() => {
    const id = setTimeout(() => setV(value), ms);
    return () => clearTimeout(id);
  }, [value, ms]);
  return v;
}

/** Find podcasts: search Apple Podcasts (or Podcast Index), popular shows, an address, OPML. */
export function FindPodcastsDialog({
  open,
  tab: initialTab = "search",
  onClose,
}: {
  open: boolean;
  tab?: FindTab;
  onClose: () => void;
}) {
  const [tab, setTab] = useState<FindTab>(initialTab);
  const { data } = usePodcasts();
  const followed = useMemo(() => new Set(data?.feeds.map((f) => f.url)), [data?.feeds]);
  const [auto, setAuto] = useState(false);
  const onSettings = () => {
    onClose();
    openPodcastSettings();
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Find podcasts"
      description="Follow shows to see their new episodes here. Play them streaming, or download them to listen offline."
      className="w-[680px]"
    >
      <div role="tablist" aria-label="How" className="flex gap-1 rounded-lg bg-muted p-1">
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

      {tab === "search" && <SearchTab followed={followed} auto={auto} onSettings={onSettings} />}
      {tab === "popular" && <PopularTab followed={followed} auto={auto} onSettings={onSettings} />}
      {tab === "address" && (
        <AddressTab
          space="podcasts"
          folder={null}
          auto={auto}
          followed={followed}
          onDone={onClose}
        />
      )}
      {tab === "opml" && <OpmlTab space="podcasts" folder={null} onDone={onClose} />}

      {tab !== "opml" && (
        <label className="flex items-center gap-2 border-t pt-3 text-[12.5px]">
          <input type="checkbox" checked={auto} onChange={(e) => setAuto(e.target.checked)} />
          Download new episodes automatically
        </label>
      )}
    </Dialog>
  );
}

function SearchTab({
  followed,
  auto,
  onSettings,
}: {
  followed: Set<string>;
  auto: boolean;
  onSettings: () => void;
}) {
  const { data: index } = useIndexStatus();
  const [query, setQuery] = useState("");
  const [useIndex, setUseIndex] = useState(false);
  const settled = useSettled(query);
  const search = useSearchShows(settled, useIndex && !!index?.configured);
  const busy = search.isFetching || settled !== query;
  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="flex gap-2">
        <div className="relative flex-1">
          <Search className="absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" />
          <Input
            autoFocus
            aria-label="Search podcasts"
            placeholder="A show, a host or a subject"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            className="pl-8"
          />
        </div>
        <div
          role="radiogroup"
          aria-label="Search in"
          className="flex shrink-0 rounded-lg border p-0.5 text-[12px]"
        >
          {(
            [
              [false, "Apple Podcasts"],
              [true, "Podcast Index"],
            ] as const
          ).map(([v, label]) => (
            <button
              key={label}
              type="button"
              role="radio"
              aria-checked={useIndex === v}
              disabled={v && !index?.configured}
              title={
                v && !index?.configured
                  ? "Add your free Podcast Index key in Settings › Online details"
                  : undefined
              }
              onClick={() => setUseIndex(v)}
              className={cn(
                "rounded-md px-2.5 py-1 font-medium text-muted-foreground disabled:opacity-40",
                useIndex === v && "bg-muted text-foreground",
              )}
            >
              {label}
            </button>
          ))}
        </div>
      </div>
      {settled.trim().length < 2 ? (
        <p className="text-[12.5px] text-muted-foreground">
          Searches the Apple Podcasts directory (free, no account).
          {!index?.configured && (
            <>
              {" "}
              With your own Podcast Index key you can also search its open index.{" "}
              <button
                type="button"
                className="font-medium text-foreground underline underline-offset-2"
                onClick={onSettings}
              >
                Add a key
              </button>
            </>
          )}
        </p>
      ) : search.error ? (
        <p className="text-[12.5px] text-destructive">
          {search.error instanceof Error ? search.error.message : String(search.error)}
        </p>
      ) : (
        <Results shows={search.data} busy={busy} followed={followed} auto={auto} />
      )}
    </div>
  );
}

function PopularTab({
  followed,
  auto,
  onSettings,
}: {
  followed: Set<string>;
  auto: boolean;
  onSettings: () => void;
}) {
  const { data: index } = useIndexStatus();
  const on = !!index?.configured;
  const [category, setCategory] = useState("");
  const { data: categories = [] } = useCategories(on);
  const trending = useTrending(category || null, on);
  if (!on)
    return (
      <div className="flex flex-col items-start gap-3 text-[12.5px] text-muted-foreground">
        <p>
          Popular shows come from Podcast Index, an open directory. It needs a free key of your own:
          sign up at podcastindex.org, then paste the key and secret in Settings › Online details.
          They stay on this computer.
        </p>
        <Button variant="outline" size="sm" onClick={onSettings}>
          <Settings2 /> Open Settings
        </Button>
      </div>
    );
  return (
    <div className="flex min-h-0 flex-col gap-3">
      <label className="flex items-center gap-2 text-[12.5px]">
        <span className="text-muted-foreground">Subject</span>
        <NativeSelect
          value={category}
          onChange={(e) => setCategory(e.target.value)}
          className="w-56"
        >
          <option value="">Everything</option>
          {categories.map((c) => (
            <option key={c} value={c}>
              {c}
            </option>
          ))}
        </NativeSelect>
      </label>
      {trending.error ? (
        <p className="text-[12.5px] text-destructive">
          {trending.error instanceof Error ? trending.error.message : String(trending.error)}
        </p>
      ) : (
        <Results shows={trending.data} busy={trending.isFetching} followed={followed} auto={auto} />
      )}
    </div>
  );
}

function Results({
  shows,
  busy,
  followed,
  auto,
}: {
  shows: Show[] | undefined;
  busy: boolean;
  followed: Set<string>;
  auto: boolean;
}) {
  if (!shows)
    return busy ? (
      <RowsSkeleton rows={4} picture="square" label="Finding shows…" className="-mx-5" />
    ) : null;
  if (!shows.length)
    return <p className="py-6 text-center text-[12.5px] text-muted-foreground">No shows found.</p>;
  return (
    <ul
      aria-label="Shows"
      aria-busy={busy}
      className={cn("-mx-1 flex max-h-[46vh] flex-col overflow-auto", busy && "opacity-60")}
    >
      {shows.map((s) => (
        <ShowResult key={s.feedUrl} show={s} following={followed.has(s.feedUrl)} auto={auto} />
      ))}
    </ul>
  );
}

function ShowResult({
  show: s,
  following,
  auto,
}: {
  show: Show;
  following: boolean;
  auto: boolean;
}) {
  const add = useAddFeed("podcasts");
  const [done, setDone] = useState(false);
  const meta = [
    s.author,
    s.categories.slice(0, 2).join(", "),
    s.episodes ? `${s.episodes} episodes` : null,
  ].filter(Boolean);
  return (
    <li className="flex gap-3 rounded-lg px-1 py-2 hover:bg-muted/50">
      <Artwork src={s.artwork} title={s.title} className="size-14 text-[15px]" />
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="line-clamp-2 font-medium" title={s.title}>
          {s.title}
        </span>
        {meta.length > 0 && (
          <span className="truncate text-[12px] text-muted-foreground">{meta.join(" · ")}</span>
        )}
        {s.about && (
          <span className="line-clamp-2 text-[12px] text-muted-foreground">{s.about}</span>
        )}
      </div>
      <div className="shrink-0 self-center">
        {following || done ? (
          <span className="flex items-center gap-1 px-2 text-[12.5px] text-muted-foreground">
            <Check className="size-4" /> Following
          </span>
        ) : (
          <Button
            size="sm"
            variant="outline"
            disabled={add.isPending}
            onClick={() =>
              add.mutate(
                { url: s.feedUrl, title: s.title, folder: null, autoDownload: auto },
                {
                  onSuccess: () => {
                    setDone(true);
                    toast.success(`Following “${s.title}”`);
                  },
                },
              )
            }
          >
            {add.isPending ? <Loader2 className="animate-spin" /> : <Plus />} Follow
          </Button>
        )}
      </div>
    </li>
  );
}
