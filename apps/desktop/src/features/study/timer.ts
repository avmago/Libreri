/**
 * The study timer: focus sessions (work, short breaks, a long break every
 * few rounds), a plain countdown and a stopwatch. One for the window; it
 * keeps going while you move between books and the library.
 */
import { toast } from "sonner";
import { create } from "zustand";
import { newId, type StudySession } from "./model";
import { useReading, useStudy } from "./store";

export type TimerMode = "focus" | "timer" | "stopwatch";
export type Phase = "work" | "break" | "long";

/** The book being read when a stretch began. */
interface Context {
  bookId: string;
  title: string;
  page: number | null;
}

/** Shown when a focus stretch ends: what was read, and a box for a note. */
export interface BreakCard {
  minutes: number;
  bookId: string | null;
  title: string | null;
  pageFrom: number | null;
  pageTo: number | null;
  next: Phase;
}

interface TimerState {
  mode: TimerMode;
  status: "idle" | "running" | "paused";
  phase: Phase;
  /** Focus round, from 1. */
  round: number;
  /** Running: when the stretch ends (countdowns). */
  endsAt: number | null;
  /** Paused: what was left (countdowns) or counted (stopwatch). */
  heldMs: number;
  /** Stopwatch running: counted before `since`, and since when. */
  since: number | null;
  /** The length of the stretch, for progress. */
  lengthMs: number;
  /** When the stretch began (for the session's start). */
  began: number | null;
  context: Context | null;
  card: BreakCard | null;
  /** The popover in the reader's toolbar. */
  open: boolean;
  setOpen: (o: boolean) => void;
  setMode: (m: TimerMode) => void;
  start: () => void;
  pause: () => void;
  resume: () => void;
  /** Focus: go on to the next part now. */
  skip: () => void;
  stop: () => void;
  /** Called every second. */
  tick: () => void;
  closeCard: () => void;
  /** Start the break the card offers. */
  startBreak: () => void;
}

const MIN = 60_000;

/** The book being read now. */
function context(): Context | null {
  const r = useReading.getState().now;
  return r ? { bookId: r.bookId, title: r.title, page: r.page } : null;
}

/** Saves a stretch of reading to the calendar. */
function record(kind: StudySession["kind"], began: number, ms: number, ctx: Context | null) {
  const minutes = Math.round(ms / MIN);
  if (minutes < 1) return null;
  const { data, update } = useStudy.getState();
  const count = data.options.countReading;
  const now = useReading.getState().now;
  const same = count && ctx && now?.bookId === ctx.bookId;
  const s: StudySession = {
    id: newId(),
    start: new Date(began).toISOString(),
    minutes,
    kind,
    bookId: count ? (ctx?.bookId ?? null) : null,
    title: count ? (ctx?.title ?? null) : null,
    pageFrom: same ? ctx.page : null,
    pageTo: same ? now.page : null,
  };
  update((d) => ({ ...d, sessions: [...d.sessions, s] }));
  return s;
}

