import { useMemo } from "react";
import { create } from "zustand";
import type { BookQuery, ContentType, FileType, ReadingStatus, SortKey } from "@/lib/ipc";

/** What the main area shows: a list of books, the Notes hub or Organize. */
export type Nav =
  | { kind: "all" }
  | { kind: "status"; status: ReadingStatus }
  | { kind: "favorites" }
  | { kind: "audio" }
  | { kind: "missing" }
  | { kind: "folder"; path: string }
  | { kind: "tag"; tag: string }
  | { kind: "category"; path: string }
  | { kind: "collection"; id: string; name: string; query: BookQuery }
  | { kind: "notes" }
  | { kind: "feeds" }
  | { kind: "podcasts" }
  | { kind: "search"; query?: string }
  | { kind: "organize" };

export type ViewMode = "grid" | "list" | "shelf";

interface LibraryViewState {
  nav: Nav;
  search: string;
  fileTypes: FileType[];
  contentTypes: ContentType[];
  tags: string[];
  sort: SortKey;
  descending: boolean;
  view: ViewMode;
  includeSubfolders: boolean;
  /** Selected book ids, in the order they were selected. */
  selection: string[];
  /** Where a Shift+click range starts. */
  anchor: string | null;
  detailsOpen: boolean;
  expanded: Record<string, boolean>;

  setNav: (nav: Nav) => void;
  setSearch: (search: string) => void;
  toggleFileType: (t: FileType) => void;
  toggleContentType: (t: ContentType) => void;
  toggleTag: (t: string) => void;
  clearFilters: () => void;
  setSort: (sort: SortKey) => void;
  setDescending: (d: boolean) => void;
  setView: (v: ViewMode) => void;
  setIncludeSubfolders: (v: boolean) => void;
  /** `ordered` is the list as displayed, for Shift+click ranges. */
  select: (id: string, mode: "replace" | "toggle" | "range", ordered: string[]) => void;
  setSelection: (ids: string[]) => void;
  toggleDetails: () => void;
  setDetailsOpen: (open: boolean) => void;
  setExpanded: (path: string, open: boolean) => void;
  /** A folder was renamed or moved: the view follows it (and its subfolders). */
  folderMoved: (from: string, to: string) => void;
}

const toggle = <T>(list: T[], item: T) =>
  list.includes(item) ? list.filter((x) => x !== item) : [...list, item];

/** Sorting that makes sense by default in each direction. */
const DESCENDING_BY_DEFAULT: SortKey[] = ["added", "year", "size", "pages", "lastOpened", "rating"];

export const useLibraryView = create<LibraryViewState>((set) => ({
  nav: { kind: "all" },
  search: "",
  fileTypes: [],
  contentTypes: [],
  tags: [],
  sort: "title",
  descending: false,
  view: "grid",
  includeSubfolders: false,
  selection: [],
  anchor: null,
  detailsOpen: true,
  expanded: {},

  setNav: (nav) => set({ nav, selection: [], anchor: null, search: "" }),
  setSearch: (search) => set({ search }),
  toggleFileType: (t) => set((s) => ({ fileTypes: toggle(s.fileTypes, t) })),
  toggleContentType: (t) => set((s) => ({ contentTypes: toggle(s.contentTypes, t) })),
  toggleTag: (t) => set((s) => ({ tags: toggle(s.tags, t) })),
  clearFilters: () => set({ fileTypes: [], contentTypes: [], tags: [], search: "" }),
  setSort: (sort) =>
    set((s) =>
      s.sort === sort
        ? { descending: !s.descending }
        : { sort, descending: DESCENDING_BY_DEFAULT.includes(sort) },
    ),
  setDescending: (descending) => set({ descending }),
  setView: (view) => set({ view }),
  setIncludeSubfolders: (includeSubfolders) => set({ includeSubfolders }),
  select: (id, mode, ordered) =>
    set((s) => {
      if (mode === "toggle") {
        return { selection: toggle(s.selection, id), anchor: id };
      }
      if (mode === "range" && s.anchor && ordered.includes(s.anchor)) {
        const a = ordered.indexOf(s.anchor);
        const b = ordered.indexOf(id);
        const [from, to] = a < b ? [a, b] : [b, a];
        return { selection: ordered.slice(from, to + 1) };
      }
      return { selection: [id], anchor: id };
    }),
  setSelection: (selection) => set({ selection, anchor: selection[0] ?? null }),
  toggleDetails: () => set((s) => ({ detailsOpen: !s.detailsOpen })),
  setDetailsOpen: (detailsOpen) => set({ detailsOpen }),
  setExpanded: (path, open) => set((s) => ({ expanded: { ...s.expanded, [path]: open } })),
  folderMoved: (from, to) =>
    set((s) => {
      const follow = (path: string) =>
        path === from ? to : path.startsWith(`${from}/`) ? to + path.slice(from.length) : null;
      const moved = s.nav.kind === "folder" ? follow(s.nav.path) : null;
      const expanded: Record<string, boolean> = {};
      for (const [path, open] of Object.entries(s.expanded)) expanded[follow(path) ?? path] = open;
      return { expanded, ...(moved !== null && { nav: { kind: "folder" as const, path: moved } }) };
    }),
}));

