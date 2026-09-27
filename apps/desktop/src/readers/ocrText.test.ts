import { describe, expect, it } from "vitest";
import type { WordDto } from "@/lib/ipc";
import { findHits, matchRects } from "./ocrText";

describe("finding in page text", () => {
  it("finds across pages, ignoring accents and case", () => {
    expect(findHits(["A café. Cafe!", "", "cafe"], "Café")).toEqual([
      { page: 1, index: 0 },
      { page: 1, index: 1 },
      { page: 3, index: 0 },
    ]);
  });

  it("gives the boxes of the matched words", () => {
    const words: WordDto[] = [
      { text: "the", rect: [0.1, 0.1, 0.05, 0.02] },
      { text: "lighthouse", rect: [0.16, 0.1, 0.2, 0.02] },
      { text: "keeper", rect: [0.37, 0.1, 0.1, 0.02] },
    ];
    expect(matchRects(words, "lighthouse keeper", 0)).toEqual([
      [0.16, 0.1, 0.2, 0.02],
      [0.37, 0.1, 0.1, 0.02],
    ]);
    expect(matchRects(words, "keeper", 1)).toEqual([]);
  });
});
