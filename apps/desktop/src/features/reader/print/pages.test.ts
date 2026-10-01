import { describe, expect, it } from "vitest";
import { parsePages } from "./pages";

describe("print pages", () => {
  it("reads ranges and single pages", () => {
    expect(parsePages("1-3, 5", 10)).toEqual([1, 2, 3, 5]);
    expect(parsePages("8-", 10)).toEqual([8, 9, 10]);
    expect(parsePages("2, 2, 1-2", 10)).toEqual([2, 1]);
    expect(parsePages("9-20", 10)).toEqual([9, 10]);
  });
  it("refuses what it cannot read", () => {
    expect(parsePages("", 10)).toBeNull();
    expect(parsePages("a", 10)).toBeNull();
    expect(parsePages("5-2", 10)).toBeNull();
    expect(parsePages("11", 10)).toBeNull();
  });
});
