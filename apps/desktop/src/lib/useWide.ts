import { useSyncExternalStore } from "react";

/** Whether the window is at least `px` wide (follows resizing). */
export function useWide(px: number): boolean {
  const query = `(min-width: ${px}px)`;
  return useSyncExternalStore(
    (change) => {
      const m = window.matchMedia(query);
      m.addEventListener("change", change);
      return () => m.removeEventListener("change", change);
    },
    () => window.matchMedia(query).matches,
    () => true,
  );
}
