/**
 * The shortcut registry: every action has an ID and a default shortcut.
 * Components never add their own keydown listeners; they register an
 * action handler here instead (docs/code-structure.md, "Where things go").
 * Later phases load user overrides from settings.
 */
import { useEffect } from "react";
import { detectPlatform, matches, parseShortcut, type ParsedShortcut } from "./keys";

export const DEFAULT_SHORTCUTS = {
  "palette.open": "Mod+K",
  "settings.open": "Mod+,",
  "shortcuts.show": "Mod+/",
  "theme.toggle": "Mod+Shift+L",
  "sidebar.toggle": "Mod+B",
  "library.close": "Mod+Shift+W",
} as const;

export type ActionId = keyof typeof DEFAULT_SHORTCUTS;

export const platform = detectPlatform();

const parsed = new Map<ActionId, ParsedShortcut>(
  (Object.entries(DEFAULT_SHORTCUTS) as [ActionId, string][]).map(([id, s]) => [
    id,
    parseShortcut(s, platform),
  ]),
);

const handlers = new Map<ActionId, () => void>();

function onKeyDown(event: KeyboardEvent) {
  for (const [id, shortcut] of parsed) {
    const handler = handlers.get(id);
    if (handler && matches(event, shortcut)) {
      event.preventDefault();
      handler();
      return;
    }
  }
}

let listening = false;

/** Registers a handler for an action while the calling component is mounted. */
export function useShortcut(id: ActionId, handler: () => void): void {
  useEffect(() => {
    if (!listening) {
      window.addEventListener("keydown", onKeyDown);
      listening = true;
    }
    handlers.set(id, handler);
    return () => {
      if (handlers.get(id) === handler) handlers.delete(id);
    };
  }, [id, handler]);
}
