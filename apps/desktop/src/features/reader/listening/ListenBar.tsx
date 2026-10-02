import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { useQuery } from "@tanstack/react-query";
import {
  ChevronDown,
  ChevronsDown,
  ExternalLink,
  Loader2,
  Minus,
  Pause,
  Play,
  Plus,
  SkipBack,
  SkipForward,
  Timer,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { DropdownMenu, menuContent, menuItem } from "@/components/ui/menu";
import { useHelperDialog } from "@/features/helpers";
import { useProfilePrefs } from "@/features/profiles";
import { bookUrl, commands, unwrap, type BookDto } from "@/lib/ipc";
import { PodcastControls, usePlayer } from "@/features/podcasts";
import { MiniPlayer } from "@/components/MiniPlayer";
import { useFloatingBar, useMiniPlay, useReportPlay } from "@/lib/floating";
import { useTabs } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import { savePosition } from "../api";
import { getEngine } from "../speech/engine";
import { baseLang, chooseVoice } from "../speech/choose";
import { openVoicesSettings } from "../speech/natural";
import { SPEEDS } from "../speech/speeds";
import type { ReadAloud } from "../speech/useReadAloud";
import { useListening } from "./store";
import { chapterAt, clock, progressAt, timeAt } from "./sync";

export type ListenMode = "read" | "audio" | "podcast";

const SLEEP = [5, 10, 15, 30, 45, 60];
const isLinux = /Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent);

/** One step faster or slower through the offered speeds. */
function step(rate: number, dir: 1 | -1): number {
  const i = SPEEDS.findIndex((s) => s >= rate - 0.001);
  const at = i < 0 ? SPEEDS.length - 1 : i;
  return SPEEDS[Math.min(SPEEDS.length - 1, Math.max(0, at + dir))]!;
}

/** Stops after some minutes (or at the end of the audiobook's chapter). */
function useSleep(stop: () => void) {
  const [until, setUntil] = useState<number | "chapter" | null>(null);
  const [now, setNow] = useState(() => Date.now());
  const stopRef = useRef(stop);
  useEffect(() => {
    stopRef.current = stop;
  });
  useEffect(() => {
    if (typeof until !== "number") return;
    const id = setInterval(() => {
      const t = Date.now();
      setNow(t);
      if (t >= until) {
        setUntil(null);
        stopRef.current();
        toast("Sleep timer: stopped");
      }
    }, 1000);
    return () => clearInterval(id);
  }, [until]);
  const left = typeof until === "number" ? Math.max(0, (until - now) / 1000) : null;
  return { until, setUntil, left };
}

/**
 * The floating player at the bottom of the page (board 4, "Read aloud ·
 * Audiobook"): reads the book aloud with a system voice, or plays its
 * linked audiobook from the place being read. Either way the book follows
 * along and turns pages.
 */
