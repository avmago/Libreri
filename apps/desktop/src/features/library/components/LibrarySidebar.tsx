import { useState } from "react";
import {
  AlertTriangle,
  BookOpen,
  Bookmark,
  CircleCheck,
  FolderPlus,
  Headphones,
  Heart,
  Library,
  X,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { commands, type LibrarySummary } from "@/lib/ipc";
import { useShortcut } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";
import { useFacets, useFolders } from "../api";
import { useDrag } from "../drag";
import { useJobs } from "../hooks/useLibraryEvents";
import { useLibraryView, type Nav } from "../store";
import { FolderTree, type FolderEditing } from "./FolderTree";

function same(a: Nav, b: Nav) {
  return JSON.stringify(a) === JSON.stringify(b);
}

function NavItem({
  nav,
  label,
  Icon,
  count,
}: {
  nav: Nav;
  label: string;
  Icon: LucideIcon;
  count: number;
}) {
  const { nav: current, setNav } = useLibraryView();
  const active = same(current, nav);
  return (
    <button
      type="button"
      onClick={() => setNav(nav)}
      aria-current={active ? "page" : undefined}
      className={cn(
        "flex h-7 items-center gap-2.5 rounded-md px-2.5 text-left",
        active ? "bg-muted font-medium" : "hover:bg-muted/60",
      )}
    >
      <Icon className="size-4" aria-hidden />
      <span className="flex-1 truncate">{label}</span>
      <span className="text-[11px] text-muted-foreground">{count || ""}</span>
    </button>
  );
}

function JobsStrip() {
  // Quick checks for changes finish before they have anything to show.
  const jobs = Object.values(useJobs((s) => s.jobs)).filter((j) => j.total > 0);
  if (!jobs.length) return null;
  return (
    <div className="flex flex-col gap-2 border-t px-3 py-2.5" aria-live="polite">
      {jobs.map((j) => {
        const pct = j.total ? Math.round((j.done / j.total) * 100) : null;
        return (
          <div key={j.id} className="flex flex-col gap-1">
            <div className="flex items-center gap-2 text-[12px]">
              <span className="flex-1 truncate font-medium">{j.label}</span>
              {j.total > 0 && (
                <span className="text-muted-foreground tabular-nums">
                  {j.done}/{j.total}
                </span>
              )}
              <button
                type="button"
                aria-label="Cancel"
                onClick={() => void commands.cancelJob(j.id)}
                className="rounded p-0.5 text-muted-foreground hover:bg-muted hover:text-foreground"
              >
                <X className="size-3" />
              </button>
            </div>
            <div className="h-1 overflow-hidden rounded-full bg-muted">
              <div
                className={cn(
                  "h-full rounded-full bg-primary transition-[width]",
                  pct === null && "w-1/3 animate-pulse",
                )}
                style={pct === null ? undefined : { width: `${pct}%` }}
              />
            </div>
            {j.message && (
              <span className="truncate font-mono text-[10.5px] text-muted-foreground">
                {j.message}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}

export function LibrarySidebar({
  library,
  collapsed,
}: {
  library: LibrarySummary;
  collapsed: boolean;
}) {
  const { data: facets } = useFacets();
  const { data: folders = [] } = useFolders();
  const [editing, setEditing] = useState<FolderEditing>(null);
  const overRoot = useDrag((s) => s.item !== null && s.target === "");
  useShortcut("library.newFolder", () => {
    const { nav } = useLibraryView.getState();
    setEditing({ mode: "create", parent: nav.kind === "folder" ? nav.path : "" });
    if (nav.kind === "folder") useLibraryView.getState().setExpanded(nav.path, true);
  });

  if (collapsed) {
    return (
      <div className="mt-2 flex flex-col items-center gap-1">
        <Library className="size-4 text-muted-foreground" aria-hidden />
      </div>
    );
  }

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-2.5 pb-3">
        <div className="flex flex-col gap-px">
          <h2 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
            LIBRARY
          </h2>
          <NavItem
            nav={{ kind: "all" }}
            label="All Books"
            Icon={Library}
            count={facets?.total ?? 0}
          />
          <NavItem
            nav={{ kind: "status", status: "reading" }}
            label="Currently Reading"
            Icon={BookOpen}
            count={facets?.reading ?? 0}
          />
          <NavItem
            nav={{ kind: "status", status: "wantToRead" }}
            label="Want to Read"
            Icon={Bookmark}
            count={facets?.wantToRead ?? 0}
          />
          <NavItem
            nav={{ kind: "status", status: "finished" }}
            label="Finished"
            Icon={CircleCheck}
            count={facets?.finished ?? 0}
          />
          <NavItem
            nav={{ kind: "favorites" }}
            label="Favourites"
            Icon={Heart}
            count={facets?.favorites ?? 0}
          />
          <NavItem
            nav={{ kind: "audio" }}
            label="Audiobooks"
            Icon={Headphones}
            count={facets?.audio ?? 0}
          />
          {(facets?.missing ?? 0) > 0 && (
            <NavItem
              nav={{ kind: "missing" }}
              label="Missing files"
              Icon={AlertTriangle}
              count={facets?.missing ?? 0}
            />
          )}
        </div>
        <div className="flex flex-col gap-px">
          <div
            data-drop-folder=""
            className={cn(
              "flex h-6 items-center justify-between rounded-md pr-1 pl-2.5",
              overRoot && "bg-muted ring-2 ring-primary ring-inset",
            )}
          >
            <h2 className="text-[11px] font-semibold tracking-wide text-muted-foreground">
              FOLDERS
            </h2>
            <Button
              variant="ghost"
              size="icon"
              className="size-6"
              aria-label="New folder"
              title="New folder"
              onClick={() => setEditing({ mode: "create", parent: "" })}
            >
              <FolderPlus className="!size-3.5" />
            </Button>
          </div>
          <FolderTree folders={folders} editing={editing} setEditing={setEditing} />
        </div>
      </div>
      <JobsStrip />
      <div className="flex items-center gap-2.5 border-t px-4 py-2.5">
        <Library className="size-4 shrink-0" aria-hidden />
        <span className="flex min-w-0 flex-col">
          <span className="truncate font-medium">{library.name}</span>
          <span
            className="truncate font-mono text-[10.5px] text-muted-foreground"
            title={library.path}
          >
            {library.path}
          </span>
        </span>
      </div>
    </div>
  );
}
