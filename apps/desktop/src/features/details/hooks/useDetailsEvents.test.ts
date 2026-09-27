import { describe, expect, it } from "vitest";
import { summarise } from "./useDetailsEvents";

describe("fill report", () => {
  it("says what happened", () => {
    const s = summarise({
      jobId: "1",
      filled: ["a", "b"],
      unsure: ["c"],
      unchanged: 1,
      failed: [{ file: "Dune", reason: "Open Library: too many requests" }],
    });
    expect(s.title).toBe("Filled in details of 2 books");
    expect(s.description).toBe(
      "1 book had no sure match · 1 book unchanged · 1 book could not be looked up\nDune: Open Library: too many requests",
    );
  });
});
