import { describe, expect, it } from "vitest";
import { PAGE_THEMES, softLink } from "./themes";

describe("softLink", () => {
  it("mixes the link colour 70/30 with the text", () => {
    expect(softLink({ link: "#0000ff", fg: "#ffffff" })).toBe("#4d4dff");
  });
  it("gives every theme a six-digit colour", () => {
    for (const t of PAGE_THEMES) expect(softLink(t)).toMatch(/^#[0-9a-f]{6}$/);
  });
});
