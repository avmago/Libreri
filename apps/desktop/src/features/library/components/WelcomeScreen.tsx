import { FileText, Folder, FolderOpen, FolderPlus, Library, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useForgetRecentLibrary, useSettings } from "@/features/settings";
import { useLibraryActions } from "../hooks/useLibraryActions";

const TREE: { name: string; hint?: string; depth: number; file?: boolean }[] = [
  { name: "MyLibrary/", depth: 0 },
  { name: "Books/", hint: "books, audiobooks, your folders", depth: 1 },
  { name: "Notes/<profile>/", hint: "one .md per book", depth: 1 },
  { name: ".library-data/", hint: "managed by Libreri", depth: 1 },
  { name: "library.db", hint: "catalogue and search", depth: 2, file: true },
  { name: "thumbnails/  covers/", depth: 2 },
  { name: "metadata/", hint: "JSON backups", depth: 2 },
  { name: "annotations/", hint: "highlights, ink", depth: 2 },
];

export function WelcomeScreen() {
  const { createNew, openExisting, openPath, busy } = useLibraryActions();
  const { data: settings } = useSettings();
  const forget = useForgetRecentLibrary();
  const recent = settings?.recentLibraries ?? [];

  return (
    <main className="flex h-full items-center justify-center gap-14 overflow-auto bg-sidebar px-6 py-10">
      <section className="flex w-[440px] max-w-full flex-col gap-6">
        <div className="flex size-12 items-center justify-center rounded-xl bg-primary text-primary-foreground">
          <Library className="size-6" aria-hidden />
        </div>
        <div className="flex flex-col gap-2">
          <h1 className="text-[28px] font-semibold tracking-tight">Welcome to Libreri</h1>
          <p className="text-sm leading-relaxed text-muted-foreground">
            Your library is a normal folder on your disk. Pick where it lives — books stay as plain
            files you can back up, sync or open anywhere.
          </p>
        </div>
        <div className="flex flex-col gap-2.5">
          <Button size="lg" onClick={createNew} disabled={busy} className="justify-start">
            <FolderPlus /> Create a new library…
          </Button>
          <Button
            size="lg"
            variant="outline"
            onClick={openExisting}
            disabled={busy}
            className="justify-start"
          >
            <FolderOpen /> Open an existing library…
          </Button>
        </div>

        {recent.length > 0 && (
          <div className="flex flex-col gap-1.5">
            <h2 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
              RECENT LIBRARIES
            </h2>
            <ul className="overflow-hidden rounded-lg border bg-background">
              {recent.map((r) => (
                <li
                  key={r.path}
                  className="flex items-center gap-3 border-b px-3.5 py-2.5 last:border-b-0"
                >
                  <button
                    type="button"
                    disabled={!r.available || busy}
                    onClick={() => openPath(r.path)}
                    className="flex min-w-0 flex-1 items-center gap-3 text-left disabled:opacity-50"
                  >
                    <Library className="size-4 shrink-0" aria-hidden />
                    <span className="flex min-w-0 flex-col">
                      <span className="font-medium">{r.name}</span>
                      <span className="truncate font-mono text-[11px] text-muted-foreground">
                        {r.available ? r.path : `Not found · ${r.path}`}
                      </span>
                    </span>
                  </button>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={`Remove ${r.name} from recent libraries`}
                    onClick={() => forget.mutate(r.path)}
                  >
                    <X />
                  </Button>
                </li>
              ))}
            </ul>
          </div>
        )}
      </section>

      <aside className="hidden w-[400px] flex-col gap-3.5 rounded-xl border bg-background p-5 lg:flex">
        <h2 className="text-sm font-semibold">What gets created</h2>
        <ul className="flex flex-col font-mono text-xs leading-7">
          {TREE.map((n) => (
            <li
              key={n.name}
              className={`flex items-center gap-2 ${n.depth === 2 ? "text-muted-foreground" : ""}`}
              style={{ paddingLeft: n.depth * 18 }}
            >
              {n.file ? (
                <FileText className="size-3.5" aria-hidden />
              ) : (
                <Folder className="size-3.5" aria-hidden />
              )}
              <span className="flex-1">{n.name}</span>
              {n.hint && (
                <span className="font-sans text-[11px] text-muted-foreground">{n.hint}</span>
              )}
            </li>
          ))}
        </ul>
        <p className="rounded-md bg-muted px-3 py-2.5 text-xs leading-relaxed text-muted-foreground">
          Safe to keep in iCloud Drive, Dropbox or OneDrive — a lock file stops two computers
          editing at once.
        </p>
      </aside>
    </main>
  );
}
