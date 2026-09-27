import { useEffect } from "react";

const ACTIVITY = ["pointerdown", "pointermove", "keydown", "wheel", "touchstart"] as const;

/**
 * Calls `lock` after `minutes` without keyboard, mouse or touch activity.
 * 0 minutes = never. Activity inside book pages (iframes) is forwarded by
 * the readers as key events, and pointer moves over them still reach the
 * window through the reader's overlay.
 */
export function useAutoLock(minutes: number, lock: () => void) {
  useEffect(() => {
    if (!minutes) return;
    let last = Date.now();
    const bump = () => {
      last = Date.now();
    };
    for (const e of ACTIVITY) window.addEventListener(e, bump, { passive: true, capture: true });
    const t = setInterval(() => {
      if (Date.now() - last >= minutes * 60_000) lock();
    }, 5_000);
    return () => {
      clearInterval(t);
      for (const e of ACTIVITY) window.removeEventListener(e, bump, { capture: true });
    };
  }, [minutes, lock]);
}
