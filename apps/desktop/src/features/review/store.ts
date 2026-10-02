import { toast } from "sonner";
import { create } from "zustand";
import { useEffect, useMemo } from "react";
import { useAllNotes } from "@/features/notes";
import { commands, unwrap } from "@/lib/ipc";
import { buildCards, DEFAULT_REVIEW, dueQueue, parseReview, type ReviewData } from "./model";

interface ReviewState {
  data: ReviewData;
  loaded: boolean;
  /** Reads the signed-in profile's cards (once per profile). */
  load: () => Promise<void>;
  /** Changes them and saves (one save at a time, the latest wins). */
  update: (fn: (d: ReviewData) => ReviewData) => void;
}

let saving: Promise<void> = Promise.resolve();
let loading: Promise<void> | null = null;

export const useReview = create<ReviewState>((set, get) => ({
  data: DEFAULT_REVIEW,
  loaded: false,
  load: () => {
    if (get().loaded) return Promise.resolve();
    loading ??= unwrap(commands.reviewRead())
      .then((json) => set({ data: parseReview(json), loaded: true }))
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
          await unwrap(commands.reviewWrite(JSON.stringify(next)));
        })
        .catch((e: unknown) => {
          toast.error("Review cards could not be saved", {
            description: e instanceof Error ? e.message : String(e),
          });
        });
    };
    void run();
  },
}));

/** Every card, and today's queue (worked out again once a minute at most). */
export function useCards(now: Date) {
  const notes = useAllNotes();
  const data = useReview((s) => s.data);
  const loaded = useReview((s) => s.loaded);
  const load = useReview((s) => s.load);
  useEffect(() => {
    void load();
  }, [load]);
  const minute = Math.floor(now.getTime() / 60_000);
  return useMemo(() => {
    const cards = buildCards(notes.data ?? [], data);
    const at = new Date(minute * 60_000);
    return { cards, data, loading: notes.isLoading || !loaded, ...dueQueue(cards, data, at) };
  }, [notes.data, notes.isLoading, data, loaded, minute]);
}

/** How many cards are left today (for the sidebar and the calendar). */
export function useDueCount() {
  const { queue } = useCards(new Date());
  return queue.length;
}
