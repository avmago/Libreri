import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { commands, events } from "@/lib/ipc";
import { report } from "@/lib/windows";
import { summariseForeign, summariseImport } from "../model";
import { usePortability } from "../store";

const reveal = (path: string) =>
  void commands.revealPath(path).then((r) => r.status === "error" && toast.error(r.error.message));

/** Scheduled backup errors already shown, so a missing drive is not reported every hour. */
const shownErrors = new Set<string>();

/** Reports finished exports, archive imports and backups. */
export function usePortabilityEvents() {
  const qc = useQueryClient();
  const showMissing = usePortability((s) => s.showMissing);
  useEffect(() => {
    const offs = [
      events.exportFinished.listen(({ payload: r }) => {
        const what = r.books === 1 ? "1 book" : `${r.books} books`;
        const show = r.warnings.length ? toast.warning : toast.success;
        report(() =>
          show(`Exported ${what}`, {
            description: [r.path, ...r.warnings.slice(0, 3)].join("\n"),
            duration: 8_000,
            action: { label: "Show", onClick: () => reveal(r.path) },
          }),
        );
      }),
      events.archiveImported.listen(({ payload: r }) => {
        const { title, description } = summariseImport(r);
        const show = r.warnings.length ? toast.warning : toast.success;
        report(() =>
          show(title, {
            description,
            duration: 12_000,
            action: r.missing ? { label: "Show missing files", onClick: showMissing } : undefined,
          }),
        );
      }),
      events.foreignImported.listen(({ payload: r }) => {
        const { title, description } = summariseForeign(r);
        const show = r.failed.length ? toast.warning : toast.success;
        report(() => show(title, { description: description || undefined, duration: 12_000 }));
      }),
      events.backupFinished.listen(({ payload: r }) => {
        void qc.invalidateQueries({ queryKey: ["lib", "backups"] });
        if (r.error) {
          if (r.scheduled && shownErrors.has(r.error)) return;
          if (r.scheduled) shownErrors.add(r.error);
          const error = r.error;
          report(() => toast.error("The backup failed", { description: error, duration: 12_000 }));
        } else if (r.path && !r.scheduled) {
          const path = r.path;
          report(() =>
            toast.success("Backed up", {
              description: path,
              action: { label: "Show", onClick: () => reveal(path) },
            }),
          );
        }
      }),
    ];
    return () => {
      for (const p of offs) void p.then((off) => off());
    };
  }, [qc, showMissing]);
}
