import { Timer } from "lucide-react";
import { byDay, dayKey, duration } from "./model";
import { useStudy } from "./store";

/** "⏱ Today 50 min" in the reader's status bar. */
export function TodayReading({ onOpen }: { onOpen?: () => void }) {
  const sessions = useStudy((s) => s.data.sessions);
  const minutes = byDay(sessions).get(dayKey(new Date()))?.minutes ?? 0;
  if (!minutes) return null;
  return (
    <button
      type="button"
      className="flex items-center gap-1 text-emerald-700 hover:text-foreground dark:text-emerald-300"
      title="Reading time today (the calendar)"
      onClick={onOpen}
    >
      <Timer className="size-3.5" aria-hidden />
      Today {duration(minutes)}
    </button>
  );
}
