import { describe, expect, it } from "vitest";
import { chapterAt, clock, progressAt, timeAt } from "./sync";

describe("audio and text in step", () => {
  const pts = [
    { t: 100, progress: 0.2 },
    { t: 200, progress: 0.4 },
  ];
  it("goes between sync points both ways", () => {
    expect(progressAt(pts, 50, 1000)).toBeCloseTo(0.1);
    expect(progressAt(pts, 150, 1000)).toBeCloseTo(0.3);
    expect(progressAt(pts, 600, 1000)).toBeCloseTo(0.7);
    expect(timeAt(pts, 0.3, 1000)).toBeCloseTo(150);
    expect(timeAt(pts, 0.7, 1000)).toBeCloseTo(600);
    expect(progressAt([], 250, 1000)).toBeCloseTo(0.25);
  });
  it("writes times and finds chapters", () => {
    expect(clock(3723)).toBe("1:02:03");
    expect(clock(65)).toBe("1:05");
    expect(chapterAt([{ start: 0 }, { start: 60 }], 59.9)).toBe(1);
    expect(chapterAt([{ start: 0 }, { start: 60 }], 30)).toBe(0);
  });
});
