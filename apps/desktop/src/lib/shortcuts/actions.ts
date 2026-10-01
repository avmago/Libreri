/**
 * Every keyboard action in Libreri, with its default keys.
 *
 * Defaults use Ctrl/Alt/Shift + letters and avoid the Super key and common
 * Linux desktop bindings (Ctrl+Alt+T, Ctrl+Alt+arrows, Alt+F-keys). `Mod`
 * is Ctrl on Linux and Windows and ⌘ on macOS. Each profile can change any
 * of them in Settings › Shortcuts.
 */

export type ActionGroup =
  | "General"
  | "Tabs and windows"
  | "Library"
  | "Books"
  | "Reader"
  | "Highlights and notes"
  | "Notes hub"
  | "Organize";

export interface ActionDef {
  label: string;
  group: ActionGroup;
  /** `null` = no key by default (can still be assigned). */
  keys: string | null;
  /** Still works while typing in a text field. */
  whileTyping?: boolean;
}

const rateLabel = (n: number) => (n === 0 ? "Remove rating" : `Rate ${n} star${n > 1 ? "s" : ""}`);

export const ACTIONS = {
  // General
  "palette.open": {
    label: "Open the command palette",
    group: "General",
    keys: "Mod+K",
    whileTyping: true,
  },
  "settings.open": { label: "Open Settings", group: "General", keys: "Mod+,", whileTyping: true },
  "shortcuts.show": {
    label: "Show keyboard shortcuts",
    group: "General",
    keys: "Mod+/",
    whileTyping: true,
  },
  "theme.toggle": {
    label: "Switch light / dark theme",
    group: "General",
    keys: "Mod+Shift+L",
    whileTyping: true,
  },
  "profile.switch": {
    label: "Lock or switch profile",
    group: "General",
    keys: "Mod+Shift+P",
    whileTyping: true,
  },
  "library.close": {
    label: "Close the library",
    group: "General",
    keys: "Mod+Shift+W",
    whileTyping: true,
  },
  "go.library": {
    label: "Go to the library",
    group: "General",
    keys: "Mod+Shift+1",
    whileTyping: true,
  },
  "go.notes": {
    label: "Go to the Notes hub",
    group: "General",
    keys: "Mod+Shift+2",
    whileTyping: true,
  },
  "go.feeds": {
    label: "Go to Feeds",
    group: "General",
    keys: "Mod+Shift+0",
    whileTyping: true,
  },
  "go.podcasts": {
    label: "Go to Podcasts",
    group: "General",
    keys: "Mod+Shift+9",
    whileTyping: true,
  },
  "go.search": {
    label: "Search inside books, details and notes",
    group: "General",
    keys: "Mod+Alt+F",
    whileTyping: true,
  },
  "library.health": { label: "Check library health", group: "General", keys: null },
  "library.backup": { label: "Back up the library now", group: "General", keys: null },
  "library.importArchive": {
    label: "Import a Libreri archive",
    group: "General",
    keys: null,
  },
  "library.importForeign": {
    label: "Import from Calibre, Zotero, Goodreads…",
    group: "General",
    keys: null,
  },
  "go.organize": {
    label: "Go to Organize",
    group: "General",
    keys: "Mod+Shift+3",
    whileTyping: true,
  },
  "app.fullscreen": { label: "Full screen", group: "General", keys: "F11", whileTyping: true },

  // Tabs and windows
  "tabs.close": { label: "Close tab", group: "Tabs and windows", keys: "Mod+W", whileTyping: true },
  "tabs.reopen": {
    label: "Reopen closed tab",
    group: "Tabs and windows",
    keys: "Mod+Shift+T",
    whileTyping: true,
  },
  "tabs.next": {
    label: "Next tab",
    group: "Tabs and windows",
    keys: "Ctrl+Tab",
    whileTyping: true,
  },
  "tabs.previous": {
    label: "Previous tab",
    group: "Tabs and windows",
    keys: "Ctrl+Shift+Tab",
    whileTyping: true,
  },
  "tabs.goto1": { label: "Go to tab 1 (library)", group: "Tabs and windows", keys: "Mod+Alt+1" },
  "tabs.goto2": { label: "Go to tab 2", group: "Tabs and windows", keys: "Mod+Alt+2" },
  "tabs.goto3": { label: "Go to tab 3", group: "Tabs and windows", keys: "Mod+Alt+3" },
  "tabs.goto4": { label: "Go to tab 4", group: "Tabs and windows", keys: "Mod+Alt+4" },
  "tabs.goto5": { label: "Go to tab 5", group: "Tabs and windows", keys: "Mod+Alt+5" },
  "tabs.goto6": { label: "Go to tab 6", group: "Tabs and windows", keys: "Mod+Alt+6" },
  "tabs.goto7": { label: "Go to tab 7", group: "Tabs and windows", keys: "Mod+Alt+7" },
  "tabs.goto8": { label: "Go to tab 8", group: "Tabs and windows", keys: "Mod+Alt+8" },
  "tabs.gotoLast": { label: "Go to the last tab", group: "Tabs and windows", keys: "Mod+Alt+9" },
  "tabs.split": {
    label: "Split view: show another book beside this one",
    group: "Tabs and windows",
    keys: "Mod+\\",
  },
  "tabs.splitFocus": {
    label: "Split view: switch side",
    group: "Tabs and windows",
    keys: "Mod+Shift+\\",
  },
  "tabs.newWindow": {
    label: "Move tab to a new window",
    group: "Tabs and windows",
    keys: "Mod+Shift+N",
  },
  "window.new": { label: "New window", group: "Tabs and windows", keys: "Mod+N" },

  // Library
  "sidebar.toggle": {
    label: "Show or hide the sidebar",
    group: "Library",
    keys: "Mod+B",
    whileTyping: true,
  },
  "library.search": {
    label: "Search the library",
    group: "Library",
    keys: "Mod+F",
    whileTyping: true,
  },
  "library.import": { label: "Import files", group: "Library", keys: "Mod+O" },
  "library.importFolder": { label: "Import a folder", group: "Library", keys: "Mod+Shift+O" },
  "library.newFolder": { label: "New folder", group: "Library", keys: "Mod+Alt+N" },
  "library.refresh": {
    label: "Check the library folder for changes",
    group: "Library",
    keys: "F5",
  },
  "view.grid": { label: "Show as grid", group: "Library", keys: "Mod+1" },
  "view.list": { label: "Show as list", group: "Library", keys: "Mod+2" },
  "view.shelf": { label: "Show as shelves", group: "Library", keys: "Mod+3" },
  "details.toggle": { label: "Show or hide details", group: "Library", keys: "Mod+I" },
  "details.edit": { label: "Edit details", group: "Library", keys: "Mod+E" },
  "details.save": { label: "Save details", group: "Library", keys: "Mod+S", whileTyping: true },
  "details.find": { label: "Find details online", group: "Library", keys: "Mod+Shift+D" },
  "details.fill": { label: "Fill in missing details online", group: "Library", keys: null },
  "filters.clear": { label: "Clear search and filters", group: "Library", keys: "Mod+Shift+X" },
  "library.export": {
    label: "Export the selected books (or the library)",
    group: "Library",
    keys: "Mod+Alt+E",
  },
  "collection.save": {
    label: "Save this search as a smart collection",
    group: "Library",
    keys: "Mod+Shift+S",
  },
  "sort.title": { label: "Sort by title", group: "Library", keys: null },
  "sort.author": { label: "Sort by author", group: "Library", keys: null },
  "sort.added": { label: "Sort by date added", group: "Library", keys: null },
  "sort.lastOpened": { label: "Sort by last opened", group: "Library", keys: null },

  // Books (the selection in the library)
  "books.open": { label: "Open", group: "Books", keys: "Enter" },
  "books.selectAll": { label: "Select all", group: "Books", keys: "Mod+A" },
  "books.clearSelection": { label: "Clear the selection", group: "Books", keys: "Escape" },
  "books.next": { label: "Next book", group: "Books", keys: "ArrowRight" },
  "books.previous": { label: "Previous book", group: "Books", keys: "ArrowLeft" },
  "books.down": { label: "Book below", group: "Books", keys: "ArrowDown" },
  "books.up": { label: "Book above", group: "Books", keys: "ArrowUp" },
  "books.first": { label: "First book", group: "Books", keys: "Home" },
  "books.last": { label: "Last book", group: "Books", keys: "End" },
  "books.bulkEdit": {
    label: "Edit the selected books together",
    group: "Books",
    keys: "Mod+Shift+E",
  },
  "books.reveal": { label: "Show the file", group: "Books", keys: "Mod+Shift+R" },
  "books.trash": { label: "Move to the trash", group: "Books", keys: "Delete" },
  "books.trashMac": { label: "Move to the trash (Mac)", group: "Books", keys: "Mod+Backspace" },
  "books.favorite": { label: "Favourite", group: "Books", keys: "Mod+D" },
  "books.cite": { label: "Cite the selected books", group: "Books", keys: "Mod+Alt+C" },
  "books.rate0": { label: rateLabel(0), group: "Books", keys: "Alt+0" },
  "books.rate1": { label: rateLabel(1), group: "Books", keys: "Alt+1" },
  "books.rate2": { label: rateLabel(2), group: "Books", keys: "Alt+2" },
  "books.rate3": { label: rateLabel(3), group: "Books", keys: "Alt+3" },
  "books.rate4": { label: rateLabel(4), group: "Books", keys: "Alt+4" },
  "books.rate5": { label: rateLabel(5), group: "Books", keys: "Alt+5" },
  "books.markWantToRead": { label: "Mark as Want to Read", group: "Books", keys: "Alt+W" },
  "books.markReading": { label: "Mark as Reading", group: "Books", keys: "Alt+R" },
  "books.markFinished": { label: "Mark as Finished", group: "Books", keys: "Alt+F" },
  "books.markNone": { label: "Clear the reading status", group: "Books", keys: "Alt+N" },

  // Reader
  "reader.next": { label: "Next page", group: "Reader", keys: "ArrowRight" },
  "reader.previous": { label: "Previous page", group: "Reader", keys: "ArrowLeft" },
  "reader.pageDown": { label: "Page down", group: "Reader", keys: "PageDown" },
  "reader.pageUp": { label: "Page up", group: "Reader", keys: "PageUp" },
  "reader.space": { label: "Next page (Space)", group: "Reader", keys: "Space" },
  "reader.spaceBack": {
    label: "Previous page (Shift+Space)",
    group: "Reader",
    keys: "Shift+Space",
  },
  "reader.scrollDown": { label: "Scroll down a little", group: "Reader", keys: "ArrowDown" },
  "reader.scrollUp": { label: "Scroll up a little", group: "Reader", keys: "ArrowUp" },
  "reader.start": { label: "Go to the start", group: "Reader", keys: "Home" },
  "reader.end": { label: "Go to the end", group: "Reader", keys: "End" },
  "reader.nextChapter": { label: "Next chapter", group: "Reader", keys: "Mod+ArrowDown" },
  "reader.previousChapter": { label: "Previous chapter", group: "Reader", keys: "Mod+ArrowUp" },
  "reader.back": { label: "Back (after following a link)", group: "Reader", keys: "Alt+ArrowLeft" },
  "reader.forward": { label: "Forward", group: "Reader", keys: "Alt+ArrowRight" },
  "reader.goToPage": { label: "Go to page", group: "Reader", keys: "Mod+G" },
  "reader.find": { label: "Find in book", group: "Reader", keys: "Mod+F", whileTyping: true },
  "reader.findNext": { label: "Find next", group: "Reader", keys: "F3", whileTyping: true },
  "reader.findPrevious": {
    label: "Find previous",
    group: "Reader",
    keys: "Shift+F3",
    whileTyping: true,
  },
  "reader.zoomIn": { label: "Zoom in / larger text", group: "Reader", keys: "Mod+=" },
  "reader.zoomOut": { label: "Zoom out / smaller text", group: "Reader", keys: "Mod+-" },
  "reader.zoomReset": { label: "Normal size", group: "Reader", keys: "Mod+0" },
  "reader.fitWidth": { label: "Fit page width (PDF)", group: "Reader", keys: "Mod+8" },
  "reader.fitPage": { label: "Fit whole page (PDF)", group: "Reader", keys: "Mod+9" },
  "reader.contents": { label: "Show or hide contents and marks", group: "Reader", keys: "Mod+B" },
  "reader.notebook": {
    label: "Show or hide the notebook",
    group: "Reader",
    keys: "Mod+J",
    whileTyping: true,
  },
  "reader.themeNext": { label: "Next page theme", group: "Reader", keys: "Alt+T" },
  "reader.pdfModeNext": { label: "Next dark-page mode (PDF)", group: "Reader", keys: "Alt+D" },
  "reader.focusMode": {
    label: "Focus mode (hide the toolbar)",
    group: "Reader",
    keys: "Mod+Shift+F",
  },
  "reader.details": { label: "Book details", group: "Reader", keys: "Mod+I" },
  "reader.markup": {
    label: "Markup mode: draw and write on the pages (PDF, DjVu, comics)",
    group: "Reader",
    keys: "Alt+M",
  },
  "reader.readAloud": {
    label: "Read aloud from here, or stop",
    group: "Reader",
    keys: "Mod+Shift+U",
  },

  // Highlights and notes (in the reader)
  "reader.bookmark": { label: "Bookmark this page", group: "Highlights and notes", keys: "Mod+D" },
  "reader.highlight": {
    label: "Highlight the selected text",
    group: "Highlights and notes",
    keys: "Mod+Shift+H",
  },
  "reader.comment": {
    label: "Comment on the selected text",
    group: "Highlights and notes",
    keys: "Mod+Shift+M",
  },
  "reader.addLink": {
    label: "Link a web page, video or recording to the selection or page",
    group: "Highlights and notes",
    keys: "Alt+L",
  },
  "reader.addToNotebook": {
    label: "Quote the selection in the notebook",
    group: "Highlights and notes",
    keys: "Mod+Shift+K",
  },
  "reader.nextHighlight": {
    label: "Next highlight",
    group: "Highlights and notes",
    keys: "Alt+ArrowDown",
  },
  "reader.previousHighlight": {
    label: "Previous highlight",
    group: "Highlights and notes",
    keys: "Alt+ArrowUp",
  },

  // Notes hub
  "notes.search": { label: "Search notes", group: "Notes hub", keys: "Mod+F", whileTyping: true },
  "notes.new": { label: "New note", group: "Notes hub", keys: "Mod+Alt+N" },
  "notes.copyMarkdown": {
    label: "Copy the shown notes as Markdown",
    group: "Notes hub",
    keys: "Mod+Shift+C",
  },
  "notes.open": { label: "Open at the page", group: "Notes hub", keys: "Enter" },

  // Organize
  "organize.rename": {
    label: "Rename the selected tag or category",
    group: "Organize",
    keys: "F2",
  },
  "organize.merge": { label: "Merge the selected tags", group: "Organize", keys: "Mod+M" },
  "organize.delete": {
    label: "Remove the selected tag or category",
    group: "Organize",
    keys: "Delete",
  },
} as const satisfies Record<string, ActionDef>;

