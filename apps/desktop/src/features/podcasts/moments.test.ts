import { describe, expect, it } from "vitest";
import { cueAt, episodeLink, momentMarkdown, parsePodcastLink, saidAt } from "./moments";

const cues = [
  { start: 0, end: 5, speaker: null, text: "One." },
  { start: 5, end: 9, speaker: null, text: "Two." },
  { start: 9, end: null, speaker: null, text: "Three." },
];

describe("moments", () => {
  it("links to a time and reads it back", () => {
    const l = episodeLink("p1-abc", 754.6);
    expect(l).toBe("libreri://podcast/p1-abc?t=754");
    expect(parsePodcastLink(l)).toEqual({ id: "p1-abc", seconds: 754 });
    expect(parsePodcastLink("libreri://book/x")).toBeNull();
  });

  it("finds what was being said", () => {
    expect(cueAt(cues, 6)).toBe(1);
    expect(cueAt(cues, 100)).toBe(2);
    expect(saidAt(cues, 6)).toBe("One. Two.");
    expect(saidAt([], 6)).toBe("");
  });

  it("writes the moment with the page being read", () => {
    const md = momentMarkdown({
      id: "e1",
      seconds: 75,
      title: "Ep",
      show: "Show",
      said: "Two.",
      place: { bookId: "b".repeat(64), title: "Book", page: 42 },
      text: "A thought",
    });
    expect(md).toContain("**1:15** [Ep — Show](libreri://podcast/e1?t=75)");
    expect(md).toContain(`[p. 42](libreri://book/${"b".repeat(64)}#page=42)`);
    expect(md).toContain("> Two.");
    expect(md).toContain("A thought");
  });
});
