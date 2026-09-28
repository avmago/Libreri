import { describe, expect, it } from "vitest";
import {
  duplicate,
  initial,
  insert,
  move,
  parseRanges,
  remove,
  rotate,
  setCrop,
  setRedactions,
  splitAt,
  splitEvery,
  summary,
  toPlan,
  turnBox,
} from "./model";

describe("edit pages", () => {
  it("moves, turns and removes pages", () => {
    let s = initial(4);
    s = move(s, ["book-3", "book-4"], 0);
    expect(s.pages.map((p) => p.uid)).toEqual(["book-3", "book-4", "book-1", "book-2"]);
    s = rotate(s, ["book-1"], 90);
    s = rotate(s, ["book-1"], 90);
    s = remove(s, ["book-2"]);
    const plan = toPlan(s);
    expect(plan.pages).toEqual([
      { kind: "page", page: 3, rotate: 0, crop: null },
      { kind: "page", page: 4, rotate: 0, crop: null },
      { kind: "page", page: 1, rotate: 180, crop: null },
    ]);
    expect(summary(s, 4)).toEqual(["1 page removed", "pages reordered", "1 page turned"]);
  });

  it("moves forward past the end", () => {
    const s = move(initial(3), ["book-1"], 3);
    expect(s.pages.map((p) => p.uid)).toEqual(["book-2", "book-3", "book-1"]);
  });

  it("turns crops with their page", () => {
    expect(turnBox([0, 0, 0.5, 0.25], 90)).toEqual([0.75, 0, 0.25, 0.5]);
    for (const deg of [90, 180, 270]) {
      let b = turnBox([0.1, 0.2, 0.3, 0.4], deg);
      b = turnBox(b, 360 - deg);
      expect(b.map((v) => Math.round(v * 1000) / 1000)).toEqual([0.1, 0.2, 0.3, 0.4]);
    }
    let s = setCrop(initial(1), ["book-1"], [0, 0, 0.5, 0.5]);
    s = rotate(s, ["book-1"], 90);
    expect(s.pages[0]!.crop).toEqual([0.5, 0, 0.5, 0.5]);
    expect(setCrop(s, ["book-1"], [0, 0, 1, 1]).pages[0]!.crop).toBeNull();
  });

  it("keeps redactions only for pages that stay", () => {
    let s = setRedactions(initial(2), 2, [[0.1, 0.1, 0.2, 0.05]]);
    expect(toPlan(s).redactions).toHaveLength(1);
    s = remove(s, ["book-2"]);
    expect(toPlan(s).redactions).toHaveLength(0);
  });

  it("adds pages and copies", () => {
    let s = duplicate(initial(2), ["book-1"]);
    s = insert(s, 1, [{ kind: "blank", width: 100, height: 200 }]);
    expect(toPlan(s).pages.map((p) => p.kind)).toEqual(["page", "blank", "page", "page"]);
    expect(summary(s, 2)).toEqual(["2 pages added"]);
    expect(toPlan(s, [s.pages[0]!.uid]).pages).toHaveLength(1);
  });

  it("splits", () => {
    const s = initial(5);
    expect(splitEvery(s, 2).map((p) => p.length)).toEqual([2, 2, 1]);
    expect(splitAt(s, ["book-1", "book-4"])).toEqual([
      ["book-1", "book-2", "book-3"],
      ["book-4", "book-5"],
    ]);
  });

  it("reads page ranges", () => {
    expect(parseRanges("1-3, 5, 9-", 10)).toEqual([1, 2, 3, 5, 9, 10]);
    expect(parseRanges("12", 10)).toEqual([]);
    expect(parseRanges("two", 10)).toEqual([]);
  });
});
