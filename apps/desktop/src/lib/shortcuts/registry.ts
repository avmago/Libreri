/**
 * The shortcut registry: every action has an ID and keys (see actions.ts).
 * Components never add their own keydown listeners; they register an
 * action handler here instead (docs/code-structure.md, "Where things go").
 * Each profile's changed keys and the optional Vim keys are applied with
 * `applyShortcutPrefs`.
 */
import { createContext, createElement, useContext, useEffect, useRef, type ReactNode } from "react";
import { create } from "zustand";
import { ACTION_IDS, ACTIONS, VIM_KEYS, type ActionId } from "./actions";
import { detectPlatform, matches, parseSequence, type ParsedShortcut } from "./keys";

export type { ActionId } from "./actions";

/** Default keys by action (null = none). */
export const DEFAULT_SHORTCUTS = Object.fromEntries(
  ACTION_IDS.map((id) => [id, ACTIONS[id].keys]),
) as Record<ActionId, string | null>;

export const platform = detectPlatform();

interface BindingState {
  overrides: Partial<Record<ActionId, string | null>>;
  vim: boolean;
}

/** The keys in use right now (defaults plus the profile's changes). */
export const useShortcutBindings = create<BindingState>(() => ({ overrides: {}, vim: false }));

/** Applies a profile's shortcut changes and Vim keys. */
export function applyShortcutPrefs(overrides: Record<string, string | null>, vim: boolean) {
  const clean: Partial<Record<ActionId, string | null>> = {};
  for (const [id, keys] of Object.entries(overrides)) {
    if (id in ACTIONS) clean[id as ActionId] = keys;
  }
  useShortcutBindings.setState({ overrides: clean, vim });
}

/** The keys for an action now, or null if it has none. */
export function shortcutFor(id: ActionId): string | null {
  const { overrides } = useShortcutBindings.getState();
  return id in overrides ? (overrides[id] ?? null) : DEFAULT_SHORTCUTS[id];
}

/** Like `shortcutFor`, but re-renders when the keys change. */
export function useShortcutKeys(id: ActionId): string | null {
  return useShortcutBindings((s) =>
    id in s.overrides ? (s.overrides[id] ?? null) : DEFAULT_SHORTCUTS[id],
  );
}

let parsed = new Map<ActionId, ParsedShortcut[][]>();

function rebuild() {
  const next = new Map<ActionId, ParsedShortcut[][]>();
  const add = (id: ActionId, keys: string | null) => {
    if (!keys) return;
    try {
      next.set(id, [...(next.get(id) ?? []), parseSequence(keys, platform)]);
    } catch {
      /* an unreadable saved shortcut is ignored */
    }
  };
  for (const id of ACTION_IDS) add(id, shortcutFor(id));
  if (useShortcutBindings.getState().vim) for (const [id, keys] of VIM_KEYS) add(id, keys);
  parsed = next;
}
rebuild();
useShortcutBindings.subscribe(rebuild);

function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return (
    target.isContentEditable ||
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement
  );
}

/**
 * Inside a dialog, menu or pop-up list, keys belong to it (Enter, Delete,
 * arrows…). Lists of books and tags are marked `data-shortcuts` so the app's
 * keys keep working there.
 */
function inOverlay(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLElement &&
    target.closest(
      "[role=dialog], [role=alertdialog], [role=menu], [role=listbox]:not([data-shortcuts])",
    ) !== null
  );
}

interface Entry {
  handler: () => void;
  /** Registered inside a `ShortcutScope` (a view), not app-wide. */
  scoped: boolean;
  active: () => boolean;
}

/** Newest registration last. */
const handlers = new Map<ActionId, Entry[]>();

/** Keys pressed so far of a multi-key sequence ("g" of "g g"). */
let pendingKeys: KeyboardEvent[] = [];
let pendingTimer: ReturnType<typeof setTimeout> | undefined;

function resetPending() {
  pendingKeys = [];
  clearTimeout(pendingTimer);
}

function activeEntry(id: ActionId): { entry: Entry; scoped: boolean } | null {
  const entries = (handlers.get(id) ?? []).filter((e) => e.active()).reverse();
  const scoped = entries.find((e) => e.scoped);
  if (scoped) return { entry: scoped, scoped: true };
  const global = entries.find((e) => !e.scoped);
  return global ? { entry: global, scoped: false } : null;
}

function onKeyDown(event: KeyboardEvent) {
  if (event.defaultPrevented || event.isComposing) return;
  const typing = isTyping(event.target);
  const overlay = inOverlay(event.target);
  const allowed = (id: ActionId) =>
    ("whileTyping" in ACTIONS[id] && ACTIONS[id].whileTyping) || (!typing && !overlay);

  const steps = [...pendingKeys, event];
  let full: { entry: Entry; scoped: boolean } | null = null;
  let partial = false;
  for (const [id, sequences] of parsed) {
    if (!allowed(id)) continue;
    for (const seq of sequences) {
      if (seq.length < steps.length) continue;
      const ok = steps.every((e, i) => matches(e, seq[i]!));
      if (!ok) continue;
      const found = activeEntry(id);
      if (!found) continue;
      if (seq.length === steps.length) {
        // The view the user is looking at wins over app-wide actions.
        if (!full || (found.scoped && !full.scoped)) full = found;
      } else {
        partial = true;
      }
    }
  }
  if (full) {
    resetPending();
    event.preventDefault();
    full.entry.handler();
    return;
  }
  if (partial) {
    event.preventDefault();
    pendingKeys = steps;
    clearTimeout(pendingTimer);
    pendingTimer = setTimeout(resetPending, 1000);
    return;
  }
  resetPending();
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
  const handlerRef = useRef(handler);
  useEffect(() => {
    activeRef.current = scope ?? true;
    handlerRef.current = handler;
  });
  useEffect(() => {
    if (!listening) {
      window.addEventListener("keydown", onKeyDown);
      listening = true;
    }
    const entry: Entry = {
      handler: () => handlerRef.current(),
      scoped: scope !== null,
      active: () => activeRef.current,
    };
    handlers.set(id, [...(handlers.get(id) ?? []), entry]);
    return () => {
      handlers.set(
        id,
        (handlers.get(id) ?? []).filter((e) => e !== entry),
      );
    };
  }, [id, scope]);
}

/** Runs an action as if its keys were pressed (command palette, menus). */
export function runAction(id: ActionId): boolean {
  const found = activeEntry(id);
  found?.entry.handler();
  return found !== null;
}
