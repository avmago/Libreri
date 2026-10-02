import { useEffect, useState } from "react";
import { clock } from "./model";
import { timerReading, type useTimer } from "./timer";

/** Re-renders every second while the timer runs. */
export function useNow(active: boolean) {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    if (!active) return;
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [active]);
  return now;
}

export const PHASE = { work: "Focus", break: "Break", long: "Long break" } as const;

/** What the clock says: "Focus 15:32", "Timer 4:10", "1:02:03". */
export function timerLabel(s: ReturnType<typeof useTimer.getState>, now: number) {
  const { ms } = timerReading(s, now);
  const name = s.mode === "focus" ? PHASE[s.phase] : s.mode === "timer" ? "Timer" : "Stopwatch";
  return { name, time: clock(ms) };
}
