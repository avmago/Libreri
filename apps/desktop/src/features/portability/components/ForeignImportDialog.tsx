import { useState, type ReactNode } from "react";
import { ArrowDown, ArrowUp, BookMarked, FileText, GraduationCap, Library } from "lucide-react";
import { toast } from "sonner";
import { homeDir, join } from "@tauri-apps/api/path";
import { open } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { NativeSelect } from "@/components/ui/input";
import { FILE_TYPE_LABEL, flattenFolders, useFolders } from "@/features/library";
import type { FileType, ForeignSummaryDto } from "@/lib/ipc";
import { useForeignSummary, useImportForeign } from "../api";
import { usePortability } from "../store";

/** The Calibre format order Libreri uses unless changed. */
const DEFAULT_ORDER: FileType[] = [
  "epub",
  "pdf",
  "azw3",
  "mobi",
  "fb2",
  "djvu",
  "cbz",
  "cbr",
  "rtf",
  "txt",
];

async function home(sub: string): Promise<string | undefined> {
  try {
    return await join(await homeDir(), sub);
  } catch {
    return undefined;
  }
}

/** Calibre, Zotero, BibTeX/RIS and Goodreads/StoryGraph into this library. */
export function ForeignImportDialog() {
  const foreign = usePortability((s) => s.foreign);
  const close = usePortability((s) => s.close);
  return (
    <Dialog
      open={foreign !== null}
      onOpenChange={(o) => !o && close()}
      title="Import from another app"
      description="Book files are copied; the other app's library is only read, never changed."
      className="w-[600px]"
    >
      {foreign === "choose" && <Choose />}
      {foreign !== null && foreign !== "choose" && (
        <Preview key={foreign} path={foreign} onDone={close} />
      )}
    </Dialog>
  );
}

function SourceButton({
  icon,
  title,
  hint,
  onClick,
}: {
  icon: ReactNode;
  title: string;
  hint: string;
  onClick: () => void;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      className="flex items-start gap-3 rounded-lg border px-3 py-2.5 text-left hover:bg-muted/60 [&_svg]:mt-0.5 [&_svg]:size-4 [&_svg]:shrink-0"
    >
      {icon}
      <span className="flex flex-col gap-0.5">
        <span className="font-medium">{title}</span>
        <span className="text-[12.5px] text-muted-foreground">{hint}</span>
      </span>
    </button>
  );
}

function Choose() {
  const openForeign = usePortability((s) => s.openForeign);
  const pickFolder = async (title: string, sub: string) => {
    const dir = await open({ directory: true, title, defaultPath: await home(sub) });
    if (typeof dir === "string") openForeign(dir);
  };
  const pickFile = async (title: string, extensions: string[], name: string) => {
    const file = await open({ title, filters: [{ name, extensions }] });
    if (typeof file === "string") openForeign(file);
  };
  return (
    <div className="flex flex-col gap-2">
      <SourceButton
        icon={<Library />}
        title="Calibre"
        hint="Choose your Calibre library folder (the one with metadata.db). Details, tags, series, ratings and covers come along; one file per book."
        onClick={() => void pickFolder("Choose your Calibre library folder", "Calibre Library")}
      />
      <SourceButton
        icon={<GraduationCap />}
        title="Zotero"
        hint="Choose Zotero's data folder (usually Zotero in your home folder). PDFs, details, collections as categories, tags, notes and PDF highlights."
        onClick={() => void pickFolder("Choose Zotero's data folder", "Zotero")}
      />
      <SourceButton
        icon={<FileText />}
        title="Mendeley, JabRef and others"
        hint="A BibTeX (.bib) or RIS (.ris) export. Attached files listed in it are copied; entries without a file add their details to matching books."
        onClick={() =>
          void pickFile("Choose a BibTeX or RIS file", ["bib", "ris"], "BibTeX or RIS")
        }
      />
      <SourceButton
        icon={<BookMarked />}
        title="Goodreads or The StoryGraph"
        hint="Their export (.csv). Updates reading status and ratings of books already in this library; nothing is added."
        onClick={() => void pickFile("Choose a Goodreads or StoryGraph export", ["csv"], "CSV")}
      />
    </div>
  );
}

function Preview({ path, onDone }: { path: string; onDone: () => void }) {
  const { data: s, error, isLoading } = useForeignSummary(path);
  const openForeign = usePortability((s) => s.openForeign);
  if (isLoading) return <p className="text-muted-foreground">Reading…</p>;
  if (error || !s)
    return (
      <div className="flex flex-col gap-3">
        <p className="text-destructive">{error?.message ?? "This could not be read."}</p>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={() => openForeign()}>
            Back
          </Button>
          <Button variant="ghost" onClick={onDone}>
            Close
          </Button>
        </div>
      </div>
    );
  return <Plan path={path} s={s} onDone={onDone} />;
}

