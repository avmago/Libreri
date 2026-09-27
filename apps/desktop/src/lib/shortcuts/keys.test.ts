import { describe, expect, it } from "vitest";
import {
  detectPlatform,
  displayKeys,
  matches,
  parseSequence,
  parseShortcut,
  shortcutFromEvent,
} from "./keys";

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

describe("matching by physical key", () => {
  it("matches Alt+letters on a Mac and Shift+digits", () => {
    const alt = parseShortcut("Alt+R", "mac");
    expect(matches(key({ key: "®", code: "KeyR", altKey: true }), alt)).toBe(true);
    const go = parseShortcut("Mod+Shift+1", "other");
    expect(matches(key({ key: "!", code: "Digit1", ctrlKey: true, shiftKey: true }), go)).toBe(
      true,
    );
  });

  it("records shortcuts from key presses", () => {
    expect(
      shortcutFromEvent(key({ key: "k", code: "KeyK", ctrlKey: true, shiftKey: true }), "other"),
    ).toBe("Mod+Shift+K");
    expect(shortcutFromEvent(key({ key: "Shift", shiftKey: true }), "other")).toBeNull();
    expect(shortcutFromEvent(key({ key: " ", code: "Space" }), "other")).toBe("Space");
  });

  it("shows sequences", () => {
    expect(displayKeys("g g", "other")).toEqual(["G", "then", "G"]);
    expect(parseSequence("g g", "other")).toHaveLength(2);
  });
});
