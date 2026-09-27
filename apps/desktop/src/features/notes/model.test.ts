import { describe, expect, it } from "vitest";
import type { NoteDto } from "@/lib/ipc";
import { byBook, filterNotes, notesToMarkdown, relativeDate } from "./model";

const note = (id: string, bookId: string, extra: Partial<NoteDto["annotation"]> = {}): NoteDto => ({
  bookTitle: bookId === "a" ? "Alpha" : "Beta",
  fileType: "pdf",
  annotation: {
    id,
    bookId,
    kind: "highlight",
    color: "yellow",
    locator: "{}",
    quote: { exact: `quote ${id}`, prefix: "", suffix: "" },
    note: null,
    label: "p. 1",
    position: 0,
    createdAt: "",
    modifiedAt: "",
    ...extra,
  },
});

const all: NoteDto[] = [
  note("1", "a", { note: "Important" }),
  note("2", "b", { color: "blue" }),
  note("3", "a", { kind: "bookmark", quote: null, color: null }),
];

describe("notes hub", () => {
  it("filters by kind, colour, book and text", () => {
    const f = { search: "", kind: "all" as const, colors: [], bookId: null };
    expect(filterNotes(all, f)).toHaveLength(3);
    expect(filterNotes(all, { ...f, kind: "comments" }).map((n) => n.annotation.id)).toEqual(["1"]);
    expect(filterNotes(all, { ...f, kind: "bookmarks" }).map((n) => n.annotation.id)).toEqual([
      "3",
    ]);
    expect(filterNotes(all, { ...f, colors: ["blue"] }).map((n) => n.annotation.id)).toEqual(["2"]);
    expect(filterNotes(all, { ...f, bookId: "a" })).toHaveLength(2);
    expect(filterNotes(all, { ...f, search: "important" })).toHaveLength(1);
    expect(filterNotes(all, { ...f, search: "beta" })).toHaveLength(1);
  });

  it("groups by book and exports Markdown with links back", () => {
    expect(byBook(all).map((g) => [g.title, g.notes.length])).toEqual([
      ["Alpha", 2],
      ["Beta", 1],
    ]);
    const md = notesToMarkdown(all);
    expect(md).toContain("## Alpha");
    expect(md).toContain("> quote 1");
    expect(md).toContain("(libreri://book/a#annotation=1)");
    expect(md).toContain("Important");
    expect(md).toContain("- Bookmark: [p. 1](libreri://book/a#annotation=3)");
  });

  it("writes dates people read easily", () => {
    const now = Date.parse("2026-09-27T12:00:00Z");
    expect(relativeDate("2026-09-27T08:00:00Z", now)).toBe("Today");
    expect(relativeDate("2026-09-26T08:00:00Z", now)).toBe("Yesterday");
    expect(relativeDate("2026-09-23T08:00:00Z", now)).toBe("4 days ago");
  });
});
