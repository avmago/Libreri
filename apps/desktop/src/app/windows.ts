import { useMemo } from "react";
import { toast } from "sonner";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { flushSession } from "@/features/reader";
import { useTabs, type BookTab } from "@/lib/tabs";
import { windowUrl } from "@/lib/windows";

let counter = 0;

async function openWindow(tab: BookTab | null) {
  const label = `libreri-${Date.now().toString(36)}-${counter++}`;
  const w = new WebviewWindow(label, {
    url: windowUrl(tab),
    title: tab ? `${tab.title} — Libreri` : "Libreri",
    width: 1100,
    height: 780,
    minWidth: 480,
    minHeight: 480,
  });
  await new Promise<void>((resolve, reject) => {
    void w.once("tauri://created", () => resolve());
    void w.once("tauri://error", (e) => reject(new Error(String(e.payload))));
  });
}

/** Opening windows: a new empty one, or one for a book tab. */
export function useWindowActions() {
  return useMemo(
    () => ({
      newWindow: async () => {
        try {
          await openWindow(null);
        } catch (e) {
          toast.error("Could not open a new window", { description: String(e) });
        }
      },
      moveTabToWindow: async (bookId: string) => {
        const tab = useTabs.getState().tabs.find((t) => t.bookId === bookId);
        if (!tab) return;
        if (tab.feed) {
          toast("Add it to the library to read it in a window of its own");
          return;
        }
        try {
          // Save the reading position first, so the new window opens there.
          await flushSession();
          useTabs.getState().detach(bookId);
          await openWindow(tab);
        } catch (e) {
          useTabs.getState().open(tab);
          toast.error("Could not open a new window", { description: String(e) });
        }
      },
    }),
    [],
  );
}
