# 11. One registry for every keyboard shortcut

Status: accepted (2026-09-27)

Every keyboard action has an id, a label, a group and default keys in `lib/shortcuts/actions.ts` (111 actions in Phase 3). Components register handlers with `useShortcut(id, handler)`; nothing else listens for key presses, apart from typing fields and the PIN pad.

- **Defaults** use Ctrl/Alt/Shift with letters (`Mod` is ⌘ on macOS). They avoid the Super key and common Linux desktop keys (Ctrl+Alt+T, Ctrl+Alt+arrows, Alt+F-keys). A unit test checks that no two defaults clash and that none of these are used.
- **Scopes.** The library, each reader tab, the Notes hub and Organize are `ShortcutScope`s. The same keys may do different things in different places (Mod+F searches the library or finds in the book). The view that is showing wins over app-wide actions.
- **Physical keys.** When modifiers change the character (Alt+R is "®" on a Mac, Shift+1 is "!"), keys are matched by `event.code`, so shortcuts work on every keyboard layout that has the key.
- **Sequences** such as `g g` are supported (1 second between keys).
- **Rebinding.** Each profile's changes are stored in its preferences (`shortcuts`: id → keys or `null`). Settings › Shortcuts records a new combination, warns about clashes in the same place and offers to move the keys.
- **Vim keys** (optional): j/k, h/l, gg/G, / and n/N, added on top of the normal keys.
- **Cheat sheet.** Mod+/ shows every shortcut in use; it can be printed.
- Keys inside dialogs, menus and pop-up lists belong to them. Lists of books and tags are marked `data-shortcuts` so the app's keys still work there.
