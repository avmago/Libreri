import { useState } from "react";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { commands, unwrap, type CompareSourceDto } from "@/lib/ipc";
import type { CompareRequest } from "@/lib/tabs";
import { cn } from "@/lib/utils";

type Other = "version" | "book" | "file";

const inputClass =
  "h-8 min-w-0 rounded-md border border-input bg-background px-2 outline-none focus-visible:border-ring";

/** "Compare with…": an earlier version, another book or a file. */
export function CompareDialog({
  open,
  bookId,
  onClose,
  onCompare,
}: {
  open: boolean;
  bookId: string;
  onClose: () => void;
  onCompare: (r: CompareRequest) => void;
}) {
  const { data: versions = [] } = useQuery({
    queryKey: ["lib", "versions", bookId],
    queryFn: () => unwrap(commands.listVersions(bookId)),
    enabled: open,
  });
  const [other, setOther] = useState<Other | null>(null);
  const kind = other ?? (versions.length ? "version" : "book");
  const [version, setVersion] = useState<string>("");
  const [search, setSearch] = useState("");
  const [pick, setPick] = useState<{ id: string; title: string } | null>(null);
  const [file, setFile] = useState<string | null>(null);
  const { data: books = [] } = useQuery({
    queryKey: ["lib", "compare-books", search],
    queryFn: () =>
      unwrap(
        commands.listBooks({ search: search || null, fileTypes: ["pdf", "djvu"], sort: "title" }),
      ),
    enabled: open && kind === "book",
  });
  const chosenVersion = version || versions[0]?.id || "";
  const here: CompareSourceDto = { kind: "book", id: bookId };
  const request = (): CompareRequest | null => {
    if (kind === "version" && chosenVersion)
      return { a: { kind: "version", book: bookId, version: chosenVersion }, b: here };
    if (kind === "book" && pick) return { a: here, b: { kind: "book", id: pick.id } };
    if (kind === "file" && file) return { a: here, b: { kind: "file", path: file } };
    return null;
  };
  const when = (s: string) =>
    new Date(s).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Compare"
      description="Find what changed: words removed, added or changed, and pages that look different."
      className="w-[540px]"
    >
      <div
        role="radiogroup"
        aria-label="Compare with"
        className="flex gap-1 rounded-lg bg-muted p-1"
      >
        {(
          [
            ["version", "An earlier version"],
            ["book", "Another book"],
            ["file", "A file"],
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            type="button"
            role="radio"
            aria-checked={kind === id}
            onClick={() => setOther(id)}
            className={cn(
              "flex-1 rounded-md py-1.5 text-[13px] font-medium text-muted-foreground",
              kind === id && "bg-background text-foreground shadow-sm",
            )}
          >
            {label}
          </button>
        ))}
      </div>

      {kind === "version" &&
        (versions.length ? (
          <label className="flex flex-col gap-1">
            <span>The book now, compared with how it was before</span>
            <select
              value={chosenVersion}
              onChange={(e) => setVersion(e.target.value)}
              className={inputClass}
            >
              {versions.map((v) => (
                <option key={v.id} value={v.id}>
                  {v.reason} · {when(v.savedAt)}
                </option>
              ))}
            </select>
          </label>
        ) : (
          <p className="text-muted-foreground">
            This book has no earlier versions. They are kept when you edit its pages, redact, fill
            in a form or save markup into it.
          </p>
        ))}

      {kind === "book" && (
        <div className="flex flex-col gap-2">
          <input
            autoFocus
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Find a PDF or DjVu book"
            className={inputClass}
          />
          <ul className="flex max-h-64 flex-col overflow-auto rounded-md border">
            {books
              .filter((b) => b.id !== bookId)
              .map((b) => (
                <li key={b.id}>
                  <button
                    type="button"
                    onClick={() => setPick({ id: b.id, title: b.metadata.title ?? "" })}
                    className={cn(
                      "flex w-full flex-col items-start px-3 py-1.5 text-left hover:bg-muted",
                      pick?.id === b.id && "bg-muted",
                    )}
                  >
                    <span className="font-medium">{b.metadata.title}</span>
                    <span className="text-[12px] text-muted-foreground">
                      {(b.metadata.authors ?? []).join(", ")} · {b.fileType.toUpperCase()}
                    </span>
                  </button>
                </li>
              ))}
            {!books.length && <li className="px-3 py-2 text-muted-foreground">No books found.</li>}
          </ul>
        </div>
      )}

      {kind === "file" && (
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            onClick={() =>
              void openDialog({
                multiple: false,
                filters: [{ name: "PDF or DjVu", extensions: ["pdf", "djvu", "djv"] }],
              }).then((p) => typeof p === "string" && setFile(p))
            }
          >
            Choose a file…
          </Button>
          <span className="min-w-0 truncate font-mono text-[12px] text-muted-foreground">
            {file ?? "No file chosen"}
          </span>
        </div>
      )}

      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button
          disabled={!request()}
          onClick={() => {
            const r = request();
            if (r) onCompare(r);
          }}
        >
          Compare
        </Button>
      </div>
    </Dialog>
  );
}
