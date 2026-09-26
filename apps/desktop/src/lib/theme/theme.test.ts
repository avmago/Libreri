import { describe, expect, it } from "vitest";
import { nextTheme, themeClasses } from ".";

describe("themeClasses", () => {
  it("follows the OS only for System", () => {
    expect(themeClasses("system", true)).toEqual(["dark"]);
    expect(themeClasses("system", false)).toEqual([]);
    expect(themeClasses("light", true)).toEqual([]);
    expect(themeClasses("dark", false)).toEqual(["dark"]);
  });

  it("high contrast is a dark theme with extra tokens", () => {
    expect(themeClasses("highContrast", false)).toEqual(["dark", "hc"]);
  });
});

describe("nextTheme", () => {
  it("cycles through every theme", () => {
    expect(nextTheme("system")).toBe("light");
    expect(nextTheme("highContrast")).toBe("system");
  });
});
