import { useState } from "react";
import { LibreriMark } from "@/components/LibreriMark";
import {
  AlertTriangle,
  CalendarDays,
  Bookmark,
  BookOpen,
  ChevronRight,
  CircleCheck,
  FileSearch,
  Folder,
  FolderPlus,
  Headphones,
  Heart,
  Library,
  NotebookText,
  Rss,
  Podcast,
  Pencil,
  Plus,
  Shapes,
  Sparkles,
  Tags,
  Trash2,
  type LucideIcon,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { ContextMenu, menuContent, menuItem } from "@/components/ui/menu";
import { useFeedsOverview } from "@/features/feeds";
import { usePodcastsBadge } from "@/features/podcasts";
import { usePermissions } from "@/features/profiles";
import { commands, type FacetsDto, type LibrarySummary } from "@/lib/ipc";
import { useShortcut } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";
import {
  useCollections,
  useDeleteCollection,
  useFacets,
  useFolders,
  useSaveCollection,
} from "../api";
import { useLibraryDialogs } from "../dialogs";
import { useDrag } from "../drag";
import { useJobs } from "../hooks/useLibraryEvents";
import { useLibraryView, type Nav } from "../store";
import { FolderTree, type FolderEditing } from "./FolderTree";

function same(a: Nav, b: Nav) {
  if (a.kind === "search" && b.kind === "search") return true;
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

/** Feeds, with how many items are new. */
function FeedsNavItem() {
  const { data } = useFeedsOverview();
  return <NavItem nav={{ kind: "feeds" }} label="Feeds" Icon={Rss} count={data?.unread ?? 0} />;
}

/** Podcasts, with how many new episodes there are. */
function PodcastsNavItem() {
  const count = usePodcastsBadge();
  return <NavItem nav={{ kind: "podcasts" }} label="Podcasts" Icon={Podcast} count={count} />;
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

const LIBRARY_NAV: {
  nav: Nav;
  label: string;
  Icon: LucideIcon;
  count: (f: FacetsDto) => number;
}[] = [
  { nav: { kind: "all" }, label: "All Books", Icon: Library, count: (f) => f.total },
  {
    nav: { kind: "status", status: "reading" },
    label: "Currently Reading",
    Icon: BookOpen,
    count: (f) => f.reading,
  },
  {
    nav: { kind: "status", status: "wantToRead" },
    label: "Want to Read",
    Icon: Bookmark,
    count: (f) => f.wantToRead,
  },
  {
    nav: { kind: "status", status: "finished" },
    label: "Finished",
    Icon: CircleCheck,
    count: (f) => f.finished,
  },
  { nav: { kind: "favorites" }, label: "Favourites", Icon: Heart, count: (f) => f.favorites },
  { nav: { kind: "audio" }, label: "Audiobooks", Icon: Headphones, count: (f) => f.audio },
];

function SectionTitle({
  children,
  action,
}: {
  children: React.ReactNode;
  action?: React.ReactNode;
}) {
  return (
    <div className="flex h-6 items-center justify-between pr-1 pl-2.5">
      <h2 className="text-[11px] font-semibold tracking-wide text-muted-foreground">{children}</h2>
      {action}
    </div>
  );
}

/** The narrow sidebar: icons only, with the name as a tooltip. */
function Rail() {
  const { data: facets } = useFacets();
  const { nav: current, setNav } = useLibraryView();
  const { editLibrary, keepsData } = usePermissions();
  const items: { nav: Nav; label: string; Icon: LucideIcon }[] = [
    ...LIBRARY_NAV,
    ...(facets?.missing
      ? [{ nav: { kind: "missing" } as Nav, label: "Missing files", Icon: AlertTriangle }]
      : []),
    { nav: { kind: "search" }, label: "Search", Icon: FileSearch },
    { nav: { kind: "notes" }, label: "Notes", Icon: NotebookText },
    ...(keepsData ? [{ nav: { kind: "feeds" } as Nav, label: "Feeds", Icon: Rss }] : []),
    ...(keepsData ? [{ nav: { kind: "podcasts" } as Nav, label: "Podcasts", Icon: Podcast }] : []),
    ...(keepsData
      ? [{ nav: { kind: "calendar" } as Nav, label: "Calendar", Icon: CalendarDays }]
      : []),
    ...(editLibrary ? [{ nav: { kind: "organize" } as Nav, label: "Organize", Icon: Tags }] : []),
    { nav: { kind: "folder", path: "" }, label: "Folders", Icon: Folder },
  ];
  return (
    <div className="mt-1 flex flex-col items-center gap-1">
      {items.map(({ nav, label, Icon }) => {
        const active = same(current, nav) || (nav.kind === "folder" && current.kind === "folder");
        return (
          <button
            key={label}
            type="button"
            aria-label={label}
            title={label}
            aria-current={active ? "page" : undefined}
            onClick={() => setNav(nav)}
            className={cn(
              "flex size-9 items-center justify-center rounded-md",
              active
                ? "bg-muted text-foreground"
                : "text-muted-foreground hover:bg-muted/60 hover:text-foreground",
            )}
          >
            <Icon className="size-4" aria-hidden />
          </button>
        );
      })}
    </div>
  );
}

function Collections() {
  const { data: collections = [] } = useCollections();
  const { nav, setNav } = useLibraryView();
  const del = useDeleteCollection();
  const save = useSaveCollection();
  const [renaming, setRenaming] = useState<string | null>(null);
  const setSaveCollection = useLibraryDialogs((s) => s.setSaveCollection);
  return (
    <div className="flex flex-col gap-px">
      <SectionTitle
        action={
          <Button
            variant="ghost"
            size="icon"
            className="size-6"
            aria-label="Save the current search as a smart collection"
            title="Save the current search as a smart collection"
            onClick={() => setSaveCollection(true)}
          >
            <Plus className="!size-3.5" />
          </Button>
        }
      >
        COLLECTIONS
      </SectionTitle>
      {collections.length === 0 && (
        <p className="px-2.5 py-1 text-[12px] leading-snug text-muted-foreground">
          Search or filter, then save it here to come back to those books any time.
        </p>
      )}
      {collections.map((c) =>
        renaming === c.id ? (
          <input
            key={c.id}
            defaultValue={c.name}
            autoFocus
            aria-label="Collection name"
            className="mx-1 h-7 rounded-md border border-ring bg-background px-2 text-[13px] outline-none"
            onBlur={(e) => {
              setRenaming(null);
              const name = e.target.value.trim();
              if (name && name !== c.name) save.mutate({ id: c.id, name, query: c.query });
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") e.currentTarget.blur();
              if (e.key === "Escape") setRenaming(null);
            }}
          />
        ) : (
          <ContextMenu.Root key={c.id}>
            <ContextMenu.Trigger asChild>
              <button
                type="button"
                onClick={() =>
                  setNav({ kind: "collection", id: c.id, name: c.name, query: c.query })
                }
                aria-current={nav.kind === "collection" && nav.id === c.id ? "page" : undefined}
                className={cn(
                  "flex h-7 items-center gap-2.5 rounded-md px-2.5 text-left",
                  nav.kind === "collection" && nav.id === c.id
                    ? "bg-muted font-medium"
                    : "hover:bg-muted/60",
                )}
              >
                <Sparkles className="size-4 text-muted-foreground" aria-hidden />
                <span className="flex-1 truncate">{c.name}</span>
              </button>
            </ContextMenu.Trigger>
            <ContextMenu.Portal>
              <ContextMenu.Content className={menuContent}>
                <ContextMenu.Item className={menuItem} onSelect={() => setRenaming(c.id)}>
                  <Pencil /> Rename
                </ContextMenu.Item>
                <ContextMenu.Item
                  className={cn(menuItem, "text-destructive [&_svg]:!text-destructive")}
                  onSelect={() => {
                    del.mutate(c.id);
                    if (nav.kind === "collection" && nav.id === c.id) setNav({ kind: "all" });
                  }}
                >
                  <Trash2 /> Remove collection
                </ContextMenu.Item>
              </ContextMenu.Content>
            </ContextMenu.Portal>
          </ContextMenu.Root>
        ),
      )}
    </div>
  );
}

function Categories() {
  const { data: facets } = useFacets();
  const { nav, setNav } = useLibraryView();
  const [open, setOpen] = useState(false);
  const top = new Map<string, number>();
  for (const { value, count } of facets?.categories ?? []) {
    const root = value.split("/")[0]!;
    top.set(root, (top.get(root) ?? 0) + count);
  }
  if (top.size === 0) return null;
  return (
    <div className="flex flex-col gap-px">
      <button
        type="button"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
        className="flex h-6 items-center gap-1 pl-2.5 text-left text-[11px] font-semibold tracking-wide text-muted-foreground"
      >
        CATEGORIES
        <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
      </button>
      {open &&
        [...top.entries()].map(([name, count]) => {
          const active = nav.kind === "category" && nav.path === name;
          return (
            <button
              key={name}
              type="button"
              onClick={() => setNav({ kind: "category", path: name })}
              aria-current={active ? "page" : undefined}
              className={cn(
                "flex h-7 items-center gap-2.5 rounded-md px-2.5 text-left",
                active ? "bg-muted font-medium" : "hover:bg-muted/60",
              )}
            >
              <Shapes className="size-4 text-muted-foreground" aria-hidden />
              <span className="flex-1 truncate">{name}</span>
              <span className="text-[11px] text-muted-foreground">{count}</span>
            </button>
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
  const { editLibrary, keepsData } = usePermissions();
  const overRoot = useDrag((s) => s.item !== null && s.target === "");
  useShortcut("library.newFolder", () => {
    if (!editLibrary) return;
    const { nav } = useLibraryView.getState();
    setEditing({ mode: "create", parent: nav.kind === "folder" ? nav.path : "" });
    if (nav.kind === "folder") useLibraryView.getState().setExpanded(nav.path, true);
  });

  if (collapsed) return <Rail />;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex min-h-0 flex-1 flex-col gap-4 overflow-y-auto px-2.5 pb-3">
        <div className="flex flex-col gap-px">
          <SectionTitle>LIBRARY</SectionTitle>
          {LIBRARY_NAV.map((n) => (
            <NavItem
              key={n.label}
              nav={n.nav}
              label={n.label}
              Icon={n.Icon}
              count={facets ? n.count(facets) : 0}
            />
          ))}
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
          <SectionTitle>YOURS</SectionTitle>
          <NavItem nav={{ kind: "search" }} label="Search" Icon={FileSearch} count={0} />
          <NavItem nav={{ kind: "notes" }} label="Notes" Icon={NotebookText} count={0} />
          {keepsData && <FeedsNavItem />}
          {keepsData && <PodcastsNavItem />}
          {keepsData && (
            <NavItem nav={{ kind: "calendar" }} label="Calendar" Icon={CalendarDays} count={0} />
          )}
          {editLibrary && (
            <NavItem nav={{ kind: "organize" }} label="Organize" Icon={Tags} count={0} />
          )}
        </div>
        {keepsData && <Collections />}
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
            {editLibrary && (
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
            )}
          </div>
          <FolderTree folders={folders} editing={editing} setEditing={setEditing} />
        </div>
        <Categories />
      </div>
      <JobsStrip />
      <div className="flex items-center gap-2.5 border-t px-4 py-2.5">
        <LibreriMark className="size-5 shrink-0" />
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
