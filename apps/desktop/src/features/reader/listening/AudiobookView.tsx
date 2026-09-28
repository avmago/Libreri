import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AlertTriangle,
  BookOpen,
  Bookmark,
  ChevronFirst,
  ChevronLast,
  Link2,
  Link2Off,
  ListMusic,
  Loader2,
  Moon,
  Pause,
  Play,
  Plus,
  RotateCcw,
  RotateCw,
  Trash2,
  Volume2,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { DropdownMenu, menuContent, menuItem } from "@/components/ui/menu";
import { useBook } from "@/features/library";
import { usePermissions, useProfilePrefs } from "@/features/profiles";
import {
  bookUrl,
  commands,
  unwrap,
  type Annotation,
  type AudioLinkDto,
  type BookDto,
  type SyncPointDto,
} from "@/lib/ipc";
import { useTabs, type BookTab } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import { parseLocator } from "@/readers";
import {
  savePosition,
  useAnnotations,
  useDeleteAnnotation,
  usePosition,
  useSaveAnnotation,
} from "../api";
import { SPEEDS } from "../speech/speeds";
import { useListening } from "./store";
import { chapterAt, clock, progressAt } from "./sync";

const SLEEP = [5, 10, 15, 30, 45, 60];

/** Sync points with plain numbers (the bindings allow null). */
const points = (link: AudioLinkDto | undefined) =>
  (link?.points ?? []).map((p) => ({ ...p, t: p.t ?? 0, progress: p.progress ?? 0 }));

/**
 * The audiobook player (Phase 7a): chapters, speed, sleep timer,
 * bookmarks, and the book it reads kept in step through sync points.
 */
