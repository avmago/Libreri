import { useEffect, useRef } from "react";
import { commands, unwrap } from "@/lib/ipc";
import { useTabs, type BookTab, type Split } from "@/lib/tabs";
import { initialTab, isMainWindow } from "@/lib/windows";
import { DEFAULT_PREFS, useReaderPrefs, type ReaderPrefs } from "../prefs";

interface Session {
  version: 1;
  tabs: Omit<BookTab, "jumpTo">[];
  active: string | null;
  split?: Split | null;
  prefs: ReaderPrefs;
}

/** A save waiting for its debounce, run early by `flushSession`. */
let pending: (() => Promise<unknown>) | null = null;

/** Saves the open tabs now if a save is waiting (before signing out). */
export async function flushSession(): Promise<void> {
  const run = pending;
  pending = null;
  if (run) await run();
}

/**
 * Restores the open tabs and page settings when a library opens, and saves
 * them (per library and profile) whenever they change. Only the main window
 * does this; a window opened for one book shows just that book.
 */
export function useSession(libraryId: string) {
  const restored = useRef<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    restored.current = null;
    useTabs.getState().restore([], null);
    void unwrap(commands.getSession())
      .then((json) => {
        if (cancelled) return;
        const s = json ? (JSON.parse(json) as Partial<Session>) : {};
        useReaderPrefs.getState().set({ ...DEFAULT_PREFS, ...s.prefs });
        if (initialTab) useTabs.getState().restore([initialTab], initialTab.bookId);
        else if (isMainWindow) useTabs.getState().restore(s.tabs ?? [], s.active ?? null, s.split);
      })
      .catch(() => {})
      .finally(() => {
        if (!cancelled) restored.current = libraryId;
      });
    return () => {
      cancelled = true;
    };
  }, [libraryId]);

  useEffect(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    const save = () => {
      if (restored.current !== libraryId || !isMainWindow) return;
      clearTimeout(timer);
      pending = () => {
        clearTimeout(timer);
        pending = null;
        const { tabs, active, split } = useTabs.getState();
        const { theme, followApp, pdfMode } = useReaderPrefs.getState();
        const session: Session = {
          version: 1,
          tabs: tabs.map(({ bookId, title, fileType }) => ({ bookId, title, fileType })),
          active,
          split,
          prefs: { theme, followApp, pdfMode },
        };
        return commands.saveSession(JSON.stringify(session));
      };
      timer = setTimeout(() => void pending?.(), 400);
    };
    const offTabs = useTabs.subscribe(save);
    const offPrefs = useReaderPrefs.subscribe(save);
    return () => {
      clearTimeout(timer);
      pending = null;
      offTabs();
      offPrefs();
    };
  }, [libraryId]);
}
