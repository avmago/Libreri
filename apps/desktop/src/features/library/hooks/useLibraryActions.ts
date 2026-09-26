/**
 * The Create / Open flows, including the questions we ask along the way.
 * Kept out of the components so the Welcome screen and the command palette
 * share exactly the same behaviour.
 */
import { ask, message, open } from "@tauri-apps/plugin-dialog";
import { commands, isIpcError } from "@/lib/ipc";
import { useCreateLibrary, useOpenLibrary } from "../api";

const joinPath = (dir: string, name: string) => {
  const sep = dir.includes("\\") ? "\\" : "/";
  return dir.endsWith(sep) ? dir + name : dir + sep + name;
};

export function useLibraryActions() {
  const create = useCreateLibrary();
  const openLib = useOpenLibrary();

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

  return { createNew, openExisting, openPath, busy: create.isPending || openLib.isPending };
}
