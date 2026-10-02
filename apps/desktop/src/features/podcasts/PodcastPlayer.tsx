import { useEffect, useState } from "react";
import { toast } from "sonner";
import {
  ChevronsDown,
  ListOrdered,
  Loader2,
  Minus,
  Pause,
  Play,
  Plus,
  RotateCcw,
  RotateCw,
  Timer,
  X,
} from "lucide-react";
import { MiniPlayer } from "@/components/MiniPlayer";
import { Button } from "@/components/ui/button";
import { useFloatingBar } from "@/lib/floating";
import { DropdownMenu, menuContent, menuItem } from "@/components/ui/menu";
import { cn } from "@/lib/utils";
import { clock, initials, RATES } from "./model";
import { showPodcasts } from "./navigate";
import { usePlayer } from "./player";

const SLEEP = [5, 10, 15, 30, 45, 60];

/**
 * The one audio element podcasts play through (it stays while you move
 * between the library and books), and the strip along the bottom of the
 * window while an episode is loaded.
 */
export function PodcastPlayer({ hidden }: { hidden?: boolean }) {
  const playing = usePlayer((s) => s.playing);
  const sleep = usePlayer((s) => s.sleep);
  const episode = usePlayer((s) => s.episode);
  const loading = usePlayer((s) => s.loading);
  const collapsed = useFloatingBar((s) => s.collapsed);
  const setCollapsed = useFloatingBar((s) => s.setCollapsed);
  const attach = usePlayer((s) => s.attach);
  const sync = usePlayer((s) => s.sync);

  // Keep the place every 15 seconds, and when Libreri closes.
  useEffect(() => {
    if (!playing) return;
    const id = setInterval(() => usePlayer.getState().save(), 15_000);
    return () => clearInterval(id);
  }, [playing]);
  useEffect(() => {
    const leave = () => usePlayer.getState().save();
    window.addEventListener("beforeunload", leave);
    return () => window.removeEventListener("beforeunload", leave);
  }, []);

  // Sleep timer.
  useEffect(() => {
    if (typeof sleep !== "number") return;
    const id = setInterval(() => {
      const s = usePlayer.getState();
      if (typeof s.sleep === "number" && Date.now() >= s.sleep) {
        s.el?.pause();
        s.setSleep(null);
        toast("Sleep timer: stopped");
      }
    }, 1000);
    return () => clearInterval(id);
  }, [sleep]);

  // The keyboard's and the system's media keys.
  useEffect(() => {
    if (!("mediaSession" in navigator)) return;
    const ms = navigator.mediaSession;
    if (!episode) {
      ms.metadata = null;
      return;
    }
    try {
      ms.metadata = new MediaMetadata({
        title: episode.title,
        artist: episode.show,
        artwork: episode.artwork ? [{ src: episode.artwork }] : [],
      });
      const s = () => usePlayer.getState();
      ms.setActionHandler("play", () => s().toggle());
      ms.setActionHandler("pause", () => s().toggle());
      ms.setActionHandler("seekbackward", () => s().skip(-15));
      ms.setActionHandler("seekforward", () => s().skip(30));
    } catch {
      /* not offered here */
    }
  }, [episode]);

  return (
    <>
      <audio
        ref={attach}
        preload="metadata"
        onLoadedMetadata={(e) => {
          const a = e.currentTarget;
          const s = usePlayer.getState();
          if (s.startAt > 0 && Number.isFinite(a.duration) && s.startAt < a.duration - 5)
            a.currentTime = s.startAt;
          a.playbackRate = s.rate;
          s.sync({ duration: Number.isFinite(a.duration) ? a.duration : s.duration });
        }}
        onTimeUpdate={(e) => sync({ time: e.currentTarget.currentTime })}
        onPlay={() => sync({ playing: true })}
        onPlaying={() => sync({ loading: false, playing: true })}
        onWaiting={() => sync({ loading: true })}
        onPause={() => {
          sync({ playing: false, loading: false });
          usePlayer.getState().save();
        }}
        onEnded={() => usePlayer.getState().ended()}
        onError={() => {
          if (!usePlayer.getState().episode) return;
          sync({ playing: false, loading: false });
          toast.error("The episode could not be played", {
            description: "Check the connection, or download it first.",
          });
        }}
      />
      {episode && !hidden && collapsed && (
        <MiniPlayer
          fixed
          label={`${episode.title} — ${episode.show}`}
          playing={playing}
          busy={loading && playing}
          onToggle={() => usePlayer.getState().toggle()}
          picture={
            <Artwork
              src={episode.artwork}
              title={episode.show}
              className="size-full rounded-none"
            />
          }
        />
      )}
      {episode && !hidden && !collapsed && (
        // Floats over the page like the read-aloud and audiobook player.
        <div
          role="region"
          aria-label="Podcast player"
          className="fixed bottom-5 left-1/2 z-30 flex w-max max-w-[calc(100%-2rem)] -translate-x-1/2 items-center gap-1.5 rounded-2xl border bg-popover/95 p-1.5 text-[12.5px] text-popover-foreground shadow-xl backdrop-blur"
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
          <Artwork src={episode.artwork} title={episode.show} className="size-9 rounded-[10px]" />
          <button
            type="button"
            className="flex min-w-0 max-w-64 flex-col px-1 text-left leading-tight"
            title={`${episode.title} — ${episode.show}`}
            onClick={() => showPodcasts({ kind: "show", id: episode.feed })}
          >
            <span className="truncate font-medium">{episode.title}</span>
            <span className="truncate text-[11.5px] text-muted-foreground">{episode.show}</span>
          </button>
          <PodcastControls compact />
        </div>
      )}
    </>
  );
}

