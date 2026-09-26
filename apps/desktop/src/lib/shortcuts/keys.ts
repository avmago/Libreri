/**
 * Keyboard shortcut strings.
 *
 * Shortcuts are written like "Mod+Shift+K". `Mod` means Ctrl on Linux and
 * Windows and ⌘ on macOS, so one definition works everywhere.
 */

export type Platform = "mac" | "other";

export interface ParsedShortcut {
  ctrl: boolean;
  meta: boolean;
  alt: boolean;
  shift: boolean;
  key: string;
}

const MODIFIERS = new Set(["mod", "ctrl", "control", "meta", "cmd", "alt", "option", "shift"]);

export function detectPlatform(userAgent: string = navigator.userAgent): Platform {
  return /Mac|iPhone|iPad/.test(userAgent) ? "mac" : "other";
}

/** Parses "Mod+Shift+K" into flags for the given platform. */
export function parseShortcut(shortcut: string, platform: Platform): ParsedShortcut {
  const parts = shortcut.split("+").map((p) => p.trim());
  const key = parts.pop();
  if (!key) throw new Error(`Empty shortcut: "${shortcut}"`);
  const result: ParsedShortcut = {
    ctrl: false,
    meta: false,
    alt: false,
    shift: false,
    key: normaliseKey(key),
  };
  for (const raw of parts) {
    const m = raw.toLowerCase();
    if (!MODIFIERS.has(m)) throw new Error(`Unknown modifier "${raw}" in "${shortcut}"`);
    if (m === "mod") {
      if (platform === "mac") result.meta = true;
      else result.ctrl = true;
    } else if (m === "ctrl" || m === "control") result.ctrl = true;
    else if (m === "meta" || m === "cmd") result.meta = true;
    else if (m === "alt" || m === "option") result.alt = true;
    else if (m === "shift") result.shift = true;
  }
  return result;
}

function normaliseKey(key: string): string {
  return key.length === 1 ? key.toLowerCase() : key;
}

/** True when a keyboard event matches the shortcut exactly. */
export function matches(event: KeyboardEvent, shortcut: ParsedShortcut): boolean {
  return (
    event.ctrlKey === shortcut.ctrl &&
    event.metaKey === shortcut.meta &&
    event.altKey === shortcut.alt &&
    event.shiftKey === shortcut.shift &&
    normaliseKey(event.key) === shortcut.key
  );
}

/** Human-readable keys for display, e.g. ["Ctrl", "Shift", "K"] or ["⌘", "⇧", "K"]. */
export function displayKeys(shortcut: string, platform: Platform): string[] {
  const p = parseShortcut(shortcut, platform);
  const keys: string[] = [];
  if (platform === "mac") {
    if (p.ctrl) keys.push("⌃");
    if (p.alt) keys.push("⌥");
    if (p.shift) keys.push("⇧");
    if (p.meta) keys.push("⌘");
  } else {
    if (p.ctrl) keys.push("Ctrl");
    if (p.alt) keys.push("Alt");
    if (p.shift) keys.push("Shift");
    if (p.meta) keys.push("Super");
  }
  keys.push(p.key.length === 1 ? p.key.toUpperCase() : p.key);
  return keys;
}
