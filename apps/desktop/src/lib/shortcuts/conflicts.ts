import { ACTION_IDS, scopeOf, type ActionId } from "./actions";
import { displayKeys, parseSequence, type Platform } from "./keys";

/** "Ctrl+Shift+K" or "⇧⌘K", for tooltips and menus. */
export function keysLabel(keys: string | null, platform: Platform): string {
  if (!keys) return "";
  return displayKeys(keys, platform)
    .join(platform === "mac" ? "" : "+")
    .replace(/\+then\+/g, " then ");
}

function canonical(keys: string, platform: Platform): string {
  return JSON.stringify(parseSequence(keys, platform));
}

/**
 * Actions whose keys clash with `id` using `keys`: the same keys in the same
 * part of the app, or anywhere for app-wide actions.
 */
export function findConflicts(
  id: ActionId,
  keys: string,
  keysOf: (id: ActionId) => string | null,
  platform: Platform,
): ActionId[] {
  let mine: string;
  try {
    mine = canonical(keys, platform);
  } catch {
    return [];
  }
  const scope = scopeOf(id);
  return ACTION_IDS.filter((other) => {
    if (other === id) return false;
    const k = keysOf(other);
    if (!k) return false;
    let theirs: string;
    try {
      theirs = canonical(k, platform);
    } catch {
      return false;
    }
    if (theirs !== mine) return false;
    const s = scopeOf(other);
    return s === scope || s === "global" || scope === "global";
  });
}
