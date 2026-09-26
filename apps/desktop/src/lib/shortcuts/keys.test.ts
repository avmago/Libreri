import { describe, expect, it } from "vitest";
import { detectPlatform, displayKeys, matches, parseShortcut } from "./keys";

const key = (init: KeyboardEventInit) => new KeyboardEvent("keydown", init);

describe("parseShortcut", () => {
  it("maps Mod to Ctrl on Linux/Windows and ⌘ on macOS", () => {
    expect(parseShortcut("Mod+K", "other")).toMatchObject({ ctrl: true, meta: false, key: "k" });
    expect(parseShortcut("Mod+K", "mac")).toMatchObject({ ctrl: false, meta: true, key: "k" });
  });

  it("rejects unknown modifiers", () => {
    expect(() => parseShortcut("Hyper+K", "other")).toThrow(/Unknown modifier/);
  });
});

describe("matches", () => {
  it("requires the exact modifiers", () => {
    const s = parseShortcut("Mod+Shift+F", "other");
    expect(matches(key({ key: "F", ctrlKey: true, shiftKey: true }), s)).toBe(true);
    expect(matches(key({ key: "f", ctrlKey: true }), s)).toBe(false);
    expect(matches(key({ key: "f", ctrlKey: true, shiftKey: true, altKey: true }), s)).toBe(false);
  });
});

describe("displayKeys", () => {
  it("uses words on Linux and symbols on macOS", () => {
    expect(displayKeys("Mod+Shift+K", "other")).toEqual(["Ctrl", "Shift", "K"]);
    expect(displayKeys("Mod+Shift+K", "mac")).toEqual(["⇧", "⌘", "K"]);
  });
});

describe("detectPlatform", () => {
  it("recognises macOS and treats everything else alike", () => {
    expect(detectPlatform("Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0)")).toBe("mac");
    expect(detectPlatform("Mozilla/5.0 (X11; Linux x86_64)")).toBe("other");
  });
});
