import { useState } from "react";
import { Archive, Download, FolderOpen, HeartPulse, Upload } from "lucide-react";
import { toast } from "sonner";
import { open, save } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { NativeSelect } from "@/components/ui/input";
import {
  formatBytes,
  formatWhen,
  pickArchiveToImport,
  useBackUpNow,
  useBackupSettings,
  usePortability,
  useSetBackupSettings,
} from "@/features/portability";
import {
  commands,
  type BackupSettingsChange,
  type BackupSettingsDto,
  type ExportFormat,
  type SessionDto,
} from "@/lib/ipc";
import { Group, Row, Switch } from "../parts";

const KEPT_FORMATS: { id: ExportFormat; label: string; ext: string }[] = [
  { id: "bibtex", label: "BibTeX", ext: "bib" },
  { id: "cslJson", label: "CSL-JSON", ext: "json" },
  { id: "ris", label: "RIS", ext: "ris" },
  { id: "csv", label: "CSV", ext: "csv" },
  { id: "json", label: "JSON", ext: "json" },
];

const reveal = (path: string) =>
  void commands.revealPath(path).then((r) => r.status === "error" && toast.error(r.error.message));

function change(s: BackupSettingsDto): BackupSettingsChange {
  return {
    enabled: s.enabled,
    folder: s.folder,
    everyHours: s.everyHours,
    keep: s.keep,
    bookFiles: s.bookFiles,
    autoExport: s.autoExport,
  };
}

/** Board 25: export, import, backups, and a file kept up to date. */
export function ExportImportSettings({ session }: { session: SessionDto }) {
  const portability = usePortability();
  const { data: settings } = useBackupSettings();
  const isOwner = session.profile.kind === "owner";

  return (
    <>
      <Group
        title="Export and import"
        scope="yours"
        description="Take your books, details and notes anywhere. Exports never contain PINs or keys."
      >
        {session.canEditLibrary && (
          <Row
            label="Export"
            help="The whole library, or select books first. Libreri archive, Excel, CSV, JSON, BibTeX, RIS, CSL-JSON, Obsidian or Calibre."
          >
            <Button variant="outline" size="sm" onClick={() => portability.openExport(null)}>
              <Download /> Export…
            </Button>
          </Row>
        )}
        {session.canEditLibrary && (
          <Row
            label="Import from another app"
            help="Calibre, Zotero, Mendeley and other BibTeX or RIS exports, Goodreads and The StoryGraph. Files are copied."
          >
            <Button variant="outline" size="sm" onClick={() => portability.openForeign()}>
              <Upload /> Import…
            </Button>
          </Row>
        )}
        {isOwner && (
          <Row
            label="Import a Libreri archive"
            help="From another computer or a backup. Books are matched by their files; notes are never duplicated."
          >
            <Button variant="outline" size="sm" onClick={() => void pickArchiveToImport()}>
              <Upload /> Import…
            </Button>
          </Row>
        )}
        {session.canEditLibrary && (
          <Row
            label="Library health"
            help="Missing files, broken links in your notes, books that share an ISBN, and small repairs."
          >
            <Button variant="outline" size="sm" onClick={() => portability.setHealth(true)}>
              <HeartPulse /> Check…
            </Button>
          </Row>
        )}
      </Group>
      {settings && <Backups settings={settings} />}
      {settings && (
        <KeptFile
          key={`${settings.autoExport?.format ?? "off"}:${settings.autoExport?.path ?? ""}`}
          settings={settings}
        />
      )}
    </>
  );
}

