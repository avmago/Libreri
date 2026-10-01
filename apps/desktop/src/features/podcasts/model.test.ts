import { describe, expect, it } from "vitest";
import { heard, initials, lengthLine, minutes, moveInQueue } from "./model";

describe("podcasts", () => {
  it("says how long", () => {
    expect(minutes(3480)).toBe("58 min");
    expect(minutes(3840)).toBe("1 h 4 min");
    expect(minutes(7200)).toBe("2 h");
    expect(minutes(null)).toBe("");
  });

  it("says how much is left", () => {
    expect(lengthLine({ position: 1320, duration: 3780, played: false })).toBe("41 min left of 63");
    expect(lengthLine({ position: null, duration: 3480, played: false })).toBe("58 min");
    expect(lengthLine({ position: 10, duration: 3480, played: true })).toBe("Played");
  });

  it("knows how far listening got", () => {
    expect(heard({ position: 30, duration: 60, played: false })).toBe(0.5);
    expect(heard({ position: null, duration: 60, played: false })).toBeNull();
    expect(heard({ position: null, duration: 60, played: true })).toBe(1);
  });

  it("reorders the queue", () => {
    expect(moveInQueue(["a", "b", "c"], "b", -1)).toEqual(["b", "a", "c"]);
    expect(moveInQueue(["a", "b", "c"], "c", 1)).toEqual(["a", "b", "c"]);
  });

  it("makes initials", () => {
    expect(initials("The Proof Hour")).toBe("PH");
    expect(initials("Margins & Footnotes")).toBe("MF");
  });
});
