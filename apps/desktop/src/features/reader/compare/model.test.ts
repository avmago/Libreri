import { describe, expect, it } from "vitest";
import type { Change } from "@/lib/ipc";
import { byPair, counts, pairLabel, step } from "./model";

const ch = (kind: Change["kind"], pair: number): Change => ({
  kind,
  pair,
  aRects: [],
  bRects: [],
  aText: "",
  bText: "",
});

describe("compare view helpers", () => {
  it("labels page pairs", () => {
    expect(pairLabel({ a: 3, b: 3 })).toBe("p. 3");
    expect(pairLabel({ a: 3, b: 4 })).toBe("p. 3 / 4");
    expect(pairLabel({ a: null, b: 4 })).toBe("– / p. 4");
  });

  it("counts and groups changes", () => {
    const list = [ch("added", 0), ch("changed", 0), ch("look", 2), ch("pageAdded", 3)];
    expect(counts(list)).toEqual({ removed: 0, added: 1, changed: 1, look: 1, pages: 1 });
    expect([...byPair(list).keys()]).toEqual([0, 2, 3]);
  });

  it("steps through changes, wrapping", () => {
    expect(step(3, null, 1)).toBe(0);
    expect(step(3, null, -1)).toBe(2);
    expect(step(3, 2, 1)).toBe(0);
    expect(step(0, null, 1)).toBeNull();
  });
});
