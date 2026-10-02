import { toast } from "sonner";
import { create } from "zustand";
import { commands, unwrap } from "@/lib/ipc";
import { DEFAULT_STUDY, noteReached, parseStudy, type StudyData } from "./model";

interface StudyState {
  data: StudyData;
  loaded: boolean;
  /** Reads the signed-in profile's calendar (once per profile). */
  load: () => Promise<void>;
  /** Changes it and saves (one save at a time, the latest wins). */
  update: (fn: (d: StudyData) => StudyData) => void;
}

let saving: Promise<void> = Promise.resolve();
let loading: Promise<void> | null = null;

export const useStudy = create<StudyState>((set, get) => ({
  data: DEFAULT_STUDY,
  loaded: false,
  load: () => {
    if (get().loaded) return Promise.resolve();
    loading ??= unwrap(commands.studyRead())
      .then((json) => set({ data: parseStudy(json), loaded: true }))
      .catch(() => set({ loaded: true }))
      .finally(() => {
        loading = null;
      });
    return loading;
  },
  update: (fn) => {
    const run = async () => {
      if (!get().loaded) await get().load();
      const next = fn(get().data);
      if (next === get().data) return;
      set({ data: next });
      saving = saving
        .then(async () => {
          await unwrap(commands.studyWrite(JSON.stringify(next)));
        })
        .catch((e: unknown) => {
          toast.error("The reading calendar could not be saved", {
            description: e instanceof Error ? e.message : String(e),
          });
        });
    };
    void run();
  },
}));

/** The book open in the reader now, for the timer and goals. */
export interface Reading {
  bookId: string;
  title: string;
  /** Page (or, for reflowing books, a page-like position) and how many. */
  page: number | null;
  pages: number | null;
  /** 0–1 through the book. */
  progress: number;
}

export const useReading = create<{
  now: Reading | null;
  set: (r: Reading | null) => void;
}>((set) => ({
  now: null,
  set: (r) => set({ now: r }),
}));

/** The reader reports where it is: open goals for the book move on. */
export function reportReading(r: Reading | null) {
  const cur = useReading.getState().now;
  if (
    cur?.bookId === r?.bookId &&
    cur?.page === r?.page &&
    cur?.pages === r?.pages &&
    Math.abs((cur?.progress ?? 0) - (r?.progress ?? 0)) < 0.001 &&
    cur?.title === r?.title
  )
    return;
  useReading.getState().set(r);
  if (!r) return;
  const s = useStudy.getState();
  if (!s.loaded || !s.data.goals.some((g) => g.bookId === r.bookId && !g.done)) return;
  const goals = noteReached(s.data.goals, r.bookId, r.page, r.progress);
  if (goals) s.update((d) => ({ ...d, goals }));
}
