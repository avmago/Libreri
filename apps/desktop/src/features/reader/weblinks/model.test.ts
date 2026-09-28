import { describe, expect, it } from "vitest";
import { formatTime, linkHref, linkOf, parseTime } from "./model";

describe("links", () => {
  it("reads and writes times", () => {
    expect(parseTime("90")).toBe(90);
    expect(parseTime("1:30")).toBe(90);
    expect(parseTime("1:02:03")).toBe(3723);
    expect(parseTime("1m30s")).toBe(90);
    expect(parseTime("")).toBeNull();
    expect(parseTime("soon")).toBeNaN();
    expect(formatTime(3723)).toBe("1:02:03");
    expect(formatTime(67)).toBe("1:07");
  });

  it("finds the link in a locator and where it goes", () => {
    const l = linkOf(
      JSON.stringify({
        type: "pdf",
        page: 3,
        link: {
          url: "https://www.youtube.com/watch?v=abc123def45",
          kind: "video",
          title: "Keepers",
          start: 90,
          video: { provider: "youtube", id: "abc123def45" },
        },
      }),
    );
    expect(l?.title).toBe("Keepers");
    expect(linkHref(l!)).toBe("https://www.youtube.com/watch?v=abc123def45&t=90s");
    expect(linkOf("{")).toBeNull();
  });
});
