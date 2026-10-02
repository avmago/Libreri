import { describe, expect, it } from "vitest";
import { Rating } from "ts-fsrs";
import type { NoteDto } from "@/lib/ipc";
import {
  ankiNotes,
  answer,
  buildCards,
  clozeParts,
  DEFAULT_REVIEW,
  dueQueue,
  forecast,
  interval,
  kindsOf,
  opening,
  parseReview,
  previews,
  streak,
  type ReviewData,
} from "./model";

const note = (
  id: string,
  exact: string,
  comment: string | null = null,
  created = "2026-09-01T10:00:00Z",
): NoteDto => ({
  bookTitle: "The Lighthouse",
  fileType: "epub",
  annotation: {
    id,
    bookId: "b1",
    kind: "highlight",
    color: "yellow",
    locator: "{}",
    quote: { exact },
    note: comment,
    label: "Chapter 2",
    position: 0.2,
    createdAt: created,
    modifiedAt: created,
  },
});

const now = new Date("2026-10-02T09:00:00");

describe("cards", () => {
  it("makes a passage from every highlight, and Q&A from a comment", () => {
    const cards = buildCards(
      [note("a", "one two three four five"), note("b", "six seven", "Why?")],
      DEFAULT_REVIEW,
    );
    expect(cards.map((c) => c.key)).toEqual(["a:passage", "b:passage", "b:qa"]);
  });

  it("follows chosen kinds, leaving out, books left out and the auto switch", () => {
    expect(kindsOf("q", { kinds: ["qa"] }, true)).toEqual(["qa"]);
    expect(kindsOf(null, { kinds: ["qa"] }, true)).toEqual([]);
    expect(kindsOf(null, { off: true }, true)).toEqual([]);
    expect(kindsOf(null, undefined, false)).toEqual([]);
    expect(kindsOf(null, { cloze: ["x"] }, true)).toEqual(["passage", "cloze"]);
    const d: ReviewData = { ...DEFAULT_REVIEW, booksOff: ["b1"] };
    expect(buildCards([note("a", "x y")], d)).toEqual([]);
  });

  it("keeps the day's limits and puts due cards first", () => {
    const notes = Array.from({ length: 15 }, (_, i) =>
      note(`n${i}`, `quote ${i}`, null, `2026-09-${String(i + 1).padStart(2, "0")}T00:00:00Z`),
    );
    let d: ReviewData = {
      ...DEFAULT_REVIEW,
      settings: { ...DEFAULT_REVIEW.settings, newPerDay: 3 },
    };
    const cards = buildCards(notes, d);
    let q = dueQueue(cards, d, now);
    expect(q.news.map((c) => c.annotationId)).toEqual(["n0", "n1", "n2"]);
    d = answer(d, q.news[0]!, Rating.Good, now);
    q = dueQueue(cards, d, now);
    // One new card done today: two more new ones, and the answered card
    // (a short learning step) is due again today.
    expect(q.news).toHaveLength(2);
    expect(q.reviews.map((c) => c.annotationId)).toEqual(["n0"]);
    expect(forecast(cards, d, now)[0]).toBe(1);
  });

  it("answers move a card on, and Easy waits longer than Again", () => {
    const [c] = buildCards([note("a", "one two three four")], DEFAULT_REVIEW);
    const p = previews(c!, DEFAULT_REVIEW, now);
    expect(p[Rating.Again]).toMatch(/min/);
    const easy = answer(DEFAULT_REVIEW, c!, Rating.Easy, now);
    expect(new Date(easy.sched["a:passage"]!.due).getTime()).toBeGreaterThan(
      now.getTime() + 86_400_000,
    );
    expect(easy.log).toHaveLength(1);
    expect(easy.log[0]!.new).toBe(true);
    // Survives saving and reading back.
    const back = parseReview(JSON.stringify(easy));
    const again = answer(back, c!, Rating.Good, new Date(easy.sched["a:passage"]!.due));
    expect(again.log[1]!.new).toBeUndefined();
    expect(streak(again, now)).toBe(1);
  });
});

describe("faces", () => {
  it("shows the start of a passage", () => {
    const o = opening("one two three four five six seven eight nine ten");
    expect(o.start.trim()).toBe("one two three four five");
    expect(o.rest.trim()).toBe("six seven eight nine ten");
    expect(opening("short one").rest).toBe("");
  });

  it("hides whole words only", () => {
    const parts = clozeParts("The cat sat on the category mat.", ["cat"]);
    expect(parts.filter((p) => p.hidden).map((p) => p.text)).toEqual(["cat"]);
    expect(parts.map((p) => p.text).join("")).toBe("The cat sat on the category mat.");
  });

  it("formats intervals", () => {
    const t = (ms: number) => interval(now, new Date(now.getTime() + ms));
    expect(t(10 * 60_000)).toBe("10 min");
    expect(t(3 * 86_400_000)).toBe("3 d");
    expect(t(60 * 86_400_000)).toBe("2 mo");
  });
});

describe("anki", () => {
  it("writes basic and cloze notes with escaped text", () => {
    const d: ReviewData = { ...DEFAULT_REVIEW, cards: { b: { cloze: ["sea"] } } };
    const cards = buildCards(
      [note("a", "a < b and c > d, e f", "What?"), note("b", "the sea is grey")],
      d,
    );
    const n = ankiNotes(cards);
    expect(n.map((x) => [x.id, x.model])).toEqual([
      ["a:passage", "basic"],
      ["a:qa", "basic"],
      ["b:passage", "basic"],
      ["b:cloze", "cloze"],
    ]);
    expect(n[1]!.fields).toEqual([
      "What?",
      "a &lt; b and c &gt; d, e f",
      "The Lighthouse · Chapter 2",
    ]);
    expect(n[3]!.fields[0]).toBe("the {{c1::sea}} is grey");
    expect(n[0]!.tags).toEqual(["libreri", "The_Lighthouse"]);
  });
});