export function AudiobookView({ tab, active }: { tab: BookTab; active: boolean }) {
  const { bookId } = tab;
  const { data: book, error: bookError } = useBook(bookId);
  const position = usePosition(bookId);
  const { data: annotations = [] } = useAnnotations(bookId);
  const saveAnnotation = useSaveAnnotation(bookId);
  const deleteAnnotation = useDeleteAnnotation(bookId);
  const { editLibrary } = usePermissions();
  const listening = useProfilePrefs((s) => s.prefs.listening);
  const updatePrefs = useProfilePrefs((s) => s.update);
  const qc = useQueryClient();

  const { data: info } = useQuery({
    queryKey: ["lib", "reader", bookId, "audio-info"],
    queryFn: () => unwrap(commands.audioInfo(bookId)),
    staleTime: Infinity,
  });
  const linkKey = ["lib", "reader", bookId, "audio-link"];
  const { data: link } = useQuery({
    queryKey: linkKey,
    queryFn: () => unwrap(commands.getAudioLink(bookId)),
  });
  const setLink = useMutation({
    mutationFn: (text: string | null) => unwrap(commands.setAudioLink(bookId, text)),
    onSuccess: (l) => qc.setQueryData(linkKey, l),
    onError: (e) => toast.error(String(e)),
  });
  const setPoints = useMutation({
    mutationFn: (list: SyncPointDto[]) => unwrap(commands.setSyncPoints(bookId, list)),
    onSuccess: (l) => qc.setQueryData(linkKey, l),
    onError: (e) => toast.error(String(e)),
  });

  const audio = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [panel, setPanel] = useState<"chapters" | "bookmarks" | "book">("chapters");
  const [sleep, setSleep] = useState<{ until: number | null; chapter: number | null } | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const [follow, setFollow] = useState(true);
  const [linking, setLinking] = useState(false);

  const chapters = useMemo(
    () => (info?.chapters ?? []).map((c) => ({ title: c.title, start: c.start ?? 0 })),
    [info],
  );
  const chapter = chapterAt(chapters, time);
  const total = duration || info?.duration || 0;

  // Where to start: the saved place, or a request from the text ("Listen from here").
  const startAt = useMemo(() => {
    const loc = parseLocator(position.data);
    return loc?.type === "audio" ? loc.t : 0;
  }, [position.data]);
  const started = useRef(false);
  const onLoaded = () => {
    const a = audio.current;
    if (!a) return;
    setDuration(Number.isFinite(a.duration) ? a.duration : 0);
    a.playbackRate = listening.audioRate;
    if (!started.current) {
      started.current = true;
      const req = useListening.getState().playFrom[bookId];
      a.currentTime = req ? req.t : startAt;
      if (req) {
        useListening.getState().clearPlay(bookId);
        void a.play().catch(() => {});
      }
    }
  };
  // Later requests while open.
  const request = useListening((s) => s.playFrom[bookId]);
  useEffect(() => {
    const a = audio.current;
    if (!request || !a || !started.current) return;
    a.currentTime = request.t;
    void a.play().catch(() => {});
    useListening.getState().clearPlay(bookId);
  }, [request, bookId]);

  useEffect(() => {
    if (audio.current) audio.current.playbackRate = listening.audioRate;
  }, [listening.audioRate]);

  // Save the place now and then, and when pausing.
  const save = useCallback(() => {
    const a = audio.current;
    if (!a || !started.current) return;
    const t = a.currentTime;
    void savePosition(
      bookId,
      JSON.stringify({ type: "audio", t: Math.round(t * 10) / 10 }),
      a.duration ? t / a.duration : 0,
    ).catch(() => {});
  }, [bookId]);
  useEffect(() => {
    if (!playing) return;
    const id = setInterval(save, 5000);
    return () => clearInterval(id);
  }, [playing, save]);
  useEffect(() => () => save(), [save]);

  const toggle = () => {
    const a = audio.current;
    if (!a) return;
    if (a.paused) void a.play().catch((e: Error) => setError(e.message));
    else a.pause();
  };
  const seek = (t: number) => {
    const a = audio.current;
    if (!a) return;
    a.currentTime = Math.max(0, Math.min(t, (a.duration || total) - 0.2));
    setTime(a.currentTime);
  };
  const skip = (s: number) => seek((audio.current?.currentTime ?? 0) + s);
  const goChapter = (dir: 1 | -1) => {
    if (!chapters.length) return skip(dir * 300);
    const t = audio.current?.currentTime ?? 0;
    // Back: to the start of this chapter, or the one before when near its start.
    const target =
      dir > 0
        ? chapters[chapter + 1]
        : t - (chapters[chapter]?.start ?? 0) > 3
          ? chapters[chapter]
          : chapters[chapter - 1];
    if (target) seek(target.start);
  };

  // Sleep timer: stop at a time or at the end of the chapter, fading out.
  const chapterRef = useRef(chapter);
  useEffect(() => {
    chapterRef.current = chapter;
  }, [chapter]);
  useEffect(() => {
    if (!sleep) return;
    const id = setInterval(() => {
      const a = audio.current;
      const t = Date.now();
      setNow(t);
      if (!a) return;
      const endChapter = sleep.chapter !== null && chapterRef.current !== sleep.chapter;
      const left = sleep.until !== null ? (sleep.until - t) / 1000 : Infinity;
      if (left < 8 && left > 0) a.volume = Math.max(0, left / 8);
      if (left <= 0 || endChapter) {
        a.pause();
        a.volume = 1;
        setSleep(null);
        toast("Sleep timer: stopped");
      }
    }, 500);
    return () => clearInterval(id);
  }, [sleep]);

  // Media keys and the system's now-playing controls.
  useEffect(() => {
    const ms = navigator.mediaSession;
    if (!ms || !book || !active) return;
    ms.metadata = new MediaMetadata({
      title: book.metadata.title ?? "",
      artist: (book.metadata.authors ?? []).join(", "),
      album: chapters[chapter]?.title ?? "",
      artwork: book.cover ? [{ src: bookUrl(book.cover) }] : [],
    });
    const handlers: [MediaSessionAction, () => void][] = [
      ["play", () => void audio.current?.play()],
      ["pause", () => audio.current?.pause()],
      ["seekbackward", () => skip(-listening.skipBack)],
      ["seekforward", () => skip(listening.skipForward)],
      ["previoustrack", () => goChapter(-1)],
      ["nexttrack", () => goChapter(1)],
    ];
    for (const [a, h] of handlers) {
      try {
        ms.setActionHandler(a, h);
      } catch {
        /* not supported here */
      }
    }
    return () => {
      for (const [a] of handlers) {
        try {
          ms.setActionHandler(a, null);
        } catch {
          /* ignore */
        }
      }
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [book, active, chapter]);

  // The text follows the audio.
  const textId = link?.text?.id;
  const syncPoints = points(link);
  const followTo = useListening((s) => s.followTo);
  const lastFollow = useRef(0);
  useEffect(() => {
    if (!playing || !follow || !textId || !total) return;
    if (Math.abs(time - lastFollow.current) < 2) return;
    lastFollow.current = time;
    followTo(textId, progressAt(syncPoints, time, total));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [time, playing, follow, textId, total]);

  const textPlace = useListening((s) => (textId ? s.places[textId] : undefined));
  const addSyncPoint = () => {
    if (!textPlace) return;
    const t = audio.current?.currentTime ?? time;
    setPoints.mutate([
      ...syncPoints,
      {
        t,
        locator: textPlace.locator,
        progress: textPlace.progress,
        label: textPlace.label,
        auto: false,
      },
    ]);
    toast.success(`Sync point: ${clock(t)} is ${textPlace.label || "this place"}`);
  };

  const bookmarks = annotations
    .filter((a) => a.kind === "bookmark")
    .map((a) => ({ a, t: (parseLocator(a.locator) as { t?: number } | null)?.t ?? 0 }))
    .sort((x, y) => x.t - y.t);
  const addBookmark = () => {
    const t = audio.current?.currentTime ?? time;
    const where = chapters[chapter]?.title;
    const a: Annotation = {
      id: crypto.randomUUID(),
      bookId,
      kind: "bookmark",
      color: null,
      locator: JSON.stringify({ type: "audio", t: Math.round(t * 10) / 10 }),
      quote: null,
      note: null,
      label: where ? `${where} · ${clock(t)}` : clock(t),
      position: total ? t / total : 0,
      createdAt: "",
      modifiedAt: "",
    };
    saveAnnotation.mutate(a, { onSuccess: () => toast("Bookmark added") });
  };

  const openText = (b: BookDto) =>
    useTabs.getState().openBeside({
      bookId: b.id,
      title: b.metadata.title ?? "Book",
      fileType: b.fileType,
    });

  if (bookError)
    return (
      <div className="flex h-full items-center justify-center text-muted-foreground">
        This audiobook is no longer in the library.
      </div>
    );

  const sleepLeft = sleep?.until ? Math.max(0, (sleep.until - now) / 1000) : null;

  return (
    <div
      className="flex h-full min-h-0 flex-col outline-none"
      tabIndex={-1}
      onKeyDown={(e) => {
        if ((e.target as HTMLElement).closest("input, select, textarea, button")) return;
        if (e.key === " ") {
          e.preventDefault();
          toggle();
        } else if (e.key === "ArrowLeft") skip(-listening.skipBack);
        else if (e.key === "ArrowRight") skip(listening.skipForward);
      }}
    >
      <audio
        ref={audio}
        src={book?.relPath ? bookUrl(book.relPath) : undefined}
        preload="metadata"
        onLoadedMetadata={onLoaded}
        onDurationChange={(e) =>
          Number.isFinite(e.currentTarget.duration) && setDuration(e.currentTarget.duration)
        }
        onTimeUpdate={(e) => setTime(e.currentTarget.currentTime)}
        onPlay={() => {
          setPlaying(true);
          setError(null);
        }}
        onPause={() => {
          setPlaying(false);
          save();
        }}
        onError={() =>
          setError(
            "This audio file cannot be played here. The system may lack its codec; try opening it in another app.",
          )
        }
      />

      <div className="flex min-h-0 flex-1">
        {/* Player */}
        <div className="flex min-w-0 flex-1 flex-col items-center justify-center gap-5 overflow-auto p-8">
          <div className="aspect-square w-64 max-w-full overflow-hidden rounded-lg bg-muted shadow-lg">
            {book?.cover ? (
              <img
                src={bookUrl(book.cover)}
                alt=""
                className="size-full object-cover"
                draggable={false}
              />
            ) : (
              <div className="flex size-full items-center justify-center text-muted-foreground">
                <ListMusic className="size-16" aria-hidden />
              </div>
            )}
          </div>
          <div className="flex max-w-xl flex-col items-center gap-1 text-center">
            <h1 className="text-lg font-semibold">{book?.metadata.title ?? tab.title}</h1>
            <p className="text-muted-foreground">{(book?.metadata.authors ?? []).join(", ")}</p>
            {chapters[chapter] && (
              <p className="text-[13px]">
                {chapters[chapter]!.title}{" "}
                <span className="text-muted-foreground">
                  ({chapter + 1} of {chapters.length})
                </span>
              </p>
            )}
          </div>

          {/* Seek bar with chapter marks */}
          <div className="flex w-full max-w-2xl flex-col gap-1">
            <div className="relative">
              <input
                type="range"
                min={0}
                max={total || 1}
                step={1}
                value={Math.min(time, total || 1)}
                onChange={(e) => seek(Number(e.target.value))}
                className="w-full"
                aria-label="Position"
                aria-valuetext={`${clock(time)} of ${clock(total)}`}
              />
              {total > 0 &&
                chapters
                  .slice(1)
                  .map((c, i) => (
                    <span
                      key={i}
                      className="pointer-events-none absolute top-1/2 h-2.5 w-px -translate-y-1/2 bg-foreground/40"
                      style={{ left: `${(c.start / total) * 100}%` }}
                    />
                  ))}
            </div>
            <div className="flex justify-between text-[12px] text-muted-foreground tabular-nums">
              <span>{clock(time)}</span>
              <span>
                −{clock(Math.max(0, total - time) / listening.audioRate)}
                {listening.audioRate !== 1 && ` at ${listening.audioRate}×`}
              </span>
            </div>
          </div>

          <div className="flex items-center gap-2">
            <Button
              variant="ghost"
              size="icon"
              aria-label="Previous chapter"
              onClick={() => goChapter(-1)}
            >
              <ChevronFirst />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Back ${listening.skipBack} seconds`}
              title={`Back ${listening.skipBack} s (←)`}
              onClick={() => skip(-listening.skipBack)}
            >
              <RotateCcw />
            </Button>
            <Button
              className="size-14 rounded-full [&_svg]:size-6"
              aria-label={playing ? "Pause" : "Play"}
              title="Play or pause (Space)"
              onClick={toggle}
              disabled={!book}
            >
              {playing ? <Pause /> : <Play />}
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Forward ${listening.skipForward} seconds`}
              title={`Forward ${listening.skipForward} s (→)`}
              onClick={() => skip(listening.skipForward)}
            >
              <RotateCw />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Next chapter"
              onClick={() => goChapter(1)}
            >
              <ChevronLast />
            </Button>
          </div>

          <div className="flex flex-wrap items-center justify-center gap-2 text-[12.5px]">
            <select
              aria-label="Speed"
              value={listening.audioRate}
              onChange={(e) => updatePrefs({ listening: { audioRate: Number(e.target.value) } })}
              className="h-8 rounded-md border border-input bg-background px-1.5"
            >
              {SPEEDS.map((s) => (
                <option key={s} value={s}>
                  {s}×
                </option>
              ))}
            </select>
            <DropdownMenu.Root>
              <DropdownMenu.Trigger asChild>
                <Button variant={sleep ? "outline" : "ghost"} size="sm">
                  <Moon />
                  {sleep
                    ? sleepLeft !== null
                      ? clock(sleepLeft)
                      : "End of chapter"
                    : "Sleep timer"}
                </Button>
              </DropdownMenu.Trigger>
              <DropdownMenu.Portal>
                <DropdownMenu.Content className={menuContent} sideOffset={4}>
                  {SLEEP.map((m) => (
                    <DropdownMenu.Item
                      key={m}
                      className={menuItem}
                      onSelect={() => {
                        setNow(Date.now());
                        setSleep({ until: Date.now() + m * 60_000, chapter: null });
                      }}
                    >
                      {m} minutes
                    </DropdownMenu.Item>
                  ))}
                  <DropdownMenu.Item
                    className={menuItem}
                    disabled={!chapters.length}
                    onSelect={() => setSleep({ until: null, chapter })}
                  >
                    End of this chapter
                  </DropdownMenu.Item>
                  {sleep && (
                    <DropdownMenu.Item className={menuItem} onSelect={() => setSleep(null)}>
                      Turn off
                    </DropdownMenu.Item>
                  )}
                </DropdownMenu.Content>
              </DropdownMenu.Portal>
            </DropdownMenu.Root>
            <Button variant="ghost" size="sm" onClick={addBookmark}>
              <Bookmark /> Bookmark
            </Button>
            <label className="flex items-center gap-1.5 text-muted-foreground">
              <Volume2 className="size-4" aria-hidden />
              <input
                type="range"
                min={0}
                max={1}
                step={0.05}
                defaultValue={1}
                onChange={(e) => {
                  if (audio.current) audio.current.volume = Number(e.target.value);
                }}
                aria-label="Volume"
                className="w-24"
              />
            </label>
          </div>
          {error && (
            <p className="flex max-w-md items-center gap-2 text-center text-[12.5px] text-destructive">
              <AlertTriangle className="size-4 shrink-0" /> {error}
            </p>
          )}
        </div>

        {/* Side panel */}
        <aside className="flex w-80 shrink-0 flex-col border-l">
          <div role="tablist" className="flex gap-1 border-b p-1.5">
            {(
              [
                ["chapters", `Chapters${chapters.length ? ` (${chapters.length})` : ""}`],
                ["bookmarks", `Bookmarks${bookmarks.length ? ` (${bookmarks.length})` : ""}`],
                ["book", "Book"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={panel === id}
                onClick={() => setPanel(id)}
                className={cn(
                  "flex-1 rounded-md py-1 text-[12.5px] text-muted-foreground",
                  panel === id && "bg-muted font-medium text-foreground",
                )}
              >
                {label}
              </button>
            ))}
          </div>
          <div className="min-h-0 flex-1 overflow-auto text-[13px]">
            {panel === "chapters" &&
              (chapters.length ? (
                <ol className="flex flex-col py-1">
                  {chapters.map((c, i) => (
                    <li key={i}>
                      <button
                        type="button"
                        onClick={() => seek(c.start)}
                        className={cn(
                          "flex w-full items-baseline gap-2 px-3 py-1.5 text-left hover:bg-muted",
                          i === chapter && "bg-muted font-medium",
                        )}
                      >
                        <span className="min-w-0 flex-1 truncate">
                          {c.title || `Chapter ${i + 1}`}
                        </span>
                        <span className="text-[11.5px] text-muted-foreground tabular-nums">
                          {clock(c.start)}
                        </span>
                      </button>
                    </li>
                  ))}
                </ol>
              ) : (
                <p className="p-4 text-muted-foreground">
                  {info ? "This file has no chapters." : "Reading the chapters…"}
                </p>
              ))}
            {panel === "bookmarks" &&
              (bookmarks.length ? (
                <ul className="flex flex-col py-1">
                  {bookmarks.map(({ a, t }) => (
                    <li key={a.id} className="group flex items-center hover:bg-muted">
                      <button
                        type="button"
                        onClick={() => seek(t)}
                        className="min-w-0 flex-1 truncate px-3 py-1.5 text-left"
                      >
                        {a.label || clock(t)}
                      </button>
                      <Button
                        variant="ghost"
                        size="icon"
                        className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                        aria-label="Delete bookmark"
                        onClick={() => deleteAnnotation.mutate(a.id)}
                      >
                        <Trash2 />
                      </Button>
                    </li>
                  ))}
                </ul>
              ) : (
                <p className="p-4 text-muted-foreground">
                  No bookmarks yet. Press Bookmark while listening to mark a moment.
                </p>
              ))}
            {panel === "book" && (
              <div className="flex flex-col gap-3 p-3">
                {link?.text ? (
                  <>
                    <div className="flex items-start gap-2">
                      <BookOpen className="mt-0.5 size-4 shrink-0 text-muted-foreground" />
                      <div className="min-w-0 flex-1">
                        <p className="font-medium">{link.text.metadata.title}</p>
                        <p className="text-[12px] text-muted-foreground">
                          The book this audiobook reads
                        </p>
                      </div>
                    </div>
                    <div className="flex flex-wrap gap-1.5">
                      <Button size="sm" variant="outline" onClick={() => openText(link.text!)}>
                        Open beside
                      </Button>
                      {editLibrary && (
                        <Button
                          size="sm"
                          variant="ghost"
                          onClick={() => setLink.mutate(null)}
                          title="Unlink (sync points are forgotten)"
                        >
                          <Link2Off /> Unlink
                        </Button>
                      )}
                    </div>
                    <label className="flex items-center gap-2">
                      <input
                        type="checkbox"
                        checked={follow}
                        onChange={(e) => setFollow(e.target.checked)}
                      />
                      The open book follows the audio
                    </label>
                    <div className="flex flex-col gap-1.5 border-t pt-3">
                      <p className="font-medium">Sync points</p>
                      <p className="text-[12px] text-muted-foreground">
                        Match a moment of the audio to a place in the book: open the book beside, go
                        to where this moment is read, and add a sync point. Between points, Libreri
                        keeps both in step.
                      </p>
                      {editLibrary && (
                        <Button
                          size="sm"
                          className="self-start"
                          disabled={!textPlace}
                          onClick={addSyncPoint}
                          title={textPlace ? undefined : "Open the book first"}
                        >
                          <Plus /> {clock(time)} is{" "}
                          {textPlace ? textPlace.label || "the open place" : "…"}
                        </Button>
                      )}
                      <ul className="flex flex-col">
                        {syncPoints.map((p, i) => (
                          <li key={i} className="group flex items-center gap-2 py-0.5">
                            <button
                              type="button"
                              className="min-w-0 flex-1 truncate text-left hover:underline"
                              onClick={() => seek(p.t)}
                            >
                              <span className="tabular-nums">{clock(p.t)}</span>
                              <span className="text-muted-foreground">
                                {" "}
                                → {p.label || `${Math.round(p.progress * 100)}%`}
                              </span>
                            </button>
                            {editLibrary && (
                              <Button
                                variant="ghost"
                                size="icon"
                                aria-label="Remove sync point"
                                className="opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                                onClick={() =>
                                  setPoints.mutate(syncPoints.filter((_, j) => j !== i))
                                }
                              >
                                <Trash2 />
                              </Button>
                            )}
                          </li>
                        ))}
                      </ul>
                    </div>
                  </>
                ) : (
                  <>
                    <p className="text-muted-foreground">
                      Link this audiobook to the book it reads to switch between listening and
                      reading, and to have the book follow along.
                    </p>
                    {editLibrary && (
                      <Button size="sm" className="self-start" onClick={() => setLinking(true)}>
                        <Link2 /> Link to a book…
                      </Button>
                    )}
                  </>
                )}
              </div>
            )}
          </div>
        </aside>
      </div>
      <LinkBookDialog
        open={linking}
        title={book?.metadata.title ?? ""}
        onClose={() => setLinking(false)}
        onPick={(id) => {
          setLinking(false);
          setLink.mutate(id);
        }}
      />
    </div>
  );
}

/** Picks the book an audiobook reads (same title first). */
function LinkBookDialog({
  open,
  title,
  onClose,
  onPick,
}: {
  open: boolean;
  title: string;
  onClose: () => void;
  onPick: (id: string) => void;
}) {
  const [search, setSearch] = useState(title);
  const { data: books = [], isFetching } = useQuery({
    queryKey: ["lib", "link-books", search],
    queryFn: () => unwrap(commands.listBooks({ search: search || null, sort: "title" })),
    enabled: open,
  });
  const texts = books.filter(
    (b) => !["mp3", "m4b", "m4a", "aac", "ogg", "opus", "flac"].includes(b.fileType),
  );
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Link to a book"
      description="The book with the text this audiobook reads."
      className="w-[520px]"
    >
      <input
        autoFocus
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder="Find a book"
        className="h-8 rounded-md border border-input bg-background px-2 outline-none focus-visible:border-ring"
      />
      <ul className="flex max-h-72 flex-col overflow-auto rounded-md border">
        {texts.map((b) => (
          <li key={b.id}>
            <button
              type="button"
              onClick={() => onPick(b.id)}
              className="flex w-full flex-col items-start px-3 py-1.5 text-left hover:bg-muted"
            >
              <span className="font-medium">{b.metadata.title}</span>
              <span className="text-[12px] text-muted-foreground">
                {(b.metadata.authors ?? []).join(", ")} · {b.fileType.toUpperCase()}
              </span>
            </button>
          </li>
        ))}
        {!texts.length && (
          <li className="flex items-center gap-2 px-3 py-2 text-muted-foreground">
            {isFetching && <Loader2 className="size-3.5 animate-spin" />}
            {isFetching ? "Looking…" : "No books found."}
          </li>
        )}
      </ul>
    </Dialog>
  );
}
