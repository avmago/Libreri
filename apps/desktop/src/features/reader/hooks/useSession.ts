import { useEffect, useRef } from "react";
import { commands, unwrap } from "@/lib/ipc";
import { useTabs, type BookTab } from "@/lib/tabs";
import { DEFAULT_PREFS, useReaderPrefs, type ReaderPrefs } from "../prefs";

interface Session {
  version: 1;
  tabs: Omit<BookTab, "jumpTo">[];
  active: string | null;
  prefs: ReaderPrefs;
}

/**
 * Restores the open tabs and page settings when a library opens, and saves
 * them (per library and profile) whenever they change.
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
        if (json) {
          const s = JSON.parse(json) as Partial<Session>;
          useReaderPrefs.getState().set({ ...DEFAULT_PREFS, ...s.prefs });
          useTabs.getState().restore(s.tabs ?? [], s.active ?? null);
        }
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
      if (restored.current !== libraryId) return;
      clearTimeout(timer);
      timer = setTimeout(() => {
        const { tabs, active } = useTabs.getState();
        const { theme, followApp, pdfMode } = useReaderPrefs.getState();
        const session: Session = {
          version: 1,
          tabs: tabs.map(({ bookId, title, fileType }) => ({ bookId, title, fileType })),
          active,
          prefs: { theme, followApp, pdfMode },
        };
        void commands.saveSession(JSON.stringify(session));
      }, 400);
    };
    const offTabs = useTabs.subscribe(save);
    const offPrefs = useReaderPrefs.subscribe(save);
    return () => {
      clearTimeout(timer);
      offTabs();
      offPrefs();
    };
  }, [libraryId]);
}
