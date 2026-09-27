/**
 * The Create / Open flows, including the questions we ask along the way.
 * Kept out of the components so the Welcome screen and the command palette
 * share exactly the same behaviour.
 */
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { ask, message, open } from "@tauri-apps/plugin-dialog";
import { toast } from "sonner";
import { commands, isIpcError, unwrap } from "@/lib/ipc";
import { currentLibraryKey, libKey, useCreateLibrary, useOpenLibrary } from "../api";

const joinPath = (dir: string, name: string) => {
  const sep = dir.includes("\\") ? "\\" : "/";
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
};

export function useLibraryActions() {
  const create = useCreateLibrary();
  const openLib = useOpenLibrary();
  const qc = useQueryClient();
  const restore = useMutation({
    mutationFn: ({ archive, folder }: { archive: string; folder: string }) =>
      unwrap(commands.restoreLibrary(archive, folder)),
    onSuccess: (r) => {
      qc.removeQueries({ queryKey: libKey });
      qc.setQueryData(currentLibraryKey, r.library);
      void qc.invalidateQueries({ queryKey: ["settings"] });
    },
  });

  async function openPath(path: string) {
    try {
      await openLib.mutateAsync({ path });
    } catch (e) {
      if (isIpcError(e, "lockedElsewhere")) {
        const go = await ask(
          `${e.message}\n\nOpening it on two computers at once can lose changes.`,
          {
            title: "Library is open elsewhere",
            kind: "warning",
            okLabel: "Open anyway",
            cancelLabel: "Cancel",
          },
        );
        if (go) await openLib.mutateAsync({ path, force: true });
      } else {
        await message(e instanceof Error ? e.message : String(e), {
          title: "Could not open library",
          kind: "error",
        });
      }
    }
  }

  async function createNew() {
    const dir = await open({ directory: true, title: "Choose where your library will live" });
    if (typeof dir !== "string") return;

    const kind = await commands.inspectFolder(dir);
    let target = dir;
    if (kind === "library") {
      const go = await ask("This folder already contains a Libreri library. Open it instead?", {
        title: "Library found",
        okLabel: "Open library",
      });
      if (go) await openPath(dir);
      return;
    }
    if (kind === "otherFiles") {
      target = joinPath(dir, "Libreri Library");
      const go = await ask(
        `This folder already has other files. Libreri will create a new folder inside it:\n\n${target}`,
        { title: "Create library", okLabel: "Create" },
      );
      if (!go) return;
    }
    try {
      await create.mutateAsync({ path: target });
    } catch (e) {
      await message(e instanceof Error ? e.message : String(e), {
        title: "Could not create library",
        kind: "error",
      });
    }
  }

  async function openExisting() {
    const dir = await open({ directory: true, title: "Open a Libreri library" });
    if (typeof dir === "string") await openPath(dir);
  }

  /** Makes a new library from a backup or a Libreri archive. */
  async function restoreFromBackup() {
    const archive = await open({
      title: "Choose a backup or Libreri archive",
      filters: [{ name: "Libreri archive", extensions: ["libreri"] }],
    });
    if (typeof archive !== "string") return;
    const dir = await open({
      directory: true,
      title: "Choose where the restored library will live",
    });
    if (typeof dir !== "string") return;
    const kind = await commands.inspectFolder(dir);
    let target = dir;
    if (kind === "library") {
      await message(
        "This folder already holds a library. Open it and use Settings › Export & import › Import to merge the archive into it, or choose another folder.",
        { title: "Choose another folder", kind: "warning" },
      );
      return;
    }
    if (kind === "otherFiles") {
      target = joinPath(dir, "Restored Libreri Library");
      const go = await ask(
        `This folder already has other files. Libreri will create a new folder inside it:\n\n${target}`,
        { title: "Restore library", okLabel: "Restore" },
      );
      if (!go) return;
    }
    try {
      await restore.mutateAsync({ archive, folder: target });
      toast("Restoring the library…");
    } catch (e) {
      await message(e instanceof Error ? e.message : String(e), {
        title: "Could not restore",
        kind: "error",
      });
    }
  }

  return {
    createNew,
    openExisting,
    openPath,
    restoreFromBackup,
    busy: create.isPending || openLib.isPending || restore.isPending,
  };
}
