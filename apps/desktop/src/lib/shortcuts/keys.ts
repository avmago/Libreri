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

const KEY_ALIASES: Record<string, string> = { Space: " ", Esc: "Escape", Plus: "+" };

function normaliseKey(key: string): string {
  const k = KEY_ALIASES[key] ?? key;
  return k.length === 1 ? k.toLowerCase() : k;
}

const CODE_CHARS: Record<string, string> = {
  Slash: "/",
  Backslash: "\\",
  Equal: "=",
  Minus: "-",
  Comma: ",",
  Period: ".",
  Semicolon: ";",
  Quote: "'",
  BracketLeft: "[",
  BracketRight: "]",
  Backquote: "`",
};

/**
 * The character printed on the physical key, from `event.code`. Used when
 * modifiers change `event.key` (Alt+R is "®" on a Mac, Shift+1 is "!").
 */
export function keyFromCode(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit\d$/.test(code)) return code.slice(5);
  return CODE_CHARS[code] ?? null;
}

/** True when a keyboard event matches the shortcut exactly. */
export function matches(event: KeyboardEvent, shortcut: ParsedShortcut): boolean {
  if (
    event.ctrlKey !== shortcut.ctrl ||
    event.metaKey !== shortcut.meta ||
    event.altKey !== shortcut.alt ||
    event.shiftKey !== shortcut.shift
  ) {
    return false;
  }
  if (normaliseKey(event.key) === shortcut.key) return true;
  const modified = event.altKey || event.shiftKey || event.ctrlKey || event.metaKey;
  return modified && shortcut.key.length === 1 && keyFromCode(event.code) === shortcut.key;
}

/** Parses "g g" into one shortcut per key press. */
export function parseSequence(shortcut: string, platform: Platform): ParsedShortcut[] {
  return shortcut
    .trim()
    .split(/\s+/)
    .map((step) => parseShortcut(step, platform));
}

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph", "CapsLock", "Fn"]);

/**
 * Writes a pressed key as a shortcut string ("Mod+Shift+K"), for recording
 * new shortcuts. Returns null for a lone modifier.
 */
export function shortcutFromEvent(event: KeyboardEvent, platform: Platform): string | null {
  if (MODIFIER_KEYS.has(event.key)) return null;
  const parts: string[] = [];
  const mod = platform === "mac" ? event.metaKey : event.ctrlKey;
  if (mod) parts.push("Mod");
  if (platform === "mac" ? event.ctrlKey : event.metaKey)
    parts.push(platform === "mac" ? "Ctrl" : "Meta");
  if (event.altKey) parts.push("Alt");
  if (event.shiftKey) parts.push("Shift");
  const modified = event.altKey || event.shiftKey || event.ctrlKey || event.metaKey;
  let key = (modified && keyFromCode(event.code)) || event.key;
  if (key === " ") key = "Space";
  else if (key === "+") key = "Plus";
  else if (key.length === 1) key = key.toUpperCase();
  parts.push(key);
  return parts.join("+");
}

/**
 * Human-readable keys for display, e.g. ["Ctrl", "Shift", "K"] or
 * ["⌘", "⇧", "K"]. A sequence such as "g g" gives ["G", "then", "G"].
 */
export function displayKeys(shortcut: string, platform: Platform): string[] {
  const steps = shortcut.trim().split(/\s+/);
  if (steps.length > 1) {
    return steps.flatMap((s, i) =>
      i ? ["then", ...displayStep(s, platform)] : displayStep(s, platform),
    );
  }
  return displayStep(shortcut, platform);
}

const KEY_NAMES: Record<string, string> = {
  ArrowRight: "→",
  ArrowLeft: "←",
  ArrowUp: "↑",
  ArrowDown: "↓",
  Escape: "Esc",
  Backspace: "⌫",
  Delete: "Del",
  PageDown: "PgDn",
  PageUp: "PgUp",
};

function displayStep(shortcut: string, platform: Platform): string[] {
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
  keys.push(
    p.key === " "
      ? "Space"
      : p.key.length === 1
        ? p.key.toUpperCase()
        : (KEY_NAMES[p.key] ?? p.key),
  );
  return keys;
}
