/**
 * Links from a place in a book: what a "link" annotation's
 * locator holds, and times written as "1:30".
 */
import type { KnownVideo } from "@/lib/ipc";

export interface LinkInfo {
  /** The web address, or a `libreri://book/…` link; empty for files. */
  url: string;
  kind: "video" | "web" | "file" | "book";
  title: string;
  site?: string | null;
  author?: string | null;
  description?: string | null;
  /** Length in seconds. */
  duration?: number | null;
  /** Where to start playing, in seconds. */
  start?: number | null;
  /** YouTube or Vimeo, embedded with a start time. */
  video?: KnownVideo | null;
  /** Another site's player (https). */
  embed?: string | null;
  /** The picture, in the notes folder. */
  picture?: string | null;
  /** An offline copy of the page, in the notes folder. */
  copy?: string | null;
  /** A video or audio file: in the library (relative) or a full path. */
  file?: string | null;
  media?: "video" | "audio" | null;
}

/** The link in an annotation's locator. */
export function linkOf(locator: string): LinkInfo | null {
  try {
    const v = JSON.parse(locator) as { link?: LinkInfo };
    return v.link && typeof v.link === "object" ? v.link : null;
  } catch {
    return null;
  }
}

/** "1:05:09" or "4:07". */
export function formatTime(secs: number): string {
  const s = Math.max(0, Math.round(secs));
  const h = Math.floor(s / 3600);
  const m = Math.floor(s / 60) % 60;
  const r = s % 60;
  return h > 0
    ? `${h}:${String(m).padStart(2, "0")}:${String(r).padStart(2, "0")}`
    : `${m}:${String(r).padStart(2, "0")}`;
}

/** Seconds from "90", "1:30", "1:02:03" or "1m30s"; null if empty; NaN
 * if it cannot be read. */
export function parseTime(text: string): number | null {
  const t = text.trim().replace(/s$/, "");
  if (!t) return null;
  if (/^\d+(\.\d+)?$/.test(t)) return Number(t);
  if (/^\d+(:\d{1,2}){1,2}$/.test(t)) return t.split(":").reduce((a, p) => a * 60 + Number(p), 0);
  const m = /^(?:(\d+)h)?(?:(\d+)m)?(?:(\d+))?$/.exec(t);
  if (m && (m[1] || m[2] || m[3]))
    return Number(m[1] ?? 0) * 3600 + Number(m[2] ?? 0) * 60 + Number(m[3] ?? 0);
  return NaN;
}

/** Plays in the side panel. */
export const playable = (l: LinkInfo) =>
  (l.kind === "video" && Boolean(l.video || l.embed)) || l.kind === "file";

/** Where the link goes, for notes exported as Markdown. */
export function linkHref(l: LinkInfo): string {
  if (l.kind === "file") return l.file ?? "";
  if (l.video && l.start) {
    const s = Math.round(l.start);
    return l.video.provider === "youtube"
      ? `https://www.youtube.com/watch?v=${l.video.id}&t=${s}s`
      : `${l.url.split("#")[0]}#t=${s}s`;
  }
  return l.url;
}

/** "YouTube · Sea Channel · 12:34". */
export function linkByline(l: LinkInfo): string {
  const parts = [
    l.kind === "file" ? (l.media === "audio" ? "Recording" : "Video file") : l.site,
    l.author,
    l.duration ? formatTime(l.duration) : null,
  ];
  return parts.filter(Boolean).join(" · ");
}
