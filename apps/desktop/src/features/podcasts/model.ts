import { bookUrl, type FeedItem } from "@/lib/ipc";

/** Where an episode plays from: the downloaded file, or streamed. */
export function episodeSrc(item: Pick<FeedItem, "file" | "audio">): string | null {
  if (item.file) return bookUrl(item.file);
  return item.audio ?? null;
}

/** "58 min", "1 h 4 min" (empty when not known). */
export function minutes(seconds: number | null | undefined): string {
  if (!seconds || !Number.isFinite(seconds) || seconds <= 0) return "";
  const m = Math.max(1, Math.round(seconds / 60));
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  return m % 60 ? `${h} h ${m % 60} min` : `${h} h`;
}

/** How far into an episode listening got (0–1), or null when not started. */
export function heard(item: Pick<FeedItem, "position" | "duration" | "played">): number | null {
  if (item.played) return 1;
  const p = item.position ?? 0;
  const d = item.duration ?? 0;
  if (p <= 0) return null;
  return d > 0 ? Math.min(1, p / d) : null;
}

/** "41 min left of 63", "58 min", "Played". */
export function lengthLine(item: Pick<FeedItem, "position" | "duration" | "played">): string {
  if (item.played) return "Played";
  const d = item.duration ?? 0;
  const p = item.position ?? 0;
  if (p > 0 && d > p) {
    const left = minutes(d - p);
    const total = Math.round(d / 60);
    return `${left} left of ${total}`;
  }
  return minutes(d);
}

/** "1:02:03", "4:05". */
export function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

/** The offered playback speeds. */
export const RATES = [0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3];

/** Moves `id` up (-1) or down (1) in the queue. */
export function moveInQueue(queue: string[], id: string, dir: 1 | -1): string[] {
  const i = queue.indexOf(id);
  const j = i + dir;
  if (i < 0 || j < 0 || j >= queue.length) return queue;
  const next = [...queue];
  [next[i], next[j]] = [next[j]!, next[i]!];
  return next;
}

/** Initials for a show without artwork. */
export function initials(title: string): string {
  const words = title
    .replace(/[^\p{L}\p{N}\s]/gu, " ")
    .split(/\s+/)
    .filter((w) => w && !/^(the|a|an)$/i.test(w));
  return (
    words
      .slice(0, 2)
      .map((w) => w[0]!.toUpperCase())
      .join("") || "?"
  );
}
