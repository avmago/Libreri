import { toast } from "sonner";
import { create } from "zustand";
import { commands, unwrap, type FeedDto, type FeedItem } from "@/lib/ipc";
import { episodeSrc } from "./model";

/** What is playing. */
export interface Episode {
  id: string;
  feed: string;
  title: string;
  show: string;
  artwork: string | null;
  src: string;
  link: string | null;
}

const RATE_KEY = "libreri.podcastRate";
function savedRate(): number {
  try {
    const v = Number(localStorage.getItem(RATE_KEY));
    return v >= 0.5 && v <= 3 ? v : 1;
  } catch {
    return 1;
  }
}

interface PlayerState {
  episode: Episode | null;
  playing: boolean;
  loading: boolean;
  time: number;
  duration: number;
  rate: number;
  /** Seconds to jump to once the audio is ready. */
  startAt: number;
  /** The floating bar in a book shows the podcast (the strip then hides). */
  inBar: boolean;
  /** Sleep: stop at this time (ms), or at the end of the episode. */
  sleep: number | "end" | null;
  el: HTMLAudioElement | null;
  attach: (el: HTMLAudioElement | null) => void;
  play: (item: FeedItem, feed?: FeedDto | null) => void;
  toggle: () => void;
  seek: (t: number) => void;
  skip: (seconds: number) => void;
  setRate: (rate: number) => void;
  setSleep: (sleep: number | "end" | null) => void;
  setInBar: (inBar: boolean) => void;
  stop: () => void;
  /** Saves where listening is (on pause, every few seconds, on leaving). */
  save: (played?: boolean) => void;
  /** The episode ended: mark it played and go on with Up next. */
  ended: () => void;
  /** From the audio element's events. */
  sync: (a: Partial<Pick<PlayerState, "playing" | "loading" | "time" | "duration">>) => void;
}

const fail = (what: string) => (e: unknown) =>
  toast.error(what, { description: e instanceof Error ? e.message : String(e) });

export const usePlayer = create<PlayerState>((set, get) => ({
  episode: null,
  playing: false,
  loading: false,
  time: 0,
  duration: 0,
  rate: savedRate(),
  startAt: 0,
  inBar: false,
  sleep: null,
  el: null,
  attach: (el) => set({ el }),

  play: (item, feed) => {
    const { episode, el, toggle, save } = get();
    if (episode?.id === item.id) return toggle();
    const src = episodeSrc(item);
    if (!src || !el) {
      toast.error("This episode has no audio to play");
      return;
    }
    if (episode) save();
    const rate = feed?.speed || savedRate();
    set({
      episode: {
        id: item.id,
        feed: item.feed,
        title: item.title,
        show: feed?.title ?? item.source,
        artwork: feed?.artwork ?? null,
        src,
        link: item.link,
      },
      startAt: item.played ? 0 : (item.position ?? 0),
      time: item.played ? 0 : (item.position ?? 0),
      duration: item.duration ?? 0,
      loading: true,
      rate,
    });
    el.src = src;
    el.playbackRate = rate;
    void el.play().catch((e: Error) => {
      if (e.name !== "AbortError") fail("Could not play the episode")(e);
      set({ loading: false });
    });
  },

  toggle: () => {
    const { el, episode } = get();
    if (!el || !episode) return;
    if (el.paused) void el.play().catch(fail("Could not play the episode"));
    else el.pause();
  },

  seek: (t) => {
    const { el, duration } = get();
    if (!el) return;
    const end = (Number.isFinite(el.duration) ? el.duration : duration) || Infinity;
    el.currentTime = Math.max(0, Math.min(t, end - 0.5));
    set({ time: el.currentTime });
  },

  skip: (s) => get().seek((get().el?.currentTime ?? 0) + s),

  setRate: (rate) => {
    const { el, episode } = get();
    if (el) el.playbackRate = rate;
    set({ rate });
    try {
      localStorage.setItem(RATE_KEY, String(rate));
    } catch {
      /* not kept */
    }
    // Each show keeps its own speed.
    if (episode)
      void commands
        .feedChange("podcasts", episode.feed, {
          title: null,
          folder: null,
          autoDownload: null,
          speed: rate,
        })
        .catch(() => {});
  },

  setSleep: (sleep) => set({ sleep }),
  setInBar: (inBar) => set({ inBar }),

  stop: () => {
    const { el, save } = get();
    save();
    if (el) {
      el.pause();
      el.removeAttribute("src");
      el.load();
    }
    set({ episode: null, playing: false, loading: false, time: 0, duration: 0, sleep: null });
  },

  save: (played = false) => {
    const { episode, el, duration } = get();
    if (!episode || !el) return;
    const t = el.currentTime;
    const d = Number.isFinite(el.duration) && el.duration > 0 ? el.duration : duration || null;
    if (!played && t < 1) return;
    void commands.podcastProgress(episode.id, Math.round(t), d, played).catch(() => {});
  },

  ended: () => {
    const { episode, save, sleep } = get();
    if (!episode) return;
    save(true);
    if (sleep === "end") {
      set({ sleep: null, playing: false });
      toast("Sleep timer: stopped at the end of the episode");
      return;
    }
    // Go on with Up next (this episode leaves it).
    void unwrap(commands.feedsOverview("podcasts"))
      .then(async (o) => {
        const next = o.queue.find((id) => id !== episode.id);
        if (!next) return set({ playing: false });
        const [item] = await unwrap(commands.podcastEpisodes([next]));
        if (!item) return;
        get().play(
          item,
          o.feeds.find((f) => f.id === item.feed),
        );
      })
      .catch(() => set({ playing: false }));
  },

  sync: (a) => set(a),
}));
