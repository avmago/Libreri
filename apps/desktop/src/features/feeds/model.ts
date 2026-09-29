import type { FeedDto, FeedFolder, FeedItem } from "@/lib/ipc";

export interface TreeFolder {
  folder: FeedFolder;
  folders: TreeFolder[];
  feeds: FeedDto[];
  unread: number;
}

/** Folders with their subfolders and feeds, sorted by name; unread counts
 * include everything inside. Returns the top-level folders and feeds. */
export function feedTree(
  folders: FeedFolder[],
  feeds: FeedDto[],
): { folders: TreeFolder[]; feeds: FeedDto[] } {
  const byName =
    <T>(name: (t: T) => string) =>
    (a: T, b: T) =>
      name(a).localeCompare(name(b), undefined, { sensitivity: "base", numeric: true });
  const known = new Set(folders.map((f) => f.id));
  const build = (parent: string | null, seen: Set<string>): TreeFolder[] =>
    folders
      .filter((f) => (f.parent && known.has(f.parent) ? f.parent : null) === parent)
      .filter((f) => !seen.has(f.id))
      .map((folder) => {
        const inner = new Set(seen).add(folder.id);
        const sub = build(folder.id, inner);
        const own = feeds.filter((x) => x.folder === folder.id).sort(byName((x) => x.title));
        const unread =
          own.reduce((n, x) => n + x.unread, 0) + sub.reduce((n, x) => n + x.unread, 0);
        return { folder, folders: sub, feeds: own, unread };
      })
      .sort(byName((t) => t.folder.name));
  return {
    folders: build(null, new Set()),
    feeds: feeds.filter((x) => !x.folder || !known.has(x.folder)).sort(byName((x) => x.title)),
  };
}

/** Folders in tree order with their depth, for choosing one. */
export function flatFeedFolders(folders: FeedFolder[]): { folder: FeedFolder; depth: number }[] {
  const out: { folder: FeedFolder; depth: number }[] = [];
  const walk = (list: TreeFolder[], depth: number) => {
    for (const t of list) {
      out.push({ folder: t.folder, depth });
      walk(t.folders, depth + 1);
    }
  };
  walk(feedTree(folders, []).folders, 0);
  return out;
}

/** "Ada Lovelace, Alan Turing et al." */
export function authorsLine(authors: string[], max = 3): string {
  if (authors.length <= max) return authors.join(", ");
  return `${authors.slice(0, max).join(", ")} et al.`;
}

/** "Today", "Yesterday", "3 days ago" or a date. */
export function whenLine(iso: string | null | undefined, now = new Date()): string {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const day = (x: Date) => Math.floor((x.getTime() - x.getTimezoneOffset() * 60_000) / 86_400_000);
  const days = day(now) - day(d);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  return d.toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: d.getFullYear() === now.getFullYear() ? undefined : "numeric",
  });
}

/** What kind of download an item gives. */
export function downloadKind(item: FeedItem): "pdf" | "article" {
  return item.pdf ? "pdf" : "article";
}

/** arXiv's announcement types, as shown. */
export function announceLabel(a: string | null | undefined): string | null {
  switch (a) {
    case "cross":
      return "Cross-list";
    case "replace":
    case "replace-cross":
      return "Updated";
    default:
      return null;
  }
}
