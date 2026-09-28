import { describe, expect, it } from "vitest";
import { relativeLink, resolveLink, voiceMarkdown } from "./voice";

describe("voice notes in notebooks", () => {
  const note = "Notes/Me/The Lighthouse.md";
  const voice = "Notes/Me/Voice notes/2026-09-28 14-03-11.flac";

  it("links from a notebook to its recording and back", () => {
    expect(relativeLink(note, voice)).toBe("Voice notes/2026-09-28 14-03-11.flac");
    expect(relativeLink("Notes/Me/Trips/Rome.md", voice)).toBe(
      "../Voice notes/2026-09-28 14-03-11.flac",
    );
    expect(resolveLink(note, "Voice%20notes/2026-09-28%2014-03-11.flac")).toBe(voice);
    expect(resolveLink("Notes/Me/Trips/Rome.md", "../Voice%20notes/a.flac")).toBe(
      "Notes/Me/Voice notes/a.flac",
    );
    expect(resolveLink(note, "https://example.com/a.flac")).toBeNull();
    expect(resolveLink("a.md", "../../x.flac")).toBeNull();
  });

  it("writes Markdown with the transcript", () => {
    expect(voiceMarkdown(note, { path: voice, duration: 42.4, transcript: "Hello\nthere" })).toBe(
      "**Voice note** [0:42](<Voice notes/2026-09-28 14-03-11.flac>)\n\n> Hello there",
    );
  });
});
