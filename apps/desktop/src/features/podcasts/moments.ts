/**
 * Moments in an episode: a link to a time (`libreri://podcast/<id>?t=754`),
 * opening one, and "Note this moment", which writes the moment (and what
 * was being said) into the notebook of the book being read, with a link to
 * the page, or into the "Podcast moments" note when no book is open.
 */
import { toast } from "sonner";
import { commands, unwrap, type Cue } from "@/lib/ipc";
import { clock } from "./model";
import { usePlayer } from "./player";

export function episodeLink(id: string, seconds: number) {
  return `libreri://podcast/${encodeURIComponent(id)}?t=${Math.floor(seconds)}`;
}

export function parsePodcastLink(href: string): { id: string; seconds: number } | null {
  const m = /^libreri:\/\/podcast\/([^?#\s]+)(?:\?t=(\d+))?$/i.exec(href.trim());
  if (!m) return null;
  return { id: decodeURIComponent(m[1]!), seconds: Number(m[2] ?? 0) };
}

/** Plays the episode of a link from its moment. True when it was one. */
export function openPodcastLink(href: string): boolean {
  const link = parsePodcastLink(href);
  if (!link) return false;
  void unwrap(commands.podcastEpisodes([link.id]))
    .then(([item]) => {
      if (!item) throw new Error("the episode is no longer in Podcasts");
      usePlayer.getState().playAt(item, link.seconds);
    })
    .catch((e: unknown) =>
      toast.error("Could not play the moment", {
        description: e instanceof Error ? e.message : String(e),
      }),
    );
  return true;
}

/** The transcript line at a time (the last one started by then). */
export function cueAt(cues: Cue[], seconds: number): number {
  let at = -1;
  for (let i = 0; i < cues.length; i++) {
    const s = cues[i]!.start;
    if (s === null) continue;
    if (s <= seconds + 0.25) at = i;
    else break;
  }
  return at;
}

/** What was said around a moment: the line, and the one before it. */
export function saidAt(cues: Cue[], seconds: number): string {
  const i = cueAt(cues, seconds);
  if (i < 0) return "";
  return cues
    .slice(Math.max(0, i - 1), i + 1)
    .map((c) => c.text.trim())
    .join(" ")
    .replace(/\s+/g, " ")
    .slice(0, 400);
}

/** Where a book is being read, for the note's link to the page. */
export interface ReadingPlace {
  bookId: string;
  title: string;
  page: number | null;
}

/** The Markdown written for a moment. */
export function momentMarkdown(a: {
  id: string;
  seconds: number;
  title: string;
  show: string;
  said: string;
  place: ReadingPlace | null;
  text?: string;
}): string {
  const at = clock(a.seconds);
  const page =
    a.place?.page != null
      ? ` · while reading [p. ${a.place.page}](libreri://book/${a.place.bookId}#page=${a.place.page})`
      : "";
  const lines = [`- **${at}** [${a.title} — ${a.show}](${episodeLink(a.id, a.seconds)})${page}`];
  if (a.said) lines.push(`  > ${a.said}`);
  if (a.text?.trim()) lines.push(`  ${a.text.trim().replace(/\n+/g, " ")}`);
  return lines.join("\n");
}

const MOMENTS_NOTE = "Podcast moments";

/** Saves a moment: into the book's notebook, or the Podcast moments note. */
export async function saveMoment(md: string, place: ReadingPlace | null): Promise<string> {
  if (place) {
    const nb = await unwrap(commands.getNotebook(place.bookId));
    const content = `${nb.content.replace(/\s*$/, "")}\n\n${md}\n`;
    await unwrap(commands.saveNotebook(place.bookId, content));
    return `Saved in the notebook of ${place.title}`;
  }
  const list = await unwrap(commands.listNotebooks());
  let rel = list.find((n) => n.bookId === null && n.title === MOMENTS_NOTE)?.relPath;
  rel ??= await unwrap(commands.createNote(MOMENTS_NOTE));
  const old = await unwrap(commands.readNote(rel));
  await unwrap(commands.writeNote(rel, `${old.replace(/\s*$/, "")}\n\n${md}\n`));
  return `Saved in the note “${MOMENTS_NOTE}”`;
}
