import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { NativeSelect } from "@/components/ui/input";
import { KIND_LABELS, useProfiles } from "@/features/profiles";
import type { ArchiveSummaryDto, ProfileTargetDto } from "@/lib/ipc";
import { useArchiveSummary, useImportArchive } from "../api";
import { formatWhen } from "../model";
import { usePortability } from "../store";

/** Importing a Libreri archive: what is in it, and whose notes go where. */
export function ImportArchiveDialog() {
  const path = usePortability((s) => s.importing);
  const close = usePortability((s) => s.close);
  return (
    <Dialog
      open={path !== null}
      onOpenChange={(o) => !o && close()}
      title="Import a Libreri archive"
      description="Books are matched by their files, so every note finds its book and its place again."
      className="w-[600px]"
    >
      {path && <Body path={path} onDone={close} />}
    </Dialog>
  );
}

const encode = (t: ProfileTargetDto) =>
  t.kind === "existing" ? `existing:${t.profileId}` : t.kind;

const decode = (v: string): ProfileTargetDto =>
  v.startsWith("existing:")
    ? { kind: "existing", profileId: v.slice("existing:".length) }
    : v === "skip"
      ? { kind: "skip" }
      : { kind: "new" };

function Body({ path, onDone }: { path: string; onDone: () => void }) {
  const { data: summary, error, isLoading } = useArchiveSummary(path);
  if (isLoading) return <p className="text-muted-foreground">Reading the archive…</p>;
  if (error || !summary)
    return (
      <div className="flex flex-col gap-3">
        <p className="text-destructive">{error?.message ?? "The archive could not be read."}</p>
        <div className="flex justify-end">
          <Button variant="ghost" onClick={onDone}>
            Close
          </Button>
        </div>
      </div>
    );
  return <Plan path={path} summary={summary} onDone={onDone} />;
}

function Plan({
  path,
  summary: s,
  onDone,
}: {
  path: string;
  summary: ArchiveSummaryDto;
  onDone: () => void;
}) {
  const { data: profiles = [] } = useProfiles();
  const here = profiles.filter((p) => p.kind !== "guest");
  const [targets, setTargets] = useState<Record<string, ProfileTargetDto>>(() =>
    Object.fromEntries(s.profiles.map((p) => [p.archiveId, p.suggestion])),
  );
  const run = useImportArchive();

  const start = () =>
    run.mutate(
      {
        path,
        profiles: s.profiles.map((p) => ({
          archiveId: p.archiveId,
          target: targets[p.archiveId] ?? p.suggestion,
        })),
      },
      {
        onSuccess: () => {
          toast("Importing the archive…");
          onDone();
        },
      },
    );

  const rows: [string, number, string][] = [
    ["Already in this library", s.linked, "Details are merged; the newer version of each wins."],
    ["Added with their files", s.withFile, ""],
    [
      "Another copy is here",
      s.otherFile,
      "Same ISBN, DOI or title and author, different file. Notes find their place by the quoted text.",
    ],
    [
      "Kept as “file missing”",
      s.missing,
      "Notes are kept; everything reconnects when you add the same file.",
    ],
  ];

  return (
    <div className="flex flex-col gap-4">
      <p className="text-[12.5px] text-muted-foreground">
        {s.kind === "backup" ? "Backup" : "Export"} of <strong>{s.libraryName}</strong> ·{" "}
        {formatWhen(s.createdAt)} · {s.createdBy}
        {s.includesBookFiles ? " · with book files" : ""}
      </p>
      <div className="rounded-lg border">
        <div className="flex justify-between border-b px-3 py-2 font-medium">
          <span>{s.books} books</span>
          <span className="text-muted-foreground">
            {s.notes} highlights and bookmarks · {s.notebooks} notebooks
          </span>
        </div>
        {rows
          .filter(([, n]) => n > 0)
          .map(([label, n, hint]) => (
            <div key={label} className="flex gap-3 border-b px-3 py-2 last:border-b-0">
              <span className="w-12 text-right tabular-nums">{n}</span>
              <span className="flex flex-col">
                {label}
                {hint && <span className="text-[12px] text-muted-foreground">{hint}</span>}
              </span>
            </div>
          ))}
      </div>
      {s.profiles.length > 0 && (
        <div className="flex flex-col gap-2">
          <h3 className="font-medium">Whose notes go where</h3>
          {s.profiles.map((p) => (
            <label key={p.archiveId} className="flex items-center gap-3">
              <span className="flex min-w-0 flex-1 flex-col">
                <span className="truncate">{p.name}</span>
                <span className="text-[12px] text-muted-foreground">{KIND_LABELS[p.kind]}</span>
              </span>
              <NativeSelect
                aria-label={`Notes of ${p.name}`}
                className="w-56"
                value={encode(targets[p.archiveId] ?? p.suggestion)}
                onChange={(e) => setTargets({ ...targets, [p.archiveId]: decode(e.target.value) })}
              >
                {here.map((h) => (
                  <option key={h.id} value={`existing:${h.id}`}>
                    {h.name}
                  </option>
                ))}
                <option value="new">A new profile “{p.name}”</option>
                <option value="skip">Don&apos;t import</option>
              </NativeSelect>
            </label>
          ))}
          {!s.includesPins && (
            <p className="text-[12px] text-muted-foreground">
              New profiles start without a PIN; set one in Settings › Profiles & security.
            </p>
          )}
        </div>
      )}
      {run.error && <p className="text-destructive">{run.error.message}</p>}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button onClick={start} disabled={run.isPending}>
          Import
        </Button>
      </div>
    </div>
  );
}
