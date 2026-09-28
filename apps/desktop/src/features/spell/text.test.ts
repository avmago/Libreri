import { describe, expect, it } from "vitest";
import { matchCase, pieces, shiftMisses, wordBefore } from "./text";

describe("spell check text helpers", () => {
  it("finds the word being typed", () => {
    expect(wordBefore("the lightho", 11)).toEqual({ start: 4, prefix: "lightho" });
    expect(wordBefore("the lightho use", 8)).toBeNull();
    expect(wordBefore("the ", 4)).toBeNull();
    expect(wordBefore("Harte’s", 7)).toEqual({ start: 0, prefix: "Harte’s" });
    expect(wordBefore("x 12", 4)).toBeNull();
  });

  it("keeps marks on their words while typing", () => {
    const misses = [
      { start: 0, end: 3, word: "teh" },
      { start: 8, end: 12, word: "wrold" },
    ];
    // Typing in the middle moves the later mark.
    expect(shiftMisses(misses, "teh big wrold", "teh very big wrold")).toEqual([
      misses[0],
      { start: 13, end: 17, word: "wrold" },
    ]);
    // Editing the marked word drops its mark.
    expect(shiftMisses(misses, "teh big wrold", "the big wrold")).toEqual([misses[1]]);
  });

  it("splits text for drawing", () => {
    expect(pieces("a teh b", [{ start: 2, end: 5, word: "teh" }])).toEqual([
      { text: "a ", miss: null },
      { text: "teh", miss: 0 },
      { text: " b", miss: null },
    ]);
  });

  it("keeps capitals in corrections", () => {
    expect(matchCase("Teh", "the")).toBe("The");
    expect(matchCase("TEH", "the")).toBe("THE");
    expect(matchCase("teh", "the")).toBe("the");
  });
});