/** A soft two-note chime. */
export function chime() {
  try {
    const Ctx =
      window.AudioContext ??
      (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
    if (!Ctx) return;
    const ctx = new Ctx();
    [660, 880].forEach((f, i) => {
      const o = ctx.createOscillator();
      const g = ctx.createGain();
      o.frequency.value = f;
      o.type = "sine";
      const t = ctx.currentTime + i * 0.28;
      g.gain.setValueAtTime(0, t);
      g.gain.linearRampToValueAtTime(0.18, t + 0.03);
      g.gain.exponentialRampToValueAtTime(0.001, t + 0.9);
      o.connect(g).connect(ctx.destination);
      o.start(t);
      o.stop(t + 1);
    });
    setTimeout(() => void ctx.close(), 2000);
  } catch {
    /* no sound */
  }
}

/** Things that listen for breaks (the reader pauses read aloud). */
const breakListeners = new Set<() => void>();
export function onBreak(fn: () => void): () => void {
  breakListeners.add(fn);
  return () => breakListeners.delete(fn);
}

const idle = {
  status: "idle" as const,
  phase: "work" as Phase,
  round: 1,
  endsAt: null,
  heldMs: 0,
  since: null,
  lengthMs: 0,
  began: null,
  context: null,
};

export const useTimer = create<TimerState>((set, get) => {
  const lengths = () => useStudy.getState().data.focus;
  const minutesOf = (p: Phase) =>
    p === "work" ? lengths().work : p === "break" ? lengths().brk : lengths().long;

  /** Starts a countdown stretch now. */
  const countdown = (phase: Phase, ms: number, round = get().round) => {
    const now = Date.now();
    set({
      status: "running",
      phase,
      round,
      endsAt: now + ms,
      lengthMs: ms,
      heldMs: 0,
      began: now,
      since: null,
      context: phase === "work" ? context() : get().context,
    });
  };

  /** A focus stretch is over (`early`: skipped or stopped). */
  const endWork = (early: boolean) => {
    const s = get();
    const spent = s.lengthMs - (s.endsAt ? Math.max(0, s.endsAt - Date.now()) : s.heldMs);
    const saved = record("focus", s.began ?? Date.now(), spent, s.context);
    const rounds = lengths().rounds;
    const next: Phase = s.round % rounds === 0 ? "long" : "break";
    if (!early) {
      chime();
      if (useStudy.getState().data.options.pauseAudio) {
        // Podcasts and audiobooks play through audio elements.
        document.querySelectorAll("audio").forEach((a) => a.pause());
        breakListeners.forEach((f) => f());
      }
    }
    return { saved, next };
  };

  return {
    mode: "focus",
    ...idle,
    card: null,
    open: false,
    setOpen: (open) => set({ open }),
    setMode: (mode) => {
      if (get().status !== "idle") get().stop();
      set({ mode });
    },
    start: () => {
      const { mode } = get();
      if (mode === "focus") countdown("work", minutesOf("work") * MIN, 1);
      else if (mode === "timer")
        countdown("work", useStudy.getState().data.options.timerMinutes * MIN, 1);
      else
        set({
          ...idle,
          status: "running",
          since: Date.now(),
          began: Date.now(),
          context: context(),
        });
    },
    pause: () => {
      const s = get();
      if (s.status !== "running") return;
      if (s.mode === "stopwatch")
        set({
          status: "paused",
          heldMs: s.heldMs + (Date.now() - (s.since ?? Date.now())),
          since: null,
        });
      else
        set({ status: "paused", heldMs: Math.max(0, (s.endsAt ?? 0) - Date.now()), endsAt: null });
    },
    resume: () => {
      const s = get();
      if (s.status !== "paused") return;
      if (s.mode === "stopwatch") set({ status: "running", since: Date.now() });
      else {
        const now = Date.now();
        const fresh = s.phase === "work" && s.began === null;
        set({
          status: "running",
          endsAt: now + s.heldMs,
          heldMs: 0,
          began: s.began ?? now,
          context: fresh ? context() : s.context,
        });
      }
    },
    skip: () => {
      const s = get();
      if (s.mode !== "focus" || s.status === "idle") return;
      if (s.phase === "work") {
        const { next } = endWork(true);
        countdown(next, minutesOf(next) * MIN);
      } else countdown("work", minutesOf("work") * MIN, s.round + 1);
    },
    stop: () => {
      const s = get();
      if (s.status === "idle") return;
      if (s.mode === "focus" && s.phase === "work") endWork(true);
      if (s.mode === "stopwatch") {
        const ms = s.heldMs + (s.since ? Date.now() - s.since : 0);
        const saved = record("stopwatch", s.began ?? Date.now(), ms, s.context);
        if (saved) toast(`Saved ${saved.minutes} min of reading`);
      }
      set({ ...idle });
    },
    tick: () => {
      const s = get();
      if (s.status !== "running" || s.mode === "stopwatch" || !s.endsAt) return;
      if (Date.now() < s.endsAt) return;
      if (s.mode === "timer") {
        chime();
        toast("Time's up", {
          description: `${useStudy.getState().data.options.timerMinutes} minutes`,
        });
        set({ ...idle });
        return;
      }
      if (s.phase === "work") {
        const { saved, next } = endWork(false);
        const card: BreakCard = {
          minutes: saved?.minutes ?? Math.round(s.lengthMs / MIN),
          bookId: s.context?.bookId ?? null,
          title: s.context?.title ?? null,
          pageFrom: saved?.pageFrom ?? null,
          pageTo: saved?.pageTo ?? null,
          next,
        };
        // Waits on the card: the break starts when asked.
        set({
          status: "paused",
          phase: next,
          heldMs: minutesOf(next) * MIN,
          lengthMs: minutesOf(next) * MIN,
          endsAt: null,
          card,
        });
      } else {
        chime();
        toast("Break over", { description: "Back to reading when you are ready." });
        const round = s.round + 1;
        set({
          status: "paused",
          phase: "work",
          round,
          heldMs: minutesOf("work") * MIN,
          lengthMs: minutesOf("work") * MIN,
          endsAt: null,
          began: null,
          context: context(),
        });
      }
    },
    closeCard: () => set({ card: null }),
    startBreak: () => {
      const s = get();
      set({ card: null });
      if (s.status === "paused" && s.phase !== "work") s.resume();
    },
  };
});

/** Time left (countdowns) or counted (stopwatch), and how far through. */
export function timerReading(s: TimerState, now = Date.now()) {
  if (s.mode === "stopwatch") {
    const ms = s.heldMs + (s.since ? now - s.since : 0);
    return { ms, progress: null as number | null };
  }
  const left = s.status === "running" && s.endsAt ? Math.max(0, s.endsAt - now) : s.heldMs;
  return { ms: left, progress: s.lengthMs ? 1 - left / s.lengthMs : 0 };
}
