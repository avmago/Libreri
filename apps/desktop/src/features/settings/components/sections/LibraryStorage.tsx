import { FolderOpen, HeartPulse, RefreshCw, Wrench } from "lucide-react";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { useQuery } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { useFacets } from "@/features/library";
import { usePortability } from "@/features/portability";
import { commands, unwrap, type LibrarySummary, type SessionDto } from "@/lib/ipc";
import { Group, Row } from "../parts";

function size(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = bytes / 1024;
  let i = 0;
  while (v >= 1024 && i < units.length - 1) {
    v /= 1024;
    i++;
  }
  return `${v < 10 ? v.toFixed(1) : Math.round(v)} ${units[i]}`;
}

export function LibrarySettings({
  library,
  session,
}: {
  library: LibrarySummary;
  session: SessionDto;
}) {
  const { data: facets } = useFacets();
  const setHealth = usePortability((s) => s.setHealth);
  const { data: pageCache, refetch: refetchCache } = useQuery({
    queryKey: ["page-cache"],
    queryFn: () => unwrap(commands.pageCacheSize()),
    staleTime: 30_000,
  });
  const { data: storage } = useQuery({
    queryKey: ["lib", "storage"],
    queryFn: () => unwrap(commands.libraryStorage()),
    staleTime: 30_000,
  });
  const rebuild = async () => {
    const ok = await ask(
      "Libreri rebuilds its catalogue from the book files and the JSON backups in .library-data. Your books and notes are not touched; the old catalogue is kept as library.db.bak.",
      { title: "Rebuild the library index?", okLabel: "Rebuild" },
    );
    if (ok) {
      const r = await commands.rebuildLibraryIndex();
      if (r.status === "error") toast.error(r.error.message);
      else toast("Rebuilding the library index…");
    }
  };
  return (
    <>
      <Group title="This library" scope="library">
        <Row label="Name">
          <span className="font-medium">{library.name}</span>
        </Row>
        <Row
          label="Folder"
          help={<span className="font-mono text-[11.5px] break-all">{library.path}</span>}
        >
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              void commands
                .revealFolder("")
                .then((r) => r.status === "error" && toast.error(r.error.message))
            }
          >
            <FolderOpen /> Show
          </Button>
        </Row>
        <Row label="Books">
          <span className="tabular-nums">{facets?.total ?? library.bookCount}</span>
        </Row>
      </Group>
      <Group
        title="Storage"
        scope="library"
        description="Safe to keep in iCloud Drive, Dropbox or OneDrive: a lock file stops two computers from changing it at once."
      >
        <Row label="Books/" help="Your book files and folders, exactly as on disk.">
          <span className="tabular-nums">{storage ? size(storage.books ?? 0) : "…"}</span>
        </Row>
        <Row label="Notes/" help="Everyone's Markdown notebooks, one folder per profile.">
          <span className="tabular-nums">{storage ? size(storage.notes ?? 0) : "…"}</span>
        </Row>
        <Row
          label="Page cache (this computer)"
          help="Pages of comics and DjVu books, kept to open them quickly. Up to 2 GB; made again when needed."
        >
          <span className="tabular-nums">{pageCache !== undefined ? size(pageCache ?? 0) : "…"}</span>
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              void commands.clearPageCache().then(() => {
                toast("Page cache cleared");
                void refetchCache();
              })
            }
          >
            Clear
          </Button>
        </Row>
        <Row
          label=".library-data/"
          help="Catalogue, covers, and JSON backups of details, profiles and highlights."
        >
          <span className="tabular-nums">{storage ? size(storage.data ?? 0) : "…"}</span>
        </Row>
      </Group>
      <Group title="Maintenance" scope="library">
        <Row
          label="Check the folder for changes"
          help="Libreri also does this by itself while it runs."
        >
          <Button variant="outline" size="sm" onClick={() => void commands.rescanLibrary()}>
            <RefreshCw /> Check now
          </Button>
        </Row>
        {session.canEditLibrary && (
          <Row
            label="Check library health"
            help="Missing files, broken links in your notes, missing backups of details."
          >
            <Button variant="outline" size="sm" onClick={() => setHealth(true)}>
              <HeartPulse /> Check…
            </Button>
          </Row>
        )}
        {session.canEditLibrary && (
          <Row
            label="Rebuild the library index"
            help="Rebuilds the catalogue from the files and JSON backups, if something looks wrong."
          >
            <Button variant="outline" size="sm" onClick={() => void rebuild()}>
              <Wrench /> Rebuild…
            </Button>
          </Row>
        )}
      </Group>
    </>
  );
}
