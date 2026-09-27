import { describe, expect, it } from "vitest";
import { describeText, hitPlace, markWords } from "./model";

describe("search model", () => {
  it("names places", () => {
    expect(hitPlace({ page: 12, section: null, label: null, snippet: [] })).toBe("Page 12");
    expect(hitPlace({ page: null, section: 2, label: "The Storm", snippet: [] })).toBe("The Storm");
    expect(hitPlace({ page: null, section: 2, label: null, snippet: [] })).toBe("Part 3");
  });

  it("describes text states", () => {
    const s = { emptyPages: 3, pages: 10, ocrPages: 0 };
    expect(describeText("partial", s).label).toBe("3 of 10 pages are scans without text");
    expect(describeText(null, s).tone).toBe("muted");
    expect(describeText("text", { ...s, ocrPages: 4 }).label).toContain("4 pages read with OCR");
  });

  it("marks words", () => {
    expect(markWords("The Keeper's Light", "keeper")).toEqual([
      { text: "The ", hit: false },
      { text: "Keeper", hit: true },
      { text: "'s Light", hit: false },
    ]);
    expect(markWords("abc", "")).toEqual([{ text: "abc", hit: false }]);
  });
});
