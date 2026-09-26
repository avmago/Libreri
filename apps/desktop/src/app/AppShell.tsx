import { useCallback, useMemo } from "react";
import {
  BookOpen,
  Bookmark,
  CircleCheck,
  Folder,
  FolderOpen,
  FolderPlus,
  Headphones,
  Library,
  Moon,
  PanelLeft,
  Plus,
  Search,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { CommandPalette, type PaletteAction } from "@/features/command-palette";
import { LibraryHome, useCloseLibrary, useLibraryActions } from "@/features/library";
import { ThemeMenu, useSetTheme, useSettings } from "@/features/settings";
import type { LibrarySummary } from "@/lib/ipc";
import { DEFAULT_SHORTCUTS, useShortcut } from "@/lib/shortcuts";
import { nextTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { useUi } from "./ui-store";

const NAV = [
  { label: "All Books", Icon: Library, active: true },
  { label: "Currently Reading", Icon: BookOpen },
  { label: "Want to Read", Icon: Bookmark },
  { label: "Finished", Icon: CircleCheck },
  { label: "Audiobooks", Icon: Headphones },
];

export function AppShell({ library }: { library: LibrarySummary }) {
  const { paletteOpen, setPaletteOpen, sidebarCollapsed, toggleSidebar } = useUi();
  const { createNew, openExisting } = useLibraryActions();
  const closeLibrary = useCloseLibrary();
  const { data: settings } = useSettings();
  const setTheme = useSetTheme();

  const cycleTheme = useCallback(
    () => setTheme.mutate(nextTheme(settings?.theme ?? "system")),
    [setTheme, settings?.theme],
  );
  const openPalette = useCallback(() => setPaletteOpen(true), [setPaletteOpen]);
  const close = useCallback(() => closeLibrary.mutate(), [closeLibrary]);

  useShortcut("palette.open", openPalette);
  useShortcut("sidebar.toggle", toggleSidebar);
  useShortcut("theme.toggle", cycleTheme);
  useShortcut("library.close", close);

  const actions: PaletteAction[] = useMemo(
    () => [
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
    [createNew, openExisting, close, toggleSidebar, cycleTheme],
  );

  return (
    <div className="flex h-full flex-col">
      {/* Tab strip */}
      <div className="flex h-10 shrink-0 items-end gap-0.5 border-b bg-sidebar px-3">
        <div className="-mb-px flex h-8 items-center gap-2 rounded-t-lg border border-b-background bg-background px-3.5 font-medium">
          <Library className="size-4" aria-hidden /> Library
        </div>
        <Button variant="ghost" size="icon" aria-label="New tab" disabled className="mb-0.5">
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
        {/* Sidebar */}
        <nav
          aria-label="Library"
          className={cn(
            "flex shrink-0 flex-col gap-4 border-r bg-sidebar px-2.5 pt-3 transition-[width]",
            sidebarCollapsed ? "w-14 items-center px-2" : "w-58",
          )}
        >
          <Button
            variant="ghost"
            size="icon"
            aria-label="Show or hide sidebar"
            onClick={toggleSidebar}
            className="self-start"
          >
            <PanelLeft />
          </Button>
          {!sidebarCollapsed && (
            <>
              <div className="flex flex-col gap-px">
                <h2 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                  LIBRARY
                </h2>
                {NAV.map(({ label, Icon, active }) => (
                  <div
                    key={label}
                    className={cn(
                      "flex h-7 items-center gap-2.5 rounded-md px-2.5",
                      active && "bg-muted font-medium",
                    )}
                  >
                    <Icon className="size-4" aria-hidden />
                    <span className="flex-1">{label}</span>
                    <span className="text-[11px] text-muted-foreground">
                      {active ? library.bookCount : 0}
                    </span>
                  </div>
                ))}
              </div>
              <div className="flex flex-col gap-px">
                <h2 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                  FOLDERS
                </h2>
                <div className="flex h-7 items-center gap-2.5 px-2.5 text-muted-foreground">
                  <Folder className="size-4" aria-hidden /> No folders yet
                </div>
              </div>
            </>
          )}
          <div className="flex-1" />
          <div
            className={cn(
              "-mx-2.5 flex items-center gap-2.5 border-t px-4 py-2.5",
              sidebarCollapsed && "mx-0 justify-center px-0",
            )}
          >
            <Library className="size-4 shrink-0" aria-hidden />
            {!sidebarCollapsed && (
              <span className="flex min-w-0 flex-col">
                <span className="truncate font-medium">{library.name}</span>
                <span className="truncate font-mono text-[10.5px] text-muted-foreground">
                  {library.path}
                </span>
              </span>
            )}
          </div>
        </nav>

        <main className="min-w-0 flex-1">
          <LibraryHome library={library} />
        </main>
      </div>

      <CommandPalette open={paletteOpen} onOpenChange={setPaletteOpen} actions={actions} />
    </div>
  );
}