/** The selected ids as a set, for checking thousands of books quickly. */
export function useSelectedIds(): Set<string> {
  const selection = useLibraryView((s) => s.selection);
  return useMemo(() => new Set(selection), [selection]);
}

type QueryInput = Pick<
  LibraryViewState,
  | "nav"
  | "search"
  | "fileTypes"
  | "contentTypes"
  | "tags"
  | "sort"
  | "descending"
  | "includeSubfolders"
>;

/** The Rust query for what the main area shows. */
export function buildQuery(s: QueryInput): BookQuery {
  const q: BookQuery = {
    search: s.search.trim() || null,
    fileTypes: s.fileTypes,
    contentTypes: s.contentTypes,
    tags: s.tags,
    sort: s.sort,
    descending: s.descending,
  };
  switch (s.nav.kind) {
    case "collection": {
      // The saved search, with what is typed now and the chosen order.
      const saved = s.nav.query;
      return {
        ...saved,
        search: q.search ?? saved.search ?? null,
        fileTypes: [...(saved.fileTypes ?? []), ...q.fileTypes!],
        contentTypes: [...(saved.contentTypes ?? []), ...q.contentTypes!],
        tags: [...(saved.tags ?? []), ...q.tags!],
        sort: s.sort,
        descending: s.descending,
      };
    }
    case "tag":
      q.tags = [...q.tags!, s.nav.tag];
      break;
    case "category":
      q.category = s.nav.path;
      break;
    case "folder":
      q.folder = s.nav.path;
      // Searching inside a folder looks in its subfolders too.
      q.includeSubfolders = s.includeSubfolders || q.search !== null;
      break;
    case "status":
      q.status = s.nav.status;
      break;
    case "favorites":
      q.favoritesOnly = true;
      break;
    case "audio":
      q.audio = true;
      break;
    case "missing":
      q.missingOnly = true;
      break;
  }
  return q;
}

export function navTitle(nav: Nav): string {
  switch (nav.kind) {
    case "tag":
      return `#${nav.tag}`;
    case "category":
      return nav.path.split("/").join(" › ");
    case "collection":
      return nav.name;
    case "notes":
      return "Notes";
    case "feeds":
      return "Feeds";
    case "podcasts":
      return "Podcasts";
    case "organize":
      return "Organize";
    case "search":
      return "Search";
    case "all":
      return "All Books";
    case "favorites":
      return "Favourites";
    case "audio":
      return "Audiobooks";
    case "missing":
      return "Missing files";
    case "folder":
      return nav.path === "" ? "Books" : nav.path.slice(nav.path.lastIndexOf("/") + 1);
    case "status":
      return nav.status === "reading"
        ? "Currently Reading"
        : nav.status === "wantToRead"
          ? "Want to Read"
          : nav.status === "finished"
            ? "Finished"
            : "Stopped reading";
  }
}