function Plan({ path, s, onDone }: { path: string; s: ForeignSummaryDto; onDone: () => void }) {
  const { data: folders = [] } = useFolders();
  const present = s.formats.map((f) => f.fileType);
  const [order, setOrder] = useState<FileType[]>(() => [
    ...DEFAULT_ORDER.filter((t) => present.includes(t)),
    ...present.filter((t) => !DEFAULT_ORDER.includes(t)),
  ]);
  const [folder, setFolder] = useState("");
  const [replace, setReplace] = useState(false);
  const run = useImportForeign();
  const count = (t: FileType) => s.formats.find((f) => f.fileType === t)?.count ?? 0;
  const move = (i: number, by: number) => {
    const next = [...order];
    const [x] = next.splice(i, 1);
    next.splice(i + by, 0, x!);
    setOrder(next);
  };

  const start = () =>
    run.mutate(
      {
        path,
        options: { formats: s.source === "calibre" ? order : [], folder, replacePersonal: replace },
      },
      {
        onSuccess: () => {
          toast(`Importing from ${s.label}…`);
          onDone();
        },
      },
    );

  const facts = s.readingLog
    ? [
        `${s.books} books in the export`,
        `${s.matched} of them are in this library and will be updated`,
      ]
    : [
        `${s.books} ${s.books === 1 ? "item" : "items"}`,
        `${s.withFiles} with a file Libreri can open`,
        ...(s.highlights ? [`${s.highlights} PDF highlights and notes`] : []),
        ...(s.notes ? [`${s.notes} notes (added to the book's notebook)`] : []),
        ...(s.rated ? [`${s.rated} rated`] : []),
      ];

  return (
    <div className="flex flex-col gap-4">
      <div className="rounded-lg border px-3 py-2.5">
        <p className="font-medium">{s.label}</p>
        <ul className="list-disc pl-5 text-[12.5px] text-muted-foreground">
          {facts.map((f) => (
            <li key={f}>{f}</li>
          ))}
        </ul>
        {!s.readingLog && s.books > s.withFiles && (
          <p className="pt-1 text-[12.5px] text-muted-foreground">
            Items without a file add their details to a matching book here, if there is one.
          </p>
        )}
      </div>

      {s.source === "calibre" && order.length > 1 && (
        <div className="flex flex-col gap-1.5">
          <h3 className="font-medium">When a book has several files, take the first of</h3>
          <ol className="flex flex-col gap-1">
            {order.map((t, i) => (
              <li key={t} className="flex items-center gap-2 rounded-md border px-2.5 py-1">
                <span className="w-5 text-muted-foreground tabular-nums">{i + 1}.</span>
                <span className="flex-1">
                  {FILE_TYPE_LABEL[t]}{" "}
                  <span className="text-[12px] text-muted-foreground">({count(t)} files)</span>
                </span>
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={`Move ${FILE_TYPE_LABEL[t]} up`}
                  disabled={i === 0}
                  onClick={() => move(i, -1)}
                >
                  <ArrowUp />
                </Button>
                <Button
                  size="icon"
                  variant="ghost"
                  aria-label={`Move ${FILE_TYPE_LABEL[t]} down`}
                  disabled={i === order.length - 1}
                  onClick={() => move(i, 1)}
                >
                  <ArrowDown />
                </Button>
              </li>
            ))}
          </ol>
        </div>
      )}

      {!s.readingLog && (
        <label className="flex items-center gap-3">
          <span className="flex-1 font-medium">Copy the files into</span>
          <NativeSelect className="w-64" value={folder} onChange={(e) => setFolder(e.target.value)}>
            <option value="">Books (top level)</option>
            {flattenFolders(folders).map(({ folder: f, depth }) => (
              <option key={f.path} value={f.path}>
                {"  ".repeat(depth)}
                {f.name}
              </option>
            ))}
          </NativeSelect>
        </label>
      )}

      {(s.rated > 0 || s.readingLog) && (
        <label className="flex items-start gap-2">
          <input
            type="checkbox"
            className="mt-0.5"
            checked={replace}
            onChange={(e) => setReplace(e.target.checked)}
          />
          <span className="flex flex-col">
            Replace my reading status and ratings
            <span className="text-[12px] text-muted-foreground">
              Off: only books without a status or rating get one. Ratings and status go to your
              profile only.
            </span>
          </span>
        </label>
      )}

      {s.warnings.length > 0 && (
        <details className="text-[12.5px] text-muted-foreground">
          <summary>{s.warnings.length} notes about this library</summary>
          <ul className="pt-1 pl-4">
            {s.warnings.map((w) => (
              <li key={w}>{w}</li>
            ))}
          </ul>
        </details>
      )}
      {run.error && <p className="text-destructive">{run.error.message}</p>}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button
          onClick={start}
          disabled={run.isPending || (s.readingLog ? s.matched === 0 : s.books === 0)}
        >
          {s.readingLog ? `Update ${s.matched} books` : "Import"}
        </Button>
      </div>
    </div>
  );
}