/** Back, play, forward, the time, speed, sleep and close. */
export function PodcastControls({ compact }: { compact: boolean }) {
  const p = usePlayer();
  const total = p.duration;
  return (
    <>
      <Button variant="ghost" size="icon" aria-label="Back 15 seconds" onClick={() => p.skip(-15)}>
        <RotateCcw />
      </Button>
      <Button
        size="icon"
        className="size-9 rounded-full"
        aria-label={p.playing ? "Pause" : "Play"}
        onClick={p.toggle}
      >
        {p.loading && p.playing ? (
          <Loader2 className="animate-spin" />
        ) : p.playing ? (
          <Pause />
        ) : (
          <Play />
        )}
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label="Forward 30 seconds"
        onClick={() => p.skip(30)}
      >
        <RotateCw />
      </Button>
      {compact ? (
        <div className="flex shrink-0 flex-col gap-0.5 px-1">
          <input
            type="range"
            aria-label="Position"
            min={0}
            max={Math.max(1, Math.round(total))}
            step={1}
            value={Math.min(Math.round(p.time), Math.round(total) || 1)}
            onChange={(e) => p.seek(Number(e.target.value))}
            className="h-1 w-32 accent-foreground"
          />
          <span className="text-[11px] text-muted-foreground tabular-nums">
            {clock(p.time)}
            {total ? ` / ${clock(total)}` : ""}
          </span>
        </div>
      ) : (
        <div className="flex min-w-32 flex-1 items-center gap-2">
          <span className="w-12 text-right text-[11.5px] text-muted-foreground tabular-nums">
            {clock(p.time)}
          </span>
          <input
            type="range"
            aria-label="Position"
            min={0}
            max={Math.max(1, Math.round(total))}
            step={1}
            value={Math.min(Math.round(p.time), Math.round(total) || 1)}
            onChange={(e) => p.seek(Number(e.target.value))}
            className="h-1 min-w-0 flex-1 accent-foreground"
          />
          <span className="w-12 text-[11.5px] text-muted-foreground tabular-nums">
            {total ? clock(total) : "--:--"}
          </span>
        </div>
      )}
      <RateControl />
      <SleepMenu />
      <Button
        variant="ghost"
        size="icon"
        aria-label="Up next"
        title="Up next"
        onClick={() => showPodcasts({ kind: "list", id: "queue" })}
      >
        <ListOrdered />
      </Button>
      <Button variant="ghost" size="icon" aria-label="Stop the podcast" onClick={p.stop}>
        <X />
      </Button>
    </>
  );
}

function RateControl() {
  const rate = usePlayer((s) => s.rate);
  const setRate = usePlayer((s) => s.setRate);
  const i = RATES.findIndex((r) => r >= rate - 0.001);
  const at = i < 0 ? RATES.length - 1 : i;
  return (
    <div
      className="flex shrink-0 items-center rounded-lg border"
      title="Speed (kept for this show)"
    >
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        aria-label="Slower"
        disabled={at <= 0}
        onClick={() => setRate(RATES[Math.max(0, at - 1)]!)}
      >
        <Minus />
      </Button>
      <span className="w-10 text-center tabular-nums">{rate}×</span>
      <Button
        variant="ghost"
        size="icon"
        className="size-7"
        aria-label="Faster"
        disabled={at >= RATES.length - 1}
        onClick={() => setRate(RATES[Math.min(RATES.length - 1, at + 1)]!)}
      >
        <Plus />
      </Button>
    </div>
  );
}

function SleepMenu() {
  const sleep = usePlayer((s) => s.sleep);
  const setSleep = usePlayer((s) => s.setSleep);
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (typeof sleep !== "number") return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [sleep]);
  const left = typeof sleep === "number" ? Math.max(0, (sleep - now) / 1000) : null;
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button
          variant={sleep ? "outline" : "ghost"}
          size={sleep ? "sm" : "icon"}
          aria-label="Sleep timer"
          title="Sleep timer"
        >
          <Timer />
          {left !== null ? clock(left) : sleep === "end" ? "Episode" : null}
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className={menuContent} sideOffset={6} side="top">
          {SLEEP.map((m) => (
            <DropdownMenu.Item
              key={m}
              className={menuItem}
              onSelect={() => setSleep(Date.now() + m * 60_000)}
            >
              Stop in {m} minutes
            </DropdownMenu.Item>
          ))}
          <DropdownMenu.Item className={menuItem} onSelect={() => setSleep("end")}>
            Stop at the end of this episode
          </DropdownMenu.Item>
          {sleep && (
            <DropdownMenu.Item className={menuItem} onSelect={() => setSleep(null)}>
              Turn off
            </DropdownMenu.Item>
          )}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}

/** A show's artwork, or its initials. */
export function Artwork({
  src,
  title,
  className,
}: {
  src: string | null | undefined;
  title: string;
  className?: string;
}) {
  const [broken, setBroken] = useState(false);
  if (src && !broken)
    return (
      <img
        src={src}
        alt=""
        onError={() => setBroken(true)}
        className={cn("shrink-0 rounded-md bg-muted object-cover", className)}
      />
    );
  return (
    <div
      aria-hidden
      className={cn(
        "flex shrink-0 items-center justify-center rounded-md bg-muted font-semibold text-muted-foreground",
        className,
      )}
    >
      {initials(title)}
    </div>
  );
}
