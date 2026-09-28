import { commands, unwrap, type KnownVideo } from "@/lib/ipc";

/** Fetches a link's details once, when it is added. */
export const fetchLink = (url: string) => unwrap(commands.linkFetch(url));
/** Keeps the fetched picture and, if asked, an offline copy of the page. */
export const saveLink = (url: string, title: string, keepCopy: boolean) =>
  unwrap(commands.linkSave(url, title, keepCopy));
/** A video or audio file chosen to link to. */
export const linkFile = (path: string) => unwrap(commands.linkFile(path));
/** Where a linked file plays from (a `book://` path). */
export const mediaUrl = (file: string) => unwrap(commands.linkMediaUrl(file));
export const playerFor = (
  url: string,
  video: KnownVideo | null,
  embed: string | null,
  start: number | null,
) => unwrap(commands.linkPlayer(url, video, embed, start));
export const popOut = (page: string, title: string) => unwrap(commands.linkPopOut(page, title));
export const openLinkFile = (file: string) => unwrap(commands.linkOpenFile(file));
export const openWeb = (url: string) => unwrap(commands.openExternalUrl(url));