function Backups({ settings: s }: { settings: BackupSettingsDto }) {
  const set = useSetBackupSettings();
  const now = useBackUpNow();
  const portability = usePortability();
  const edit = s.canEdit;
  const apply = (patch: Partial<BackupSettingsChange>) =>
    set.mutate({ ...change(s), ...patch }, { onError: (e) => toast.error(e.message) });

  const chooseFolder = async (enable: boolean) => {
    const folder = await open({
      directory: true,
      title: "Choose a folder for backups (ideally on another drive)",
    });
    if (typeof folder === "string") apply({ folder, enabled: enable || s.enabled });
  };

  return (
    <Group
      title="Backups"
      scope="computer"
      description="A Libreri archive of the whole library: details, everyone's notes and profiles (with their PINs), made while Libreri is open. Restore one from the Welcome screen, or import it here."
    >
      <Row
        label="Back up automatically"
        help={s.enabled ? undefined : "Off. Choose a folder to turn it on."}
      >
        <Switch
          label="Back up automatically"
          checked={s.enabled}
          onChange={(v) => (v && !s.folder ? void chooseFolder(true) : apply({ enabled: v }))}
        />
      </Row>
      <Row
        label="Folder"
        help={
          <span className="font-mono text-[11.5px] break-all">{s.folder ?? "Not chosen yet"}</span>
        }
      >
        {s.folder && (
          <Button variant="ghost" size="sm" onClick={() => reveal(s.folder!)}>
            Show
          </Button>
        )}
        <Button
          variant="outline"
          size="sm"
          disabled={!edit}
          onClick={() => void chooseFolder(false)}
        >
          <FolderOpen /> Choose…
        </Button>
      </Row>
      <Row label="How often" htmlFor="backup-every">
        <NativeSelect
          id="backup-every"
          disabled={!edit}
          value={String(s.everyHours)}
          onChange={(e) => apply({ everyHours: Number(e.target.value) })}
        >
          <option value="24">Every day</option>
          <option value="168">Every week</option>
        </NativeSelect>
      </Row>
      <Row label="Keep" help="Older backups are removed." htmlFor="backup-keep">
        <NativeSelect
          id="backup-keep"
          disabled={!edit}
          value={String(s.keep)}
          onChange={(e) => apply({ keep: Number(e.target.value) })}
        >
          {[3, 5, 10, 20, 50].map((n) => (
            <option key={n} value={n}>
              The last {n}
            </option>
          ))}
        </NativeSelect>
      </Row>
      <Row
        label="Include the book files"
        help="Makes backups as large as your library. Without them, restoring keeps every note and reconnects when the files are added."
      >
        <Switch
          label="Include the book files"
          checked={s.bookFiles}
          onChange={(v) => edit && apply({ bookFiles: v })}
        />
      </Row>
      <Row
        label="Last backup"
        help={
          s.lastError ? (
            <span className="text-destructive">{s.lastError}</span>
          ) : (
            formatWhen(s.lastBackup)
          )
        }
      >
        <Button
          variant="outline"
          size="sm"
          disabled={!edit || !s.folder || now.isPending}
          onClick={() =>
            now.mutate(undefined, {
              onSuccess: (id) => toast(id ? "Backing up…" : "A backup is already running"),
              onError: (e) => toast.error(e.message),
            })
          }
        >
          <Archive /> Back up now
        </Button>
      </Row>
      {s.backups.length > 0 && (
        <div className="flex flex-col gap-1 px-4 py-3">
          <span className="font-medium">Backups in the folder</span>
          <ul className="flex flex-col">
            {s.backups.map((b) => (
              <li key={b.path} className="flex items-center gap-3 py-1 text-[12.5px]">
                <span className="flex-1">
                  {formatWhen(b.createdAt)}{" "}
                  <span className="text-muted-foreground">
                    · {b.books} books · {formatBytes(b.size)}
                    {b.includesBookFiles ? " · with files" : ""}
                  </span>
                </span>
                <Button variant="ghost" size="sm" onClick={() => reveal(b.path)}>
                  Show
                </Button>
                {edit && (
                  <Button variant="ghost" size="sm" onClick={() => portability.openImport(b.path)}>
                    Restore…
                  </Button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
      {!edit && (
        <p className="px-4 py-3 text-[12.5px] text-muted-foreground">
          Only the owner can change backups.
        </p>
      )}
    </Group>
  );
}

function KeptFile({ settings: s }: { settings: BackupSettingsDto }) {
  const set = useSetBackupSettings();
  const edit = s.canEdit;
  const [format, setFormat] = useState<ExportFormat | "off">(s.autoExport?.format ?? "off");

  const choose = async (f: ExportFormat) => {
    const info = KEPT_FORMATS.find((k) => k.id === f)!;
    const path = await save({
      title: `Keep a ${info.label} file up to date`,
      defaultPath: `library.${info.ext}`,
      filters: [{ name: info.label, extensions: [info.ext] }],
    });
    if (!path) return setFormat(s.autoExport?.format ?? "off");
    set.mutate(
      { ...change(s), autoExport: { format: f, path } },
      { onError: (e) => toast.error(e.message) },
    );
  };

  return (
    <Group
      title="Keep a file up to date"
      scope="computer"
      description="Libreri rewrites this file when the library changes: a .bib for LaTeX, or a CSL-JSON file for Pandoc and Zotero. Book details only, never notes."
    >
      <Row label="Format" htmlFor="kept-format">
        <NativeSelect
          id="kept-format"
          disabled={!edit}
          value={format}
          onChange={(e) => {
            const v = e.target.value as ExportFormat | "off";
            setFormat(v);
            if (v === "off") set.mutate({ ...change(s), autoExport: null });
            else void choose(v);
          }}
        >
          <option value="off">Off</option>
          {KEPT_FORMATS.map((k) => (
            <option key={k.id} value={k.id}>
              {k.label}
            </option>
          ))}
        </NativeSelect>
      </Row>
      {s.autoExport && (
        <Row
          label="File"
          help={
            <span className="flex flex-col gap-0.5">
              <span className="font-mono text-[11.5px] break-all">{s.autoExport.path}</span>
              {s.autoExportError ? (
                <span className="text-destructive">{s.autoExportError}</span>
              ) : (
                <span>Last written {formatWhen(s.lastAutoExport)}</span>
              )}
            </span>
          }
        >
          <Button variant="ghost" size="sm" onClick={() => reveal(s.autoExport!.path)}>
            Show
          </Button>
          <Button
            variant="outline"
            size="sm"
            disabled={!edit}
            onClick={() => void choose(s.autoExport!.format)}
          >
            Change…
          </Button>
        </Row>
      )}
    </Group>
  );
}
