/**
 * The shortcut registry: every action has an ID and a default shortcut.
 * Components never add their own keydown listeners; they register an
 * action handler here instead (docs/code-structure.md, "Where things go").
 * Later phases load user overrides from settings.
 */
import { createContext, createElement, useContext, useEffect, useRef, type ReactNode } from "react";
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
  "tabs.close": "Mod+W",
  "tabs.next": "Ctrl+Tab",
  "tabs.previous": "Ctrl+Shift+Tab",
  "reader.next": "ArrowRight",
  "reader.previous": "ArrowLeft",
  "reader.pageDown": "PageDown",
  "reader.pageUp": "PageUp",
  "reader.space": "Space",
  "reader.find": "Mod+F",
  "reader.zoomIn": "Mod+=",
  "reader.zoomOut": "Mod+-",
  "reader.zoomReset": "Mod+0",
  "reader.goToPage": "Mod+G",
  "reader.bookmark": "Mod+D",
  "reader.contents": "Mod+B",
  "reader.notebook": "Mod+J",
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
  "tabs.close",
  "tabs.next",
  "tabs.previous",
  "reader.find",
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

interface Entry {
  handler: () => void;
  /** Registered inside a `ShortcutScope` (a view), not app-wide. */
  scoped: boolean;
  active: () => boolean;
}

/** Newest registration last. */
const handlers = new Map<ActionId, Entry[]>();

function onKeyDown(event: KeyboardEvent) {
  if (event.defaultPrevented) return;
  const typing = isTyping(event.target);
  const overlay = inOverlay(event.target);
  let global: Entry | undefined;
  for (const [id, shortcut] of parsed) {
    if (!matches(event, shortcut)) continue;
    if (!(WORK_WHILE_TYPING.has(id) || (!typing && !overlay))) continue;
    // Newest first.
    const entries = (handlers.get(id) ?? []).filter((e) => e.active()).reverse();
    const scoped = entries.find((e) => e.scoped);
    if (scoped) {
      // The view the user is looking at wins over app-wide actions.
      event.preventDefault();
      scoped.handler();
      return;
    }
    global ??= entries.find((e) => !e.scoped);
  }
  if (global) {
    event.preventDefault();
    global.handler();
  }
}

let listening = false;

const ScopeContext = createContext<boolean | null>(null);

/**
 * Marks part of the interface (the library, one reader tab) whose shortcuts
 * only work while it is showing.
 */
export function ShortcutScope({ active, children }: { active: boolean; children: ReactNode }) {
  return createElement(ScopeContext.Provider, { value: active }, children);
}

/** Registers a handler for an action while the calling component is mounted. */
export function useShortcut(id: ActionId, handler: () => void): void {
  const scope = useContext(ScopeContext);
  const activeRef = useRef(scope ?? true);
  useEffect(() => {
    activeRef.current = scope ?? true;
  }, [scope]);
  useEffect(() => {
    if (!listening) {
      window.addEventListener("keydown", onKeyDown);
      listening = true;
    }
    const entry: Entry = { handler, scoped: scope !== null, active: () => activeRef.current };
    handlers.set(id, [...(handlers.get(id) ?? []), entry]);
    return () => {
      handlers.set(
        id,
        (handlers.get(id) ?? []).filter((e) => e !== entry),
      );
    };
  }, [id, handler, scope]);
}