export type ActionId = keyof typeof ACTIONS;

export const ACTION_IDS = Object.keys(ACTIONS) as ActionId[];

export const GROUP_ORDER: ActionGroup[] = [
  "General",
  "Tabs and windows",
  "Library",
  "Books",
  "Reader",
  "Highlights and notes",
  "Notes hub",
  "Organize",
];

/**
 * Optional Vim-style keys, added on top of the normal ones while enabled.
 * A space separates the keys of a sequence ("g g").
 */
export const VIM_KEYS: [ActionId, string][] = [
  ["books.down", "j"],
  ["books.up", "k"],
  ["books.previous", "h"],
  ["books.next", "l"],
  ["books.first", "g g"],
  ["books.last", "Shift+G"],
  ["books.open", "o"],
  ["library.search", "/"],
  ["reader.scrollDown", "j"],
  ["reader.scrollUp", "k"],
  ["reader.previous", "h"],
  ["reader.next", "l"],
  ["reader.start", "g g"],
  ["reader.end", "Shift+G"],
  ["reader.find", "/"],
  ["reader.findNext", "n"],
  ["reader.findPrevious", "Shift+N"],
  ["notes.search", "/"],
];

/** Scopes whose actions can share keys without clashing. */
export function scopeOf(id: ActionId): "global" | "library" | "reader" | "notes" | "organize" {
  const g = ACTIONS[id].group;
  if (g === "Reader" || g === "Highlights and notes") return "reader";
  if (g === "Library" || g === "Books") return "library";
  if (g === "Notes hub") return "notes";
  if (g === "Organize") return "organize";
  return "global";
}
