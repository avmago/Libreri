import { bookUrl } from "@/lib/ipc";

export interface RecordedVoice {
  /** In the library: `Notes/<profile>/Voice notes/….flac`. */
  path: string;
  duration: number;
  /** What was said, when it was written down. */
  transcript: string | null;
}

export function clock(seconds: number): string {
  const s = Math.max(0, Math.floor(seconds));
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

const AUDIO = /\.(flac|wav|ogg|opus|m4a|mp3|webm)$/i;

/** The path from a note at `from` to `to` (both in the library), for a
 * Markdown link that also works in other editors. */
export function relativeLink(from: string, to: string): string {
  const a = from.split("/").slice(0, -1);
  const b = to.split("/");
  let i = 0;
  while (i < a.length && i < b.length - 1 && a[i] === b[i]) i++;
  return [...a.slice(i).map(() => ".."), ...b.slice(i)].join("/");
}

/** Resolves a link in a note at `from` to a path in the library. */
export function resolveLink(from: string, href: string): string | null {
  if (/^[a-z][a-z0-9+.-]*:/i.test(href) || href.startsWith("/")) return null;
  let decoded: string;
  try {
    decoded = decodeURI(href.split("#")[0]!);
  } catch {
    return null;
  }
  const out = from.split("/").slice(0, -1);
  for (const part of decoded.split("/")) {
    if (part === "..") {
      if (!out.length) return null;
      out.pop();
    } else if (part && part !== ".") out.push(part);
  }
  return out.join("/");
}

/** Markdown for a voice note in a notebook at `notePath`. */
export function voiceMarkdown(notePath: string, r: RecordedVoice): string {
  const link = `**Voice note** [${clock(r.duration)}](<${relativeLink(notePath, r.path)}>)`;
  return r.transcript ? `${link}\n\n> ${r.transcript.replace(/\n+/g, " ")}` : link;
}

/** In a rendered note, turns links to recordings into players. */
export function attachVoicePlayers(root: HTMLElement | null, notePath: string) {
  if (!root) return;
  for (const a of root.querySelectorAll<HTMLAnchorElement>("a[href]")) {
    const href = a.getAttribute("href") ?? "";
    if (!AUDIO.test(href.split("#")[0]!)) continue;
    const path = resolveLink(notePath, href);
    if (!path) continue;
    const audio = document.createElement("audio");
    audio.controls = true;
    audio.preload = "metadata";
    audio.src = bookUrl(path);
    audio.className = "lb-voice-player";
    audio.title = a.textContent ?? "";
    a.replaceWith(audio);
  }
}

/** The recording named in a voice note's place (its `audio`). */
export function voiceOf(locator: string | null | undefined): string | null {
  try {
    const v = JSON.parse(locator ?? "") as { audio?: unknown };
    return typeof v.audio === "string" ? v.audio : null;
  } catch {
    return null;
  }
}