export function ListenBar({
  mode,
  onMode,
  onClose,
  readAloud: r,
  lang,
  bookId,
  audiobooks,
  progress,
}: {
  mode: ListenMode;
  onMode: (m: ListenMode) => void;
  onClose: () => void;
  readAloud: ReadAloud;
  lang?: string;
  bookId: string;
  /** Audiobooks linked to this book (the first is played). */
  audiobooks: BookDto[];
  /** Where the reader is (0–1), to start the audiobook there. */
  progress: number;
}) {
  const audio = audiobooks[0] ?? null;
  const podcast = usePlayer((s) => s.episode !== null);
  const collapsed = useFloatingBar((s) => s.collapsed);
  const setCollapsed = useFloatingBar((s) => s.setCollapsed);
  const side = useFloatingBar((s) => s.side);
  const mini = useMiniPlay();
  const name = mode === "read" ? "Read aloud" : mode === "audio" ? "Audiobook" : "Podcast";
  return (
    <>
      {collapsed && (
        <MiniPlayer
          label={name}
          playing={mini.playing}
          busy={mini.busy}
          onToggle={() => mini.toggle?.()}
          // Clear of the full-screen button in the corner.
          className={side === "right" ? "right-16" : undefined}
        />
      )}
      <div
        role="toolbar"
        aria-label={name}
        className={cn(
          "absolute bottom-5 left-1/2 z-30 flex w-max max-w-[calc(100%-2rem)] -translate-x-1/2 items-center gap-1.5 rounded-2xl border bg-popover/95 p-1.5 text-[12.5px] text-popover-foreground shadow-xl backdrop-blur",
          // Folded: hidden, but still playing.
          collapsed && "hidden",
        )}
      >
        <Button
          variant="ghost"
          size="icon"
          className="size-7 shrink-0"
          aria-label="Fold the player"
          title="Fold the player into a small box"
          onClick={() => setCollapsed(true)}
        >
          <ChevronsDown />
        </Button>
        <div
          role="tablist"
          aria-label="Listen with"
          className="flex shrink-0 rounded-xl bg-muted p-0.5"
        >
          {(
            [
              ["read", "Read aloud"],
              ["audio", "Audiobook"],
              ["podcast", "Podcast"],
            ] as const
          )
            .filter(([id]) => id !== "podcast" || podcast)
            .map(([id, label]) => (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={mode === id}
                disabled={id === "audio" && !audio}
                title={id === "audio" && !audio ? "Link an audiobook in Edit details" : undefined}
                onClick={() => onMode(id)}
                className={cn(
                  "rounded-[10px] px-2.5 py-1 leading-tight font-medium whitespace-nowrap disabled:opacity-40",
                  // Narrow: only the one in use.
                  mode !== id && "@max-2xl:hidden",
                  mode === id
                    ? "bg-background shadow-sm"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {label}
              </button>
            ))}
        </div>
        {mode === "read" ? (
          <ReadPart r={r} lang={lang} onClose={onClose} />
        ) : mode === "podcast" ? (
          <PodcastPart onClose={onClose} />
        ) : audio ? (
          <AudioPart
            key={audio.id}
            audio={audio}
            bookId={bookId}
            progress={progress}
            onClose={onClose}
          />
        ) : null}
      </div>
    </>
  );
}

/** The podcast playing (the window's podcast player hides over books). */
function PodcastPart({ onClose }: { onClose: () => void }) {
  const episode = usePlayer((s) => s.episode);
  useEffect(() => {
    if (!episode) onClose();
  }, [episode, onClose]);
  const playing = usePlayer((s) => s.playing);
  const loading = usePlayer((s) => s.loading);
  useReportPlay(playing, loading && playing, () => usePlayer.getState().toggle());
  if (!episode) return null;
  return (
    <>
      <div className="flex min-w-0 max-w-64 flex-col px-1 leading-tight @max-3xl:hidden">
        <span className="truncate font-medium" title={episode.title}>
          {episode.title}
        </span>
        <span className="truncate text-[11.5px] text-muted-foreground" title={episode.show}>
          {episode.show}
        </span>
      </div>
      <PodcastControls compact />
    </>
  );
}

function SleepMenu({ sleep, chapters }: { sleep: ReturnType<typeof useSleep>; chapters: boolean }) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button
          variant={sleep.until ? "outline" : "ghost"}
          size={sleep.until ? "sm" : "icon"}
          aria-label="Sleep timer"
          className="@max-lg:hidden"
          title="Sleep timer"
        >
          <Timer />
          {sleep.left !== null ? clock(sleep.left) : sleep.until === "chapter" ? "Chapter" : null}
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className={menuContent} sideOffset={6} side="top">
          {SLEEP.map((m) => (
            <DropdownMenu.Item
              key={m}
              className={menuItem}
              onSelect={() => sleep.setUntil(Date.now() + m * 60_000)}
            >
              Stop in {m} minutes
            </DropdownMenu.Item>
          ))}
          {chapters && (
            <DropdownMenu.Item className={menuItem} onSelect={() => sleep.setUntil("chapter")}>
              Stop at the end of this chapter
            </DropdownMenu.Item>
          )}
          {sleep.until && (
            <DropdownMenu.Item className={menuItem} onSelect={() => sleep.setUntil(null)}>
              Turn off
            </DropdownMenu.Item>
          )}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

function Speed({ rate, onChange }: { rate: number; onChange: (r: number) => void }) {
  return (
    <div className="flex shrink-0 items-center rounded-lg border @max-xl:hidden">
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        aria-label="Slower"
        disabled={rate <= SPEEDS[0]!}
        onClick={() => onChange(step(rate, -1))}
      >
        <Minus />
      </Button>
      <span className="w-12 text-center font-mono text-[12.5px] tabular-nums" aria-live="polite">
        {rate}×
      </span>
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        aria-label="Faster"
        disabled={rate >= SPEEDS[SPEEDS.length - 1]!}
        onClick={() => onChange(step(rate, 1))}
      >
        <Plus />
      </Button>
    </div>
  );
}

function FollowToggle() {
  const follow = useProfilePrefs((s) => s.prefs.listening.follow);
  const update = useProfilePrefs((s) => s.update);
  return (
    <button
      type="button"
      className="hover:text-foreground hover:underline"
      title="Turn pages to follow along (click to change)"
      onClick={() => update({ listening: { follow: !follow } })}
    >
      {follow ? "follows along and turns pages" : "not following the page"}
    </button>
  );
}

function PlayControls({
  playing,
  busy,
  onToggle,
  onBack,
  onForward,
  back,
  forward,
}: {
  playing: boolean;
  busy?: boolean;
  onToggle: () => void;
  onBack: () => void;
  onForward: () => void;
  back: string;
  forward: string;
}) {
  return (
    <div className="flex shrink-0 items-center gap-0.5">
      <Button variant="ghost" size="icon" aria-label={back} title={back} onClick={onBack}>
        <SkipBack />
      </Button>
      <Button
        size="icon"
        className="size-9 rounded-full"
        aria-label={playing ? "Pause" : "Play"}
        onClick={onToggle}
        disabled={busy}
      >
        {busy ? <Loader2 className="animate-spin" /> : playing ? <Pause /> : <Play />}
      </Button>
      <Button variant="ghost" size="icon" aria-label={forward} title={forward} onClick={onForward}>
        <SkipForward />
      </Button>
    </div>
  );
}

const MORE = "__more";
const GROUP_LABEL = {
  kokoro: "Natural · Kokoro",
  piper: "Natural · Piper",
  system: "This computer",
} as const;

/** Reading aloud with a natural or a system voice. */
function ReadPart({ r, lang, onClose }: { r: ReadAloud; lang?: string; onClose: () => void }) {
  const listening = useProfilePrefs((s) => s.prefs.listening);
  const update = useProfilePrefs((s) => s.update);
  const openHelper = useHelperDialog((s) => s.open);
  const sleep = useSleep(() => r.pause());

  // Natural voices first, then the system's; the book's language first.
  const groups = useMemo(() => {
    const want = baseLang(lang || navigator.language || "en");
    const sorted = [...(r.engine?.voices ?? [])].sort(
      (a, b) =>
        Number(baseLang(a.lang) !== want) - Number(baseLang(b.lang) !== want) ||
        a.name.localeCompare(b.name),
    );
    return (["kokoro", "piper", "system"] as const)
      .map((g) => ({ g, voices: sorted.filter((v) => v.group === g) }))
      .filter((x) => x.voices.length);
  }, [r.engine, lang]);
  const chosen = r.engine ? chooseVoice(r.engine.voices, listening, lang) : null;
  const voice = r.engine?.voices.find((v) => v.id === chosen);
  useReportPlay(r.status === "playing", r.status === "starting", () =>
    r.status === "playing" ? r.pause() : r.status === "off" ? void r.start() : r.resume(),
  );

  if (r.status === "noVoices")
    return (
      <>
        <span className="max-w-80 px-2">
          {isLinux
            ? "This system has no voices to read aloud with. Libreri can use eSpeak NG instead."
            : "No voices were found. Add a voice in your system's speech settings, then try again."}
        </span>
        {isLinux && (
          <Button
            size="sm"
            onClick={() =>
              openHelper("espeak", () => {
                void getEngine(true).then(() => void r.start());
              })
            }
          >
            Install eSpeak NG…
          </Button>
        )}
        <Button variant="ghost" size="icon" aria-label="Close" onClick={onClose}>
          <X />
        </Button>
      </>
    );

  const playing = r.status === "playing";
  return (
    <>
      <PlayControls
        playing={playing}
        busy={r.status === "starting"}
        onToggle={() => (playing ? r.pause() : r.status === "off" ? void r.start() : r.resume())}
        onBack={() => r.skip(-1)}
        onForward={() => r.skip(1)}
        back="Previous sentence"
        forward="Next sentence"
      />
      <div className="flex min-w-0 flex-col px-1 leading-tight @max-3xl:hidden">
        <label className="relative flex items-center gap-1 font-medium">
          <span className="truncate">
            Voice: {voice ? voice.name : "Default"}
            {voice && (
              <span className="font-normal text-muted-foreground">
                {" "}
                ({voice.group === "system" ? voice.lang : GROUP_LABEL[voice.group].split(" · ")[1]})
              </span>
            )}
          </span>
          <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
          <select
            aria-label="Voice"
            className="absolute inset-0 cursor-pointer opacity-0"
            value={chosen ?? ""}
            onChange={(e) => {
              if (e.target.value === MORE) {
                openVoicesSettings();
                return;
              }
              const id = e.target.value || null;
              const base = baseLang(lang || navigator.language);
              const voiceFor = { ...listening.voiceFor };
              if (id) voiceFor[base] = id;
              else delete voiceFor[base];
              update({ listening: { voice: id, voiceFor } });
              setTimeout(r.restart, 0);
            }}
          >
            <option value="">Default voice</option>
            {groups.map(({ g, voices }) => (
              <optgroup key={g} label={GROUP_LABEL[g]}>
                {voices.map((v) => (
                  <option key={v.id} value={v.id}>
                    {v.name} ({v.lang})
                  </option>
                ))}
              </optgroup>
            ))}
            <option value={MORE}>Get more voices…</option>
          </select>
        </label>
        <span className="truncate text-[11.5px] text-muted-foreground">
          {r.number > 0 ? `Sentence ${r.number} · ` : ""}
          <FollowToggle />
        </span>
      </div>
      <Speed
        rate={listening.speechRate}
        onChange={(v) => {
          update({ listening: { speechRate: v } });
          setTimeout(r.restart, 0);
        }}
      />
      <SleepMenu sleep={sleep} chapters={false} />
      <Button variant="ghost" size="icon" aria-label="Stop reading aloud" onClick={onClose}>
        <X />
      </Button>
    </>
  );
}

/**
 * The linked audiobook, from the place being read. The book follows the
 * audio through the sync points, and the audiobook's own place is saved
 * as it plays (so its player carries on from here).
 */
function AudioPart({
  audio: book,
  bookId,
  progress,
  onClose,
}: {
  audio: BookDto;
  bookId: string;
  progress: number;
  onClose: () => void;
}) {
  const listening = useProfilePrefs((s) => s.prefs.listening);
  const update = useProfilePrefs((s) => s.update);
  const el = useRef<HTMLAudioElement>(null);
  const [playing, setPlaying] = useState(false);
  useReportPlay(playing, false, () => {
    const a = el.current;
    if (!a) return;
    if (a.paused) void a.play().catch(() => {});
    else a.pause();
  });
  const [time, setTime] = useState(0);
  const [duration, setDuration] = useState(0);
  const { data: link } = useQuery({
    queryKey: ["lib", "reader", book.id, "audio-link"],
    queryFn: () => unwrap(commands.getAudioLink(book.id)),
  });
  const { data: info } = useQuery({
    queryKey: ["lib", "reader", book.id, "audio-info"],
    queryFn: () => unwrap(commands.audioInfo(book.id)),
    staleTime: Infinity,
  });
  const points = useMemo(
    () => (link?.points ?? []).map((p) => ({ t: p.t ?? 0, progress: p.progress ?? 0 })),
    [link],
  );
  const chapters = useMemo(
    () => (info?.chapters ?? []).map((c) => ({ start: c.start ?? 0, title: c.title })),
    [info],
  );
  const total = duration || info?.duration || 0;
  const chapter = chapterAt(chapters, time);
  const sleepChapter = useRef<number | null>(null);
  const sleep = useSleep(() => el.current?.pause());

  // Start where the reader is, once the audio and its sync points are known.
  const startAt = useRef(progress);
  const started = useRef(false);
  const begin = useCallback(() => {
    const a = el.current;
    if (!a || started.current || !link || a.readyState < 1) return;
    const d = Number.isFinite(a.duration) ? a.duration : info?.duration || 0;
    started.current = true;
    a.currentTime = d ? timeAt(points, startAt.current, d) : 0;
    a.playbackRate = useProfilePrefs.getState().prefs.listening.audioRate;
    void a
      .play()
      .catch((e: Error) => toast.error("Could not play the audiobook", { description: e.message }));
  }, [link, points, info]);
  useEffect(() => begin(), [begin]);

  useEffect(() => {
    if (el.current) el.current.playbackRate = listening.audioRate;
  }, [listening.audioRate]);

  // The book follows the audio (every two seconds is plenty).
  const lastFollow = useRef(0);
  const onTime = () => {
    const a = el.current;
    if (!a) return;
    setTime(a.currentTime);
    if (sleep.until === "chapter") {
      if (sleepChapter.current === null) sleepChapter.current = chapter;
      else if (chapter !== sleepChapter.current) {
        a.pause();
        sleep.setUntil(null);
        sleepChapter.current = null;
        toast("Sleep timer: stopped at the end of the chapter");
      }
    }
    const now = Date.now();
    if (!total || now - lastFollow.current < 2000) return;
    lastFollow.current = now;
    if (useProfilePrefs.getState().prefs.listening.follow)
      useListening.getState().followTo(bookId, progressAt(points, a.currentTime, total));
  };

  // Keep the audiobook's own place.
  const save = useCallback(
    (a: HTMLAudioElement | null) => {
      if (!a || !started.current) return;
      const t = a.currentTime;
      void savePosition(
        book.id,
        JSON.stringify({ type: "audio", t: Math.round(t * 10) / 10 }),
        a.duration ? t / a.duration : 0,
      ).catch(() => {});
    },
    [book.id],
  );
  useEffect(() => {
    if (!playing) return;
    const id = setInterval(() => save(el.current), 5000);
    return () => clearInterval(id);
  }, [playing, save]);
  useEffect(() => {
    const a = el.current;
    return () => save(a);
  }, [save]);

  const seek = (t: number) => {
    const a = el.current;
    if (!a) return;
    a.currentTime = Math.max(0, Math.min(t, (a.duration || total) - 0.2));
    setTime(a.currentTime);
    lastFollow.current = 0;
  };
  const goChapter = (dir: 1 | -1) => {
    const t = el.current?.currentTime ?? 0;
    if (!chapters.length) return seek(t + dir * 30);
    const target =
      dir > 0
        ? chapters[chapter + 1]
        : t - (chapters[chapter]?.start ?? 0) > 3
          ? chapters[chapter]
          : chapters[chapter - 1];
    if (target) seek(target.start);
  };
  const openPlayer = () => {
    const t = el.current?.currentTime ?? 0;
    el.current?.pause();
    useListening.getState().requestPlay(book.id, t);
    useTabs.getState().openBeside({
      bookId: book.id,
      title: book.metadata.title ?? "Audiobook",
      fileType: book.fileType,
    });
    onClose();
  };

  return (
    <>
      <audio
        ref={el}
        src={bookUrl(book.relPath)}
        preload="metadata"
        onLoadedMetadata={(e) => {
          const d = e.currentTarget.duration;
          setDuration(Number.isFinite(d) ? d : 0);
          begin();
        }}
        onTimeUpdate={onTime}
        onPlay={() => setPlaying(true)}
        onPause={() => {
          setPlaying(false);
          save(el.current);
        }}
        onError={() => toast.error("The audiobook could not be played")}
      />
      <PlayControls
        playing={playing}
        busy={!link}
        onToggle={() => {
          const a = el.current;
          if (!a) return;
          if (a.paused) void a.play().catch(() => {});
          else a.pause();
        }}
        onBack={() => goChapter(-1)}
        onForward={() => goChapter(1)}
        back={chapters.length ? "Previous chapter" : "Back 30 seconds"}
        forward={chapters.length ? "Next chapter" : "Forward 30 seconds"}
      />
      <div className="flex min-w-0 max-w-64 flex-col px-1 leading-tight @max-3xl:hidden">
        <span className="truncate font-medium" title={book.metadata.title ?? ""}>
          {chapters[chapter]?.title || book.metadata.title}
        </span>
        <span className="truncate text-[11.5px] text-muted-foreground tabular-nums">
          {clock(time)}
          {total ? ` / ${clock(total)}` : ""}
          {" · "}
          {points.length ? (
            <FollowToggle />
          ) : (
            <span title="Add sync points in the audiobook player for an exact match">
              follows along roughly
            </span>
          )}
        </span>
      </div>
      <Speed rate={listening.audioRate} onChange={(v) => update({ listening: { audioRate: v } })} />
      <SleepMenu sleep={sleep} chapters={chapters.length > 0} />
      <Button
        variant="ghost"
        size="icon"
        aria-label="Open the audiobook player"
        className="@max-lg:hidden"
        title="Open the full player (chapters, bookmarks, sync points)"
        onClick={openPlayer}
      >
        <ExternalLink />
      </Button>
      <Button variant="ghost" size="icon" aria-label="Stop the audiobook" onClick={onClose}>
        <X />
      </Button>
    </>
  );
}
