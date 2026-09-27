import { describe, expect, it } from "vitest";
import { ACTION_IDS, ACTIONS, type ActionId } from "./actions";
import { findConflicts } from "./conflicts";

const defaults = (id: ActionId) => ACTIONS[id].keys;

describe("shortcut defaults", () => {
  it("has more than 90 actions", () => {
    expect(ACTION_IDS.length).toBeGreaterThan(90);
  });

  it("never clash with each other", () => {
    for (const id of ACTION_IDS) {
      const k = defaults(id);
      if (!k) continue;
      for (const platform of ["mac", "other"] as const) {
        expect([id, findConflicts(id, k, defaults, platform)]).toEqual([id, []]);
      }
    }
  });

  it("avoid Super and common Linux desktop keys", () => {
    for (const id of ACTION_IDS) {
      const k = defaults(id) ?? "";
      expect(k).not.toMatch(/Meta|Super/);
      expect(k).not.toMatch(/^Ctrl\+Alt\+(T|Arrow)/);
      expect(k).not.toMatch(/^Alt\+F\d/);
    }
  });

  it("finds clashes in the same place", () => {
    expect(findConflicts("books.favorite", "Mod+A", defaults, "other")).toEqual([
      "books.selectAll",
    ]);
    expect(findConflicts("reader.bookmark", "Mod+A", defaults, "other")).toEqual([]);
  });
});
