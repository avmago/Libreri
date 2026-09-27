import { useState } from "react";
import { toast } from "sonner";
import { save } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useBook, useCurrentLibrary } from "@/features/library";
import { usePermissions } from "@/features/profiles";
import { commands, type ExportFormat } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import {
  FORMATS,
  OPTION_LABEL,
  defaultFileName,
  formatInfo,
  type ExportOption,
  type FormatInfo,
} from "../model";
import { usePortability } from "../store";

const GROUPS = ["Everything", "Tables and data", "Citations", "Other apps"] as const;

/** Board 24: choose a format, what to include, and where to save. */
export function ExportDialog() {
  const exporting = usePortability((s) => s.exporting);
  const close = usePortability((s) => s.close);
  return (
    <Dialog
      open={exporting !== null}
      onOpenChange={(o) => !o && close()}
      title="Export"
      description="Your data is yours: take it to another computer, another app, or a spreadsheet."
      className="w-[720px]"
    >
      {exporting && <Form bookIds={exporting.bookIds} onDone={close} />}
    </Dialog>
  );
}

function Form({ bookIds, onDone }: { bookIds: string[] | null; onDone: () => void }) {
  const { data: library } = useCurrentLibrary();
  const { kind } = usePermissions();
  const isOwner = kind === "owner";
  const [format, setFormat] = useState<ExportFormat>(bookIds ? "bibtex" : "archive");
  const [scope, setScope] = useState<"all" | "selection">(bookIds ? "selection" : "all");
  const [opts, setOpts] = useState<Record<ExportOption, boolean>>({
    personal: true,
    notes: true,
    bookFiles: false,
    everyone: false,
  });
  const [busy, setBusy] = useState(false);
  const info = formatInfo(format);
  const first = useBook(bookIds?.length === 1 ? bookIds[0]! : null);
  const selectionOnly = scope === "selection" && info.selection && bookIds !== null;

  const shown = (o: ExportOption) => info.options.includes(o) && (o !== "everyone" || isOwner);
  // Calibre can only add books whose files are there.
  const forced = (o: ExportOption) => format === "calibre" && o === "bookFiles";

  async function start() {
    const books = selectionOnly
      ? bookIds.length === 1 && first.data
        ? [{ title: first.data.metadata.title }]
        : bookIds.map(() => ({ title: "" }))
      : null;
    const name = defaultFileName(info, library?.name ?? "Libreri", books);
    const dest = await save({
      title: info.ext ? `Export as ${info.label}` : `Name the new folder for ${info.label}`,
      defaultPath: name,
      filters: info.ext ? [{ name: info.label, extensions: [info.ext] }] : undefined,
    });
    if (!dest) return;
    setBusy(true);
    const on = (o: ExportOption) => shown(o) && (opts[o] || forced(o));
    const r = await commands.exportBooks({
      format,
      bookIds: selectionOnly ? bookIds : null,
      dest,
      personal: on("personal"),
      notes: on("notes"),
      bookFiles: on("bookFiles"),
      everyone: on("everyone"),
    });
    setBusy(false);
    if (r.status === "error") {
      toast.error(r.error.message);
      return;
    }
    toast(`Exporting as ${info.label}…`);
    onDone();
  }

  return (
    <div className="flex flex-col gap-4">
      <div className="grid grid-cols-[1fr_1fr] gap-5">
        <div role="radiogroup" aria-label="Format" className="flex flex-col gap-3">
          {GROUPS.map((g) => (
            <div key={g} className="flex flex-col gap-1">
              <h3 className="text-[11px] font-semibold tracking-wide text-muted-foreground uppercase">
                {g}
              </h3>
              {FORMATS.filter((f) => f.group === g).map((f) => (
                <FormatRow
                  key={f.id}
                  format={f}
                  checked={f.id === format}
                  onSelect={() => setFormat(f.id)}
                />
              ))}
            </div>
          ))}
        </div>
        <div className="flex flex-col gap-4">
          <p className="rounded-md bg-muted px-3 py-2.5 text-[12.5px] leading-relaxed">
            {info.hint}
          </p>
          <fieldset className="flex flex-col gap-1.5">
            <legend className="pb-1 font-medium">Books</legend>
            <label className="flex items-center gap-2">
              <input
                type="radio"
                name="scope"
                checked={!selectionOnly}
                onChange={() => setScope("all")}
              />
              The whole library{library ? ` (${library.bookCount})` : ""}
            </label>
            <label
              className={cn(
                "flex items-center gap-2",
                (!bookIds || !info.selection) && "opacity-50",
              )}
            >
              <input
                type="radio"
                name="scope"
                disabled={!bookIds || !info.selection}
                checked={selectionOnly}
                onChange={() => setScope("selection")}
              />
              {bookIds
                ? bookIds.length === 1
                  ? `The selected book${first.data ? ` (${first.data.metadata.title})` : ""}`
                  : `The ${bookIds.length} selected books`
                : "The selected books (select some first)"}
            </label>
          </fieldset>
          {info.options.some(shown) && (
            <fieldset className="flex flex-col gap-1.5">
              <legend className="pb-1 font-medium">Include</legend>
              {info.options.filter(shown).map((o) => (
                <label key={o} className="flex items-start gap-2">
                  <input
                    type="checkbox"
                    className="mt-0.5"
                    checked={opts[o] || forced(o)}
                    disabled={forced(o)}
                    onChange={(e) => setOpts({ ...opts, [o]: e.target.checked })}
                  />
                  <span className="flex flex-col">
                    {OPTION_LABEL[o]}
                    {o === "bookFiles" && format === "archive" && (
                      <span className="text-[12px] text-muted-foreground">
                        Without them the archive stays small; notes reconnect when the same files
                        are added on the other computer.
                      </span>
                    )}
                    {o === "everyone" && (
                      <span className="text-[12px] text-muted-foreground">
                        Only the owner can include other people&apos;s notes.
                      </span>
                    )}
                  </span>
                </label>
              ))}
            </fieldset>
          )}
          <p className="text-[12px] text-muted-foreground">
            Exports never contain PINs or online-source keys.
          </p>
        </div>
      </div>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button onClick={() => void start()} disabled={busy}>
          {info.ext ? "Export…" : "Choose folder…"}
        </Button>
      </div>
    </div>
  );
}

function FormatRow({
  format,
  checked,
  onSelect,
}: {
  format: FormatInfo;
  checked: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      role="radio"
      aria-checked={checked}
      onClick={onSelect}
      className={cn(
        "flex items-center justify-between rounded-md border px-2.5 py-1.5 text-left",
        checked ? "border-primary bg-muted font-medium" : "hover:bg-muted/60",
      )}
    >
      {format.label}
      <span className="font-mono text-[11px] text-muted-foreground">
        {format.ext ? `.${format.ext}` : "folder"}
      </span>
    </button>
  );
}
