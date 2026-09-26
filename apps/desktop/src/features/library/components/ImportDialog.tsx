import { FileUp } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { NativeSelect } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { useFolders, useImportPaths } from "../api";
import { useImport } from "../import";
import { flattenFolders } from "./folderUtils";

function basename(p: string) {
  return p.split(/[\\/]/).filter(Boolean).pop() ?? p;
}

export function ImportDialog() {
  const { pending, mode, close, setMode, setFolder } = useImport();
  const { data: folders = [] } = useFolders();
  const importPaths = useImportPaths();
  if (!pending) return null;
  const folder = pending.folder;

  const n = pending.paths.length;
  const start = () => {
    importPaths.mutate(
      { paths: pending.paths, folder, mode },
      { onError: (e) => toast.error("Could not start the import", { description: String(e) }) },
    );
    close();
  };

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && close()}
      title={n === 1 ? `Import “${basename(pending.paths[0] ?? "")}”` : `Import ${n} items`}
      description="Folders keep their structure. Books already in the library are skipped."
    >
      {n > 1 && (
        <ul className="max-h-28 overflow-y-auto rounded-md border bg-sidebar px-3 py-2 font-mono text-[11.5px] text-muted-foreground">
          {pending.paths.slice(0, 50).map((p) => (
            <li key={p} className="truncate">
              {basename(p)}
            </li>
          ))}
          {n > 50 && <li>…and {n - 50} more</li>}
        </ul>
      )}
      <label className="flex flex-col gap-1">
        <span className="text-[11.5px] font-medium text-muted-foreground">Into folder</span>
        <NativeSelect value={folder} onChange={(e) => setFolder(e.target.value)}>
          <option value="">Books (top level)</option>
          {flattenFolders(folders).map(({ folder: f, depth }) => (
            <option key={f.path} value={f.path}>
              {"  ".repeat(depth + 1)}
              {f.name}
            </option>
          ))}
        </NativeSelect>
      </label>
      <fieldset className="flex flex-col gap-1">
        <legend className="mb-1 text-[11.5px] font-medium text-muted-foreground">
          The original files
        </legend>
        {(
          [
            ["move", "Move them into the library", "Keeps one copy on your disk."],
            ["copy", "Copy them", "Leaves the originals where they are."],
          ] as const
        ).map(([value, label, hint]) => (
          <label
            key={value}
            className={cn(
              "flex cursor-default items-start gap-2.5 rounded-md border px-3 py-2",
              mode === value && "border-ring bg-muted/50",
            )}
          >
            <input
              type="radio"
              name="import-mode"
              value={value}
              checked={mode === value}
              onChange={() => setMode(value)}
              className="mt-0.5 accent-[var(--primary)]"
            />
            <span className="flex flex-col">
              <span className="font-medium">{label}</span>
              <span className="text-[12px] text-muted-foreground">{hint}</span>
            </span>
          </label>
        ))}
      </fieldset>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={close}>
          Cancel
        </Button>
        <Button autoFocus onClick={start}>
          <FileUp /> Import
        </Button>
      </div>
    </Dialog>
  );
}

/** Full-window hint while files are dragged over the window. */
export function DropOverlay() {
  const hovering = useImport((s) => s.hovering);
  if (!hovering) return null;
  return (
    <div className="pointer-events-none fixed inset-2 z-40 flex items-end justify-center rounded-xl border-2 border-dashed border-primary/60 bg-primary/5 pb-10">
      <span className="rounded-md bg-popover px-3 py-1.5 text-[13px] font-medium shadow">
        Drop to import — onto a folder in the sidebar to put them there
      </span>
    </div>
  );
}
