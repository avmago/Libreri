/**
 * Full-screen reading: the window fills the screen and only the
 * page shows. The reader brings its toolbar and page bar back at the top
 * and bottom edges; Esc, F11 or the button on the page leave.
 *
 * The window's own full screen (F11, Ctrl+Cmd+F or the green button on
 * macOS) and this mode go together while a book is open: whichever changes
 * first, the other follows.
 */
import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { create } from "zustand";

interface State {
  /** Reading full screen (only the page shows). */
  reading: boolean;
  set: (reading: boolean) => void;
}

export const useFullscreen = create<State>((set) => ({
  reading: false,
  set: (reading) => set({ reading }),
}));

const inTauri = () => typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

/** Sets the window's full screen, ignoring places without a window (tests). */
async function windowFullscreen(on: boolean): Promise<void> {
  if (!inTauri()) return;
  try {
    const w = getCurrentWindow();
    if ((await w.isFullscreen()) !== on) await w.setFullscreen(on);
  } catch {
    /* the reading view still changes */
  }
}

/** Starts or ends full-screen reading. */
export function setReadingFullscreen(on: boolean): void {
  useFullscreen.getState().set(on);
  void windowFullscreen(on);
}

export function toggleReadingFullscreen(): void {
  setReadingFullscreen(!useFullscreen.getState().reading);
}

/** Whether a book is the tab shown (read by the window listener). */
const shown = { current: false };

/**
 * Keeps full-screen reading in step with the window: leaving the window's
 * full screen (the green button, Ctrl+Cmd+F) ends it, and entering it while
 * a book is shown starts it. Mount once per window.
 */
export function useFullscreenSync(bookShown: boolean): void {
  useEffect(() => {
    if (!inTauri()) return;
    const w = getCurrentWindow();
    let last: boolean | null = null;
    const check = () =>
      void w
        .isFullscreen()
        .then((full) => {
          if (full === last) return;
          last = full;
          const { reading, set } = useFullscreen.getState();
          if (!full && reading) set(false);
          else if (full && !reading && shown.current) set(true);
        })
        .catch(() => {});
    const off = w.onResized(check);
    check();
    return () => void off.then((f) => f());
  }, []);
  // Leaving the book (to the library) ends full-screen reading.
  useEffect(() => {
    shown.current = bookShown;
    if (!bookShown && useFullscreen.getState().reading) setReadingFullscreen(false);
  }, [bookShown]);
}
