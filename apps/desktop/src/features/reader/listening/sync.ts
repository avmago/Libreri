/**
 * Keeping an audiobook and its text in step: sync points match moments of
 * the audio to places in the text; between them, both move in a straight
 * line. Places are 0–1 through the text book.
 */
export interface Point {
  t: number;
  progress: number;
}

/** Where the text is at `t` seconds. */
export function progressAt(points: Point[], t: number, duration: number): number {
  let prev = { t: 0, progress: 0 };
  for (const p of points) {
    if (p.t >= t) {
      const span = p.t - prev.t;
      const k = span > 0 ? (t - prev.t) / span : 1;
      return prev.progress + (p.progress - prev.progress) * clamp(k);
    }
    prev = p;
  }
  const span = duration - prev.t;
  const k = span > 0 ? (t - prev.t) / span : 1;
  return clamp(prev.progress + (1 - prev.progress) * clamp(k));
}

/** When the audio reaches `progress` through the text. */
export function timeAt(points: Point[], progress: number, duration: number): number {
  let prev = { t: 0, progress: 0 };
  for (const p of points) {
    if (p.progress >= progress) {
      const span = p.progress - prev.progress;
      const k = span > 0 ? (progress - prev.progress) / span : 1;
      return prev.t + (p.t - prev.t) * clamp(k);
    }
    prev = p;
  }
  const span = 1 - prev.progress;
  const k = span > 0 ? (progress - prev.progress) / span : 1;
  return Math.min(duration, prev.t + (duration - prev.t) * clamp(k));
}

function clamp(v: number) {
  return Math.min(1, Math.max(0, v));
}

/** "1:02:03" or "2:03". */
export function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

/** The chapter playing at `t` (index), or -1. */
export function chapterAt(chapters: { start: number }[], t: number): number {
  let at = -1;
  chapters.forEach((c, i) => {
    if (c.start <= t + 0.25) at = i;
  });
  return at;
}
