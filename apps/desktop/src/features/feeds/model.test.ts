import { describe, expect, it } from "vitest";
import type { FeedDto, FeedFolder } from "@/lib/ipc";
import { authorsLine, feedTree, flatFeedFolders, whenLine } from "./model";

const folder = (id: string, name: string, parent: string | null = null): FeedFolder => ({
  id,
  name,
  parent,
  autoDownload: false,
});
const feed = (id: string, title: string, folderId: string | null, unread: number): FeedDto => ({
  id,
  url: `https://x.org/${id}`,
  title,
  site: null,
  folder: folderId,
  autoDownload: false,
  autoFromFolder: false,
  checkedAt: null,
  error: null,
  unread,
  total: unread,
  author: null,
  artwork: null,
  speed: null,
});

describe("feeds", () => {
  it("builds the tree with unread counts", () => {
    const folders = [folder("a", "Science"), folder("b", "Physics", "a"), folder("c", "Art")];
    const feeds = [
      feed("1", "hep-th", "b", 3),
      feed("2", "Nature", "a", 2),
      feed("3", "HN", null, 5),
      feed("4", "Lost", "gone", 1),
    ];
    const t = feedTree(folders, feeds);
    expect(t.folders.map((f) => f.folder.name)).toEqual(["Art", "Science"]);
    expect(t.folders[1]!.unread).toBe(5);
    expect(t.folders[1]!.folders[0]!.feeds[0]!.title).toBe("hep-th");
    expect(t.feeds.map((f) => f.title)).toEqual(["HN", "Lost"]);
    expect(flatFeedFolders(folders).map((f) => [f.folder.name, f.depth])).toEqual([
      ["Art", 0],
      ["Science", 0],
      ["Physics", 1],
    ]);
  });

  it("writes authors and dates", () => {
    expect(authorsLine(["A", "B", "C", "D"])).toBe("A, B, C et al.");
    const now = new Date("2026-09-29T12:00:00");
    expect(whenLine("2026-09-29T08:00:00", now)).toBe("Today");
    expect(whenLine("2026-09-28T08:00:00", now)).toBe("Yesterday");
    expect(whenLine("2026-09-25T08:00:00", now)).toBe("4 days ago");
    expect(whenLine(null, now)).toBe("");
  });
});
