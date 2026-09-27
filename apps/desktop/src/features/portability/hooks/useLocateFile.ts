import { toast } from "sonner";
import { ask, open } from "@tauri-apps/plugin-dialog";
import { useQueryClient } from "@tanstack/react-query";
import { commands } from "@/lib/ipc";

/**
 * "Locate file…" for a book whose file is missing. The chosen file is
 * copied into the library (the original stays where it is).
 */
export function useLocateFile() {
  const qc = useQueryClient();
  return async (book: { id: string; title: string }) => {
    const path = await open({ title: `Where is “${book.title}”?` });
    if (typeof path !== "string") return;
    let r = await commands.locateFile(book.id, path, true, false);
    if (r.status === "ok" && r.data === "differentFile") {
      const use = await ask(
        "This is not the same file your notes were made in; perhaps another edition. Use it anyway? Highlights find their place again by the words they quote.",
        { title: "A different file", okLabel: "Use this file", kind: "warning" },
      );
      if (!use) return;
      r = await commands.locateFile(book.id, path, true, true);
    }
    if (r.status === "error") {
      toast.error(r.error.message);
      return;
    }
    toast.success(`“${book.title}” is back`);
    void qc.invalidateQueries({ queryKey: ["lib"] });
  };
}
