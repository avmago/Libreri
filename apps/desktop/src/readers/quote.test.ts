import { describe, expect, it } from "vitest";
import { findQuote, makeQuote } from "./quote";

describe("text quotes", () => {
  const text = "the cat sat on the mat. later the cat sat on the hat.";

  it("records context around a selection", () => {
    const q = makeQuote(text, 4, 7);
    expect(q.exact).toBe("cat");
    expect(q.prefix).toBe("the ");
    expect(q.suffix?.startsWith(" sat on the mat")).toBe(true);
  });

  it("finds the right occurrence again using the context", () => {
    const second = text.lastIndexOf("cat");
    const q = makeQuote(text, second, second + 3);
    const edited = `A new first line. ${text}`;
    expect(findQuote(edited, q)).toEqual([second + 18, second + 21]);
    expect(findQuote(edited, { exact: "dog", prefix: "", suffix: "" })).toBeNull();
  });
});
