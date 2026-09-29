import { useEffect, useRef } from "react";
import { commands, unwrap } from "@/lib/ipc";
import { useTabs, type BookTab, type Split } from "@/lib/tabs";
import { initialTab, isMainWindow } from "@/lib/windows";
import { DEFAULT_PREFS, useReaderPrefs, type ReaderPrefs } from "../prefs";

interface Session {
  version: 1;
  tabs: Omit<BookTab, "jumpTo" | "findText" | "compare">[];
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

/** The tabs whose books are still in the library (a book that cannot be
 * looked up for another reason keeps its tab, which then says so). */
async function openable(tabs: BookTab[]): Promise<BookTab[]> {
  const found = await Promise.all(
    tabs.map((t) =>
      commands
        .getBook(t.bookId)
        .then((r) => r.status === "ok" || r.error.kind !== "notFound")
        .catch(() => true),
    ),
  );
  return tabs.filter((_, i) => found[i]);
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
    void (async () => {
      let json: string | null;
      try {
        json = await unwrap(commands.getSession());
      } catch {
        // Not read: leave the saved session alone (nothing is saved over it
        // until the library is opened again).
        return;
      }
      if (cancelled) return;
      let s: Partial<Session> = {};
      try {
        const v: unknown = json ? JSON.parse(json) : null;
        if (v && typeof v === "object") s = v as Partial<Session>;
      } catch {
        // Damaged: start afresh; the next change saves a good one.
      }
      useReaderPrefs.getState().set({ ...DEFAULT_PREFS, ...s.prefs });
      if (initialTab) useTabs.getState().restore([initialTab], initialTab.bookId);
      else if (isMainWindow) {
        const tabs = await openable(Array.isArray(s.tabs) ? s.tabs : []);
        if (cancelled) return;
        useTabs.getState().restore(tabs, s.active ?? null, s.split);
      }
      restored.current = libraryId;
    })();
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
