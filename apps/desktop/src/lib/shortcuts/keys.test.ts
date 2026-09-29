import { describe, expect, it } from "vitest";
import {
  detectPlatform,
  displayKeys,
  keyBelongsToFocus,
  matches,
  parseSequence,
  parseShortcut,
  shortcutFromEvent,
  worksWhileTyping,
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

describe("worksWhileTyping", () => {
  it("allows Ctrl/⌘ and function keys, never bare printable keys", () => {
    expect(worksWhileTyping(parseSequence("Mod+F", "other"))).toBe(true);
    expect(worksWhileTyping(parseSequence("F3", "other"))).toBe(true);
    expect(worksWhileTyping(parseSequence("Shift+F3", "other"))).toBe(true);
    expect(worksWhileTyping(parseSequence("/", "other"))).toBe(false);
    expect(worksWhileTyping(parseSequence("n", "other"))).toBe(false);
    expect(worksWhileTyping(parseSequence("Shift+N", "other"))).toBe(false);
    expect(worksWhileTyping(parseSequence("g g", "other"))).toBe(false);
  });
});

describe("keyBelongsToFocus", () => {
  const on = (el: Element, init: KeyboardEventInit) => {
    document.body.append(el);
    let result = false;
    el.addEventListener("keydown", (e) => (result = keyBelongsToFocus(e as KeyboardEvent)));
    el.dispatchEvent(new KeyboardEvent("keydown", { ...init, bubbles: true }));
    el.remove();
    return result;
  };

  it("leaves Enter and Space to buttons and links", () => {
    expect(on(document.createElement("button"), { key: "Enter" })).toBe(true);
    expect(on(document.createElement("button"), { key: " " })).toBe(true);
    expect(on(document.createElement("button"), { key: "Enter", ctrlKey: true })).toBe(false);
    expect(on(document.createElement("div"), { key: "Enter" })).toBe(false);
    const option = document.createElement("div");
    option.setAttribute("role", "option");
    expect(on(option, { key: "Enter" })).toBe(false);
  });

  it("leaves arrows to tabs and sliders, not to plain buttons", () => {
    const tab = document.createElement("div");
    tab.setAttribute("role", "tab");
    expect(on(tab, { key: "ArrowRight" })).toBe(true);
    expect(on(document.createElement("button"), { key: "ArrowRight" })).toBe(false);
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

  it("uses the letter typed, not the key it sits on (AZERTY, Dvorak)", () => {
    const selectAll = parseShortcut("Mod+A", "other");
    const quit = parseShortcut("Mod+Q", "other");
    // AZERTY: A is where Q is on QWERTY.
    const azertyA = key({ key: "a", code: "KeyQ", ctrlKey: true });
    expect(matches(azertyA, selectAll)).toBe(true);
    expect(matches(azertyA, quit)).toBe(false);
    expect(shortcutFromEvent(azertyA, "other")).toBe("Mod+A");
    // Non-Latin layouts still reach the shortcut by its key.
    expect(matches(key({ key: "й", code: "KeyQ", ctrlKey: true }), quit)).toBe(true);
    // AZERTY digits are shifted: Ctrl+& is Ctrl+1.
    expect(
      matches(key({ key: "&", code: "Digit1", ctrlKey: true }), parseShortcut("Mod+1", "other")),
    ).toBe(true);
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
