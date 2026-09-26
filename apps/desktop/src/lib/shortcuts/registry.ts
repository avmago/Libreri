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
  "library.search": "Mod+F",
  "library.import": "Mod+O",
  "library.importFolder": "Mod+Shift+O",
  "library.newFolder": "Mod+Shift+N",
  "library.refresh": "F5",
  "view.grid": "Mod+1",
  "view.list": "Mod+2",
  "details.toggle": "Mod+I",
  "details.edit": "Mod+E",
  "details.save": "Mod+S",
  "books.selectAll": "Mod+A",
  "books.clearSelection": "Escape",
  "books.open": "Enter",
  "books.reveal": "Mod+Shift+R",
  "books.trash": "Delete",
  "books.trashMac": "Mod+Backspace",
  "books.favorite": "Mod+D",
  "books.next": "ArrowRight",
  "books.previous": "ArrowLeft",
  "books.down": "ArrowDown",
  "books.up": "ArrowUp",
} as const;

export type ActionId = keyof typeof DEFAULT_SHORTCUTS;

/** Actions that still work while typing in a text field. */
const WORK_WHILE_TYPING = new Set<ActionId>([
  "palette.open",
  "settings.open",
  "shortcuts.show",
  "theme.toggle",
  "sidebar.toggle",
  "library.close",
  "library.search",
  "details.save",
]);

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  );
}

/** Inside a dialog or menu, keys belong to it (Enter, Delete, arrows…). */
function inOverlay(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest("[role=dialog], [role=alertdialog], [role=menu], [role=listbox]") !== null
  );
}

export const platform = detectPlatform();

const parsed = new Map<ActionId, ParsedShortcut>(
  (Object.entries(DEFAULT_SHORTCUTS) as [ActionId, string][]).map(([id, s]) => [
    id,
    parseShortcut(s, platform),
  ]),
);

const handlers = new Map<ActionId, () => void>();

function onKeyDown(event: KeyboardEvent) {
  if (event.defaultPrevented) return;
  const typing = isTyping(event.target);
  const overlay = inOverlay(event.target);
  for (const [id, shortcut] of parsed) {
    const handler = handlers.get(id);
    if (!handler || !matches(event, shortcut)) continue;
    const allowed = WORK_WHILE_TYPING.has(id) || (!typing && !overlay);
    if (allowed) {
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
