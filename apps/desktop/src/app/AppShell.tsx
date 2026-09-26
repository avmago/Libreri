import { useCallback, useMemo } from "react";
import {
  FileUp,
  FolderOpen,
  FolderPlus,
  FolderUp,
  Info,
  LayoutGrid,
  Library,
  List,
  Moon,
  PanelLeft,
  Plus,
  RefreshCw,
  Search,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { CommandPalette, type PaletteAction } from "@/features/command-palette";
import {
  DropOverlay,
  ImportDialog,
  LibrarySidebar,
  LibraryView,
  pickFilesToImport,
  pickFolderToImport,
  useCloseLibrary,
  useDesktopDrop,
  useLibraryActions,
  useLibraryEvents,
  useLibraryView,
} from "@/features/library";
import { ThemeMenu, useSetTheme, useSettings } from "@/features/settings";
import { commands, type LibrarySummary } from "@/lib/ipc";
import { DEFAULT_SHORTCUTS, useShortcut } from "@/lib/shortcuts";
import { nextTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { useUi } from "./ui-store";

export function AppShell({ library }: { library: LibrarySummary }) {
  const { paletteOpen, setPaletteOpen, sidebarCollapsed, toggleSidebar } = useUi();
  const { createNew, openExisting } = useLibraryActions();
  const closeLibrary = useCloseLibrary();
  const { data: settings } = useSettings();
  const setTheme = useSetTheme();
  const { setView, toggleDetails } = useLibraryView();
  useLibraryEvents();
  useDesktopDrop();

  const cycleTheme = useCallback(
    () => setTheme.mutate(nextTheme(settings?.theme ?? "system")),
    [setTheme, settings?.theme],
  );
  const openPalette = useCallback(() => setPaletteOpen(true), [setPaletteOpen]);
  const close = useCallback(() => closeLibrary.mutate(), [closeLibrary]);
  const rebuild = useCallback(async () => {
    const ok = await ask(
      "Libreri rebuilds its catalogue from the book files and the JSON backups in .library-data. Your books and notes are not touched; the old catalogue is kept as library.db.bak.",
      { title: "Rebuild the library index?", okLabel: "Rebuild" },
    );
    if (ok) {
      commands
        .rebuildLibraryIndex()
        .then((r) => r.status === "error" && toast.error(r.error.message));
    }
  }, []);

  useShortcut("palette.open", openPalette);
  useShortcut("sidebar.toggle", toggleSidebar);
  useShortcut("theme.toggle", cycleTheme);
  useShortcut("library.close", close);

  const actions: PaletteAction[] = useMemo(
    () => [
      {
        id: "books.import",
        group: "Books",
        label: "Import files…",
        icon: FileUp,
        shortcut: DEFAULT_SHORTCUTS["library.import"],
        run: () => void pickFilesToImport(),
      },
      {
        id: "books.importFolder",
        group: "Books",
        label: "Import a folder…",
        icon: FolderUp,
        shortcut: DEFAULT_SHORTCUTS["library.importFolder"],
        run: () => void pickFolderToImport(),
      },
      {
        id: "books.rescan",
        group: "Books",
        label: "Check the library folder for changes",
        icon: RefreshCw,
        shortcut: DEFAULT_SHORTCUTS["library.refresh"],
        run: () => void commands.rescanLibrary(),
      },
      {
        id: "books.rebuild",
        group: "Books",
        label: "Rebuild the library index…",
        icon: RefreshCw,
        run: () => void rebuild(),
      },
      {
        id: "lib.new",
        group: "Library",
        label: "Create a new library…",
        icon: FolderPlus,
        run: createNew,
      },
      {
        id: "lib.open",
        group: "Library",
        label: "Open another library…",
        icon: FolderOpen,
        run: openExisting,
      },
      {
        id: "lib.close",
        group: "Library",
        label: "Close library",
        icon: X,
        shortcut: DEFAULT_SHORTCUTS["library.close"],
        run: close,
      },
      {
        id: "view.grid",
        group: "View",
        label: "Show as grid",
        icon: LayoutGrid,
        shortcut: DEFAULT_SHORTCUTS["view.grid"],
        run: () => setView("grid"),
      },
      {
        id: "view.list",
        group: "View",
        label: "Show as list",
        icon: List,
        shortcut: DEFAULT_SHORTCUTS["view.list"],
        run: () => setView("list"),
      },
      {
        id: "view.details",
        group: "View",
        label: "Show or hide details",
        icon: Info,
        shortcut: DEFAULT_SHORTCUTS["details.toggle"],
        run: toggleDetails,
      },
      {
        id: "view.sidebar",
        group: "View",
        label: "Show or hide sidebar",
        icon: PanelLeft,
        shortcut: DEFAULT_SHORTCUTS["sidebar.toggle"],
        run: toggleSidebar,
      },
      {
        id: "view.theme",
        group: "View",
        label: "Switch theme",
        icon: Moon,
        shortcut: DEFAULT_SHORTCUTS["theme.toggle"],
        run: cycleTheme,
      },
    ],
    [createNew, openExisting, close, toggleSidebar, cycleTheme, rebuild, setView, toggleDetails],
  );

  return (
    <div className="flex h-full flex-col">
      {/* Tab strip */}
      <div className="flex h-10 shrink-0 items-end gap-0.5 border-b bg-sidebar px-3">
        <div className="-mb-px flex h-8 items-center gap-2 rounded-t-lg border border-b-background bg-background px-3.5 font-medium">
          <Library className="size-4" aria-hidden /> Library
        </div>
        <Button
          variant="ghost"
          size="icon"
          aria-label="New tab"
          disabled
          title="Reader tabs arrive with the reader"
          className="mb-0.5"
        >
          <Plus />
        </Button>
        <div className="flex-1" />
        <button
          type="button"
          onClick={openPalette}
          className="mb-1.5 flex h-7 w-72 items-center gap-2 rounded-md border bg-background px-2.5 text-muted-foreground"
        >
          <Search className="size-3.5" aria-hidden />
          <span className="flex-1 text-left">Search or run a command…</span>
          <Kbd shortcut={DEFAULT_SHORTCUTS["palette.open"]} />
        </button>
        <div className="mb-1 ml-1">
          <ThemeMenu />
        </div>
      </div>

      <div className="flex min-h-0 flex-1">
        <nav
          aria-label="Library"
          className={cn(
            "flex shrink-0 flex-col gap-2 border-r bg-sidebar pt-2 transition-[width]",
            sidebarCollapsed ? "w-14 items-center" : "w-60",
          )}
        >
          <div className={cn("px-2.5", sidebarCollapsed && "px-0")}>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Show or hide sidebar"
              onClick={toggleSidebar}
            >
              <PanelLeft />
            </Button>
          </div>
          <LibrarySidebar library={library} collapsed={sidebarCollapsed} />
        </nav>

        <main className="min-w-0 flex-1">
          <LibraryView />
        </main>
      </div>

      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} actions={actions} />
      <ImportDialog />
      <DropOverlay />
    </div>
  );
}
