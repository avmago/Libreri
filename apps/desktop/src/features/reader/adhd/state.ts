import { create } from "zustand";
import { useProfilePrefs } from "@/features/profiles";

/**
 * ADHD reading paused from the reader's Aa menu or the status bar. Only
 * for now: Settings › Reader is unchanged, and it is back on the next
 * time Libreri starts.
 */
export const useAdhdPause = create<{ paused: boolean; setPaused: (paused: boolean) => void }>(
  (set) => ({ paused: false, setPaused: (paused) => set({ paused }) }),
);

/** The ADHD reading tools turned on in Settings, and whether they are in
 * use now (not paused). */
export function useAdhd() {
  const adhd = useProfilePrefs((s) => s.prefs.adhd);
  const paused = useAdhdPause((s) => s.paused);
  const enabled = adhd.bionic || adhd.line || adhd.mask;
  return { ...adhd, enabled, live: enabled && !paused, paused };
}

/** "Bionic reading · Line highlight · Reading mask" for what is turned on. */
export function adhdTools(a: { bionic: boolean; line: boolean; mask: boolean }): string {
  return [a.bionic && "Bionic reading", a.line && "Line highlight", a.mask && "Reading mask"]
    .filter(Boolean)
    .join(" · ");
}
