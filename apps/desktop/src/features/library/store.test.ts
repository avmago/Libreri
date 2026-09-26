import { beforeEach, describe, expect, it } from "vitest";
import { buildQuery, navTitle, useLibraryView } from "./store";

const base = {
  search: "",
  fileTypes: [],
  contentTypes: [],
  tags: [],
  sort: "title" as const,
  descending: false,
  includeSubfolders: false,
};

describe("buildQuery", () => {
  it("maps each sidebar entry to a Rust query", () => {
    expect(buildQuery({ ...base, nav: { kind: "all" } }).folder).toBeUndefined();
    expect(buildQuery({ ...base, nav: { kind: "favorites" } }).favoritesOnly).toBe(true);
    expect(buildQuery({ ...base, nav: { kind: "status", status: "reading" } }).status).toBe(
      "reading",
    );
    const folder = buildQuery({ ...base, nav: { kind: "folder", path: "Science" } });
    expect(folder.folder).toBe("Science");
    expect(folder.includeSubfolders).toBe(false);
  });

  it("searches subfolders too when searching inside a folder", () => {
    const q = buildQuery({
      ...base,
      search: "  quantum ",
      nav: { kind: "folder", path: "Science" },
    });
    expect(q.search).toBe("quantum");
    expect(q.includeSubfolders).toBe(true);
  });

  it("names views for the header", () => {
    expect(navTitle({ kind: "folder", path: "Science/Physics" })).toBe("Physics");
    expect(navTitle({ kind: "status", status: "wantToRead" })).toBe("Want to Read");
  });
});

describe("selection", () => {
  beforeEach(() => useLibraryView.setState({ selection: [], anchor: null }));
  const ids = ["a", "b", "c", "d", "e"];

  it("supports click, toggle and shift ranges in display order", () => {
    const { select } = useLibraryView.getState();
    select("b", "replace", ids);
    select("d", "range", ids);
    expect(useLibraryView.getState().selection).toEqual(["b", "c", "d"]);
    select("a", "toggle", ids);
    expect(useLibraryView.getState().selection).toEqual(["b", "c", "d", "a"]);
    select("a", "range", ids);
    expect(useLibraryView.getState().selection).toEqual(["a"]);
  });

  it("flips the direction when the same sort is picked twice", () => {
    const s = useLibraryView.getState();
    s.setSort("added");
    expect(useLibraryView.getState().descending).toBe(true);
    useLibraryView.getState().setSort("added");
    expect(useLibraryView.getState().descending).toBe(false);
  });
});
