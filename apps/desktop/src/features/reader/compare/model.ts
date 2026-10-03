/**
 * Compare: helpers for showing a comparison. The comparing is
 * done in Rust; this file only groups and describes the result.
 */
import type { Change, ChangeKind, PagePair } from "@/lib/ipc";

export type Box = [number, number, number, number];

export const KIND_LABEL: Record<ChangeKind, string> = {
  removed: "Removed",
  added: "Added",
  changed: "Changed",
  look: "Looks different",
  pageRemoved: "Page removed",
  pageAdded: "Page added",
};

/** Colours of the marks (Tailwind-like, readable in light and dark). */
export const KIND_COLOUR: Record<ChangeKind, string> = {
  removed: "#dc2626",
  added: "#16a34a",
  changed: "#d97706",
  look: "#2563eb",
  pageRemoved: "#dc2626",
  pageAdded: "#16a34a",
};

/** The boxes of one side of a change (the first document is side "a"). */
export function rectsOf(c: Change, side: "a" | "b"): Box[] {
  return (side === "a" ? c.aRects : c.bRects) as Box[];
}

/** "p. 3", "p. 3 / 4", "– / p. 4". */
export function pairLabel(p: PagePair | undefined): string {
  if (!p) return "";
  if (p.a && p.b) return p.a === p.b ? `p. ${p.a}` : `p. ${p.a} / ${p.b}`;
  if (p.a) return `p. ${p.a} / –`;
  return `– / p. ${p.b}`;
}

export interface Counts {
  removed: number;
  added: number;
  changed: number;
  look: number;
  pages: number;
}

export function counts(changes: Change[]): Counts {
  const n = (k: ChangeKind[]) => changes.filter((c) => k.includes(c.kind)).length;
  return {
    removed: n(["removed"]),
    added: n(["added"]),
    changed: n(["changed"]),
    look: n(["look"]),
    pages: n(["pageAdded", "pageRemoved"]),
  };
}

/** Changes per page pair (index into `pairs`). */
export function byPair(changes: Change[]): Map<number, Change[]> {
  const out = new Map<number, Change[]>();
  for (const c of changes) {
    const list = out.get(c.pair) ?? [];
    list.push(c);
    out.set(c.pair, list);
  }
  return out;
}

/** The next change from `at` in direction `dir`, wrapping around. */
export function step(total: number, at: number | null, dir: 1 | -1): number | null {
  if (!total) return null;
  if (at === null) return dir > 0 ? 0 : total - 1;
  return (at + dir + total) % total;
}
