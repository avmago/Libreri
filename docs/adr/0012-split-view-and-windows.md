# 12. Split view and book windows

Status: accepted.

**Split view** shows two book tabs side by side in one window (Mod+\\ splits the current book with the previous tab; "Open beside the current book" in the book menu). The side you click or switch to with Mod+Shift+\\ gets the keyboard. The split is saved with the tabs.

**Book windows.** Dragging a tab out of the tab strip, or Mod+Shift+N, opens the book in a window of its own. The window is another webview of the same app (`libreri-<id>` labels, same permissions as the main window), so it shares the open library, the signed-in profile and the Rust state. Details:

- The main window owns the saved tabs. A book window shows only its book and saves nothing but reading positions, so two windows never overwrite each other's tabs.
- Signing in or locking in any window sends `SessionChanged`; every window follows, so locking locks them all.
- Closing a book window's last tab closes the window. The library closes cleanly when the last window closes, not when any window closes.
- Mod+N opens a new window on the library.
