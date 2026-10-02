import { useEffect, useMemo, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, Loader2, NotebookPen, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { useReading } from "@/features/study";
import { commands, unwrap } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { clock } from "./model";
import { cueAt, momentMarkdown, saidAt, saveMoment } from "./moments";
import { usePlayer } from "./player";

/**
 * Beside the player: the episode's chapters and its transcript, which
 * follow along (click a line or a chapter to go there), and "Note this
 * moment".
 */
export function EpisodePanel() {
  const episode = usePlayer((s) => s.episode);
  const open = usePlayer((s) => s.panel);
  const setPanel = usePlayer((s) => s.setPanel);
  const time = usePlayer((s) => s.time);
  const seek = usePlayer((s) => s.seek);
  const [tab, setTab] = useState<"transcript" | "chapters">("transcript");
  const [follow, setFollow] = useState(true);
  const [noting, setNoting] = useState<number | null>(null);
  const id = episode?.id ?? "";

  const transcript = useQuery({
    queryKey: ["podcast-transcript", id],
    queryFn: () => unwrap(commands.podcastTranscript(id)),
    enabled: open && !!id,
    staleTime: Infinity,
  });
  const chapters = useQuery({
    queryKey: ["podcast-chapters", id],
    queryFn: () => unwrap(commands.podcastChapters(id)),
    enabled: open && !!id,
    staleTime: Infinity,
  });
  const cues = useMemo(() => transcript.data ?? [], [transcript.data]);
  const chs = useMemo(() => chapters.data ?? [], [chapters.data]);
  const current = cueAt(cues, time);
  const chapter = chs.reduce(
    (at, c, i) => (c.start !== null && c.start <= time + 0.25 ? i : at),
    -1,
  );

  // The line being said stays in view while following along.
  const list = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!follow || tab !== "transcript" || current < 0) return;
    list.current
      ?.querySelector<HTMLElement>(`[data-cue="${current}"]`)
      ?.scrollIntoView({ block: "center", behavior: "smooth" });
  }, [current, follow, tab]);

  if (!episode || !open) return null;
  const timed = cues.some((c) => c.start !== null);

  return (
    <aside
      aria-label="Episode: chapters and transcript"
      className="fixed top-14 right-3 bottom-24 z-40 flex w-[22rem] max-w-[calc(100vw-24px)] flex-col overflow-hidden rounded-2xl border bg-popover text-popover-foreground shadow-xl"
    >
      <header className="flex items-start gap-2 border-b p-3">
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px] font-semibold">{episode.title}</p>
          <p className="truncate text-[11.5px] text-muted-foreground">{episode.show}</p>
        </div>
        <Button
          size="sm"
          variant="outline"
          title="Write this moment, and what was being said, into your notes"
          onClick={() => setNoting(time)}
        >
          <NotebookPen /> Note this moment
        </Button>
        <Button
          variant="ghost"
          size="icon"
          className="size-7"
          aria-label="Close"
          onClick={() => setPanel(false)}
        >
          <X />
        </Button>
      </header>

      {noting !== null && (
        <NoteMoment seconds={noting} said={saidAt(cues, noting)} onDone={() => setNoting(null)} />
      )}

      <div className="flex items-center gap-2 border-b px-3 py-2">
        <div role="tablist" aria-label="Show" className="flex flex-1 rounded-lg bg-muted p-0.5">
          {(
            [
              ["transcript", "Transcript"],
              ["chapters", `Chapters${chs.length ? ` (${chs.length})` : ""}`],
            ] as const
          ).map(([k, name]) => (
            <button
              key={k}
              type="button"
              role="tab"
              aria-selected={tab === k}
              onClick={() => setTab(k)}
              className={cn(
                "flex-1 rounded-md py-1 text-[12.5px] font-medium",
                tab === k ? "bg-background shadow-sm" : "text-muted-foreground",
              )}
            >
              {name}
            </button>
          ))}
        </div>
        {tab === "transcript" && timed && (
          <label className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
            <input type="checkbox" checked={follow} onChange={(e) => setFollow(e.target.checked)} />
            Follow along
          </label>
        )}
      </div>

      <div ref={list} className="min-h-0 flex-1 overflow-auto px-2 py-2 text-[13px]">
        {tab === "transcript" ? (
          transcript.isLoading ? (
            <Loading />
          ) : transcript.isError ? (
            <Empty>{String((transcript.error as Error).message ?? transcript.error)}</Empty>
          ) : !cues.length ? (
            <Empty>This show has no transcript for this episode.</Empty>
          ) : (
            cues.map((c, i) => (
              <button
                key={i}
                type="button"
                data-cue={i}
                disabled={c.start === null}
                onClick={() => c.start !== null && seek(c.start)}
                className={cn(
                  "flex w-full gap-2 rounded-md px-2 py-1.5 text-left leading-relaxed",
                  c.start !== null && "hover:bg-muted",
                  i === current && "bg-amber-100/80 dark:bg-amber-500/20",
                  i < current && "text-muted-foreground",
                )}
              >
                {c.start !== null && (
                  <span className="w-10 shrink-0 pt-px text-[11px] text-muted-foreground tabular-nums">
                    {clock(c.start)}
                  </span>
                )}
                <span>
                  {c.speaker && <span className="font-medium">{c.speaker}: </span>}
                  {c.text}
                </span>
              </button>
            ))
          )
        ) : chapters.isLoading ? (
          <Loading />
        ) : !chs.length ? (
          <Empty>This show has no chapters for this episode.</Empty>
        ) : (
          chs.map((c, i) => (
            <button
              key={i}
              type="button"
              disabled={c.start === null}
              onClick={() => c.start !== null && seek(c.start)}
              className={cn(
                "flex w-full items-center gap-2 rounded-md px-2 py-2 text-left hover:bg-muted",
                i === chapter && "bg-muted font-medium",
              )}
            >
              <span className="w-12 shrink-0 text-[11.5px] text-muted-foreground tabular-nums">
                {c.start !== null ? clock(c.start) : ""}
              </span>
              <span className="min-w-0 flex-1 truncate">{c.title}</span>
              {i === chapter && (
                <span className="text-[11px] font-normal text-muted-foreground">Now</span>
              )}
            </button>
          ))
        )}
      </div>
    </aside>
  );
}

