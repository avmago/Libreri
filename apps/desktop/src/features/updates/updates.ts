/**
 * Updates (Phase 10): looking for a new version, and installing it. The
 * window saves what is pending (notes, place, preferences) before Libreri
 * restarts into the new version.
 */
import { toast } from "sonner";
import { create } from "zustand";
import { flushNotes } from "@/features/notes";
import { commands, events, unwrap, type UpdateDto } from "@/lib/ipc";

interface UpdateState {
  found: UpdateDto | null;
  checking: boolean;
  /** When looked last (ms), and what came of it. */
  checked: { at: number; error: string | null } | null;
  installing: { done: number; total: number | null } | null;
  check: (quiet?: boolean) => Promise<void>;
  install: () => Promise<void>;
}

export const useUpdates = create<UpdateState>((set, get) => ({
  found: null,
  checking: false,
  checked: null,
  installing: null,
  check: async (quiet = false) => {
    if (get().checking) return;
    set({ checking: true });
    try {
      const found = await unwrap(commands.updateCheck());
      set({ found, checked: { at: Date.now(), error: null } });
      if (found) offer(found);
      else if (!quiet) toast.success("Libreri is up to date");
    } catch (e) {
      const error = e instanceof Error ? e.message : String(e);
      set({ checked: { at: Date.now(), error } });
      if (!quiet) toast.error("Could not look for updates", { description: error });
    } finally {
      set({ checking: false });
    }
  },
  install: async () => {
    if (get().installing) return;
    set({ installing: { done: 0, total: null } });
    const off = await events.updateProgress.listen(({ payload }) =>
      set({ installing: { done: payload.done ?? 0, total: payload.total } }),
    );
    try {
      // Everything typed is written before the restart.
      await flushNotes();
      window.dispatchEvent(new Event("libreri:before-restart"));
      await unwrap(commands.updateInstall());
    } catch (e) {
      toast.error("The update could not be installed", {
        description: e instanceof Error ? e.message : String(e),
      });
      set({ installing: null });
    } finally {
      off();
    }
  },
}));

function offer(u: UpdateDto) {
  toast(`Libreri ${u.version} is available`, {
    id: "libreri-update",
    duration: Infinity,
    description: u.notes?.split("\n").find((l) => l.trim()) ?? `You have ${u.current}.`,
    action: { label: "Install and restart", onClick: () => void useUpdates.getState().install() },
    cancel: { label: "Later", onClick: () => {} },
  });
}

/** Once, a little after Libreri starts, when looking is on. */
export async function checkOnStart() {
  try {
    const s = await commands.updateStatus();
    if (s.available && s.checkOnStart) await useUpdates.getState().check(true);
  } catch {
    /* not offered here */
  }
}