/** The moment to note: what was said, a thought, and where it goes. */
function NoteMoment({
  seconds,
  said,
  onDone,
}: {
  seconds: number;
  said: string;
  onDone: () => void;
}) {
  const episode = usePlayer((s) => s.episode);
  const reading = useReading((s) => s.now);
  const [text, setText] = useState("");
  const [busy, setBusy] = useState(false);
  if (!episode) return null;
  const place = reading
    ? { bookId: reading.bookId, title: reading.title, page: reading.page }
    : null;
  const save = async () => {
    setBusy(true);
    try {
      const md = momentMarkdown({
        id: episode.id,
        seconds,
        title: episode.title,
        show: episode.show,
        said,
        place,
        text,
      });
      toast.success(await saveMoment(md, place));
      onDone();
    } catch (e) {
      toast.error("Could not save the moment", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col gap-2 border-b bg-muted/40 p-3 text-[12.5px]">
      <p className="font-medium">Moment at {clock(seconds)}</p>
      {said && <blockquote className="border-l-2 pl-2 text-muted-foreground">{said}</blockquote>}
      <textarea
        aria-label="Your thought"
        autoFocus
        rows={2}
        value={text}
        placeholder="A thought about it (optional)"
        onChange={(e) => setText(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) void save();
        }}
        className="rounded-md border bg-background px-2 py-1"
      />
      <p className="flex items-center gap-1.5 text-[11.5px] text-muted-foreground">
        <BookOpen className="size-3.5 shrink-0" aria-hidden />
        {place
          ? `Goes to the notebook of ${place.title}${place.page != null ? `, linked to p. ${place.page}` : ""}`
          : "Goes to the note “Podcast moments” (open a book to link it to a page)"}
      </p>
      <div className="flex justify-end gap-2">
        <Button size="sm" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button size="sm" disabled={busy} onClick={() => void save()}>
          Save
        </Button>
      </div>
    </div>
  );
}

function Loading() {
  return (
    <p className="flex items-center gap-2 p-3 text-muted-foreground">
      <Loader2 className="size-4 animate-spin" /> Loading…
    </p>
  );
}

function Empty({ children }: { children: React.ReactNode }) {
  return <p className="p-3 text-muted-foreground">{children}</p>;
}
