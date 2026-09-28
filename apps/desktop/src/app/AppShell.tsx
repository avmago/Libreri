import { useCallback, useEffect, useMemo, useState } from "react";
import {
  Archive,
  BookOpen,
  Columns2,
  Download,
  HeartPulse,
  Quote,
  Upload,
  FileUp,
  FolderOpen,
  FolderPlus,
  FolderUp,
  Globe,
  Info,
  Keyboard,
  LayoutGrid,
  Library,
  LibrarySquare,
  List,
  Lock,
  Moon,
  NotebookText,
  FileSearch,
  PanelLeft,
  RefreshCw,
  Search,
  Settings,
  Tags,
  UserRound,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { CommandPalette, type PaletteAction } from "@/features/command-palette";
import {
  BulkEditDialog,
  DropOverlay,
  ImportDialog,
  LibrarySidebar,
  LibraryView,
  SaveCollectionDialog,
  pickFilesToImport,
  pickFolderToImport,
  useCloseLibrary,
  useDesktopDrop,
  useLibraryActions,
  useLibraryDialogs,
  useLibraryEvents,
  useLibraryView,
} from "@/features/library";
import { FindDetailsDialog, useDetailsEvents } from "@/features/details";
import { HelperDialog } from "@/features/helpers";
import { NotesHub } from "@/features/notes";
import { OcrDialog, SearchView, useSearchEvents } from "@/features/search";
import { setSpeechSettingsOpener } from "@/features/speech";
import {
  CitationDialog,
  ExportDialog,
  ForeignImportDialog,
  HealthDialog,
  ImportArchiveDialog,
  pickArchiveToImport,
  usePortability,
  usePortabilityEvents,
} from "@/features/portability";
import { OrganizeView } from "@/features/organize";
import {
  ProfileMenu,
  flushPrefs,
  useAutoLock,
  useProfilePrefs,
  useSignOut,
} from "@/features/profiles";
import { ReaderView, flushSession, useSession } from "@/features/reader";
import {
  SettingsPage,
  ShortcutsSheet,
  ThemeMenu,
  useSetTheme,
  useSettings,
} from "@/features/settings";
import { commands, type LibrarySummary, type SessionDto } from "@/lib/ipc";
import {
  ShortcutScope,
  applyShortcutPrefs,
  runAction,
  shortcutFor,
  useShortcut,
  type ActionId,
} from "@/lib/shortcuts";
import { useTabs } from "@/lib/tabs";
import { isMainWindow } from "@/lib/windows";
import { nextTheme } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { TabStrip } from "./TabStrip";
import { useUi } from "./ui-store";
import { useWindowActions } from "./windows";

export function AppShell({ library, session }: { library: LibrarySummary; session: SessionDto }) {
  // This profile's preferences, loaded before anything below renders.
  useState(() => useProfilePrefs.getState().load(session.prefs, session.keepsData));
  const prefs = useProfilePrefs((s) => s.prefs);
  const updatePrefs = useProfilePrefs((s) => s.update);
  useEffect(
    () => applyShortcutPrefs(prefs.shortcuts, prefs.vimKeys),
    [prefs.shortcuts, prefs.vimKeys],
  );
  useState(() => {
    useLibraryView.getState().setView(prefs.library.view);
    useUi.getState().setSidebarCollapsed(prefs.library.sidebarCollapsed);
  });

  const ui = useUi();
  const { createNew, openExisting } = useLibraryActions();
  const closeLibrary = useCloseLibrary();
  const signOut = useSignOut();
  const { data: settings } = useSettings();
  const setTheme = useSetTheme();
  const libraryView = useLibraryView();
  const { setView, toggleDetails, nav, setNav } = libraryView;
  useLibraryEvents();
  useDetailsEvents();
  usePortabilityEvents();
  useSearchEvents();
  const setHealth = usePortability((s) => s.setHealth);
  const showMissingRequest = usePortability((s) => s.showMissingRequest);
  const openExport = usePortability((s) => s.openExport);
  const openForeign = usePortability((s) => s.openForeign);
  const isOwner = session.profile.kind === "owner";
  const backUpNow = useCallback(async () => {
    const r = await commands.backUpNow();
    if (r.status === "error") {
      toast.error(r.error.message, {
        action: { label: "Settings", onClick: () => useUi.getState().openSettings("export") },
      });
    } else toast(r.data ? "Backing up…" : "A backup is already running");
  }, []);
  useDesktopDrop(session.canEditLibrary);
  useSession(`${library.id}:${session.profile.id}`);
  const tabs = useTabs();
  const { tabs: openTabs, active: activeTab, split } = tabs;
  const windows = useWindowActions();

  // Readers are created the first time their tab is shown, then kept.
  const [visited, setVisited] = useState<Set<string>>(() => new Set());
  const shown = [activeTab, split?.left, split?.right].filter((x): x is string => Boolean(x));
  if (shown.some((id) => !visited.has(id))) setVisited(new Set([...visited, ...shown]));

  // A window opened for one book closes when its last book tab closes.
  useEffect(() => {
    if (isMainWindow || openTabs.length > 0 || !visited.size) return;
    void getCurrentWindow().close();
  }, [openTabs.length, visited.size]);

  // Remember the library view and sidebar for this profile.
  useEffect(() => {
    if (prefs.library.view !== libraryView.view)
      updatePrefs({ library: { view: libraryView.view } });
  }, [libraryView.view]); // eslint-disable-line react-hooks/exhaustive-deps
  useEffect(() => {
    if (prefs.library.sidebarCollapsed !== ui.sidebarCollapsed) {
      updatePrefs({ library: { sidebarCollapsed: ui.sidebarCollapsed } });
    }
  }, [ui.sidebarCollapsed]); // eslint-disable-line react-hooks/exhaustive-deps

  const lock = useCallback(async () => {
    await Promise.allSettled([flushSession(), flushPrefs()]);
    signOut.mutate();
  }, [signOut]);
  useAutoLock(session.profile.hasPin ? prefs.autoLockMinutes : 0, () => void lock());

  const cycleTheme = useCallback(
    () => setTheme.mutate(nextTheme(settings?.theme ?? "system")),
    [setTheme, settings?.theme],
  );
  const close = useCallback(async () => {
    await Promise.allSettled([flushSession(), flushPrefs()]);
    closeLibrary.mutate();
  }, [closeLibrary]);
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
  const goHome = useCallback(
    (kind: "all" | "notes" | "organize" | "search", query?: string) => {
      tabs.activate(null);
      ui.closeSettings();
      if (kind === "all") {
        if (nav.kind === "notes" || nav.kind === "organize" || nav.kind === "search")
          setNav({ kind: "all" });
      } else if (kind === "search") setNav({ kind: "search", query });
      else setNav({ kind });
    },
    [tabs, ui, nav.kind, setNav],
  );

  // "Show missing files" from the health check or an import report.
  useEffect(() => {
    if (!showMissingRequest) return;
    tabs.activate(null);
    ui.closeSettings();
    setNav({ kind: "missing" });
  }, [showMissingRequest]); // eslint-disable-line react-hooks/exhaustive-deps

  // "Settings" in the speech feature's messages (e.g. no model yet).
  useEffect(() => setSpeechSettingsOpener(() => useUi.getState().openSettings("speech")), []);

  useShortcut("palette.open", () => ui.setPaletteOpen(true));
  useShortcut("settings.open", () => ui.openSettings());
  useShortcut("shortcuts.show", () => ui.setShortcutsOpen(true));
  useShortcut("sidebar.toggle", ui.toggleSidebar);
  useShortcut("theme.toggle", cycleTheme);
  useShortcut("profile.switch", () => void lock());
  useShortcut("library.close", () => void close());
  useShortcut("go.library", () => goHome("all"));
  useShortcut("go.notes", () => goHome("notes"));
  useShortcut("go.search", () => goHome("search"));
  useShortcut("go.organize", () => session.canEditLibrary && goHome("organize"));
  useShortcut("library.health", () => session.canEditLibrary && setHealth(true));
  useShortcut("library.backup", () => isOwner && void backUpNow());
  useShortcut("library.importForeign", () => session.canEditLibrary && openForeign());
  useShortcut("library.importArchive", () => isOwner && void pickArchiveToImport());
  useShortcut("app.fullscreen", () => {
    const w = getCurrentWindow();
    void w.isFullscreen().then((f) => w.setFullscreen(!f));
  });
  useShortcut("tabs.close", () => activeTab && tabs.close(activeTab));
  useShortcut("tabs.reopen", tabs.reopen);
  useShortcut("tabs.next", () => tabs.cycle(1));
  useShortcut("tabs.previous", () => tabs.cycle(-1));
  useShortcut("tabs.goto1", () => tabs.activate(null));
  for (const n of [2, 3, 4, 5, 6, 7, 8] as const) {
    // A fixed list of hooks, so the rules of hooks hold.
    // eslint-disable-next-line react-hooks/rules-of-hooks
    useShortcut(`tabs.goto${n}`, () => {
      const t = useTabs.getState().tabs[n - 2];
      if (t) tabs.activate(t.bookId);
    });
  }
  useShortcut("tabs.gotoLast", () => {
    const all = useTabs.getState().tabs;
    tabs.activate(all[all.length - 1]?.bookId ?? null);
  });
  useShortcut("tabs.split", () => tabs.toggleSplit());
  useShortcut("tabs.splitFocus", () => tabs.swapSplitFocus());
  useShortcut("tabs.newWindow", () => activeTab && void windows.moveTabToWindow(activeTab));
  useShortcut("window.new", () => void windows.newWindow());

  const k = (id: ActionId) => shortcutFor(id) ?? undefined;
  const actions: PaletteAction[] = useMemo(() => {
    const list: (PaletteAction & { edit?: boolean; owner?: boolean })[] = [
      {
        id: "go.library",
        group: "Go to",
        label: "Library",
        icon: Library,
        shortcut: k("go.library"),
        run: () => goHome("all"),
      },
      {
        id: "go.notes",
        group: "Go to",
        label: "Notes hub",
        icon: NotebookText,
        shortcut: k("go.notes"),
        run: () => goHome("notes"),
      },
      {
        id: "go.search",
        group: "Go to",
        label: "Search inside books, details and notes",
        icon: FileSearch,
        shortcut: k("go.search"),
        run: () => goHome("search"),
      },
      {
        id: "go.organize",
        group: "Go to",
        label: "Organize tags and categories",
        icon: Tags,
        shortcut: k("go.organize"),
        run: () => goHome("organize"),
        edit: true,
      },
      {
        id: "settings",
        group: "Go to",
        label: "Settings",
        icon: Settings,
        shortcut: k("settings.open"),
        run: () => ui.openSettings(),
      },
      {
        id: "shortcuts",
        group: "Go to",
        label: "Keyboard shortcuts",
        icon: Keyboard,
        shortcut: k("shortcuts.show"),
        run: () => ui.setShortcutsOpen(true),
      },
      {
        id: "books.import",
        group: "Books",
        label: "Import files…",
        icon: FileUp,
        shortcut: k("library.import"),
        run: () => void pickFilesToImport(),
        edit: true,
      },
      {
        id: "books.importFolder",
        group: "Books",
        label: "Import a folder…",
        icon: FolderUp,
        shortcut: k("library.importFolder"),
        run: () => void pickFolderToImport(),
        edit: true,
      },
      {
        id: "books.bulk",
        group: "Books",
        label: "Edit the selected books together…",
        icon: LibrarySquare,
        shortcut: k("books.bulkEdit"),
        run: () => void runAction("books.bulkEdit"),
        edit: true,
      },
      {
        id: "details.find",
        group: "Books",
        label: "Find details of the selected book online…",
        icon: Globe,
        shortcut: k("details.find"),
        run: () => void runAction("details.find"),
        edit: true,
      },
      {
        id: "details.fill",
        group: "Books",
        label: "Fill in missing details of the selected books online",
        icon: Globe,
        shortcut: k("details.fill"),
        run: () => void runAction("details.fill"),
        edit: true,
      },
      {
        id: "books.cite",
        group: "Books",
        label: "Cite the selected books…",
        icon: Quote,
        shortcut: k("books.cite"),
        run: () => void runAction("books.cite"),
      },
      {
        id: "library.export",
        group: "Library",
        label: "Export…",
        icon: Download,
        shortcut: k("library.export"),
        run: () => {
          if (!runAction("library.export")) openExport(null);
        },
        edit: true,
      },
      {
        id: "books.collection",
        group: "Books",
        label: "Save this search as a smart collection…",
        icon: Search,
        shortcut: k("collection.save"),
        run: () => useLibraryDialogs.getState().setSaveCollection(true),
      },
      {
        id: "books.rescan",
        group: "Books",
        label: "Check the library folder for changes",
        icon: RefreshCw,
        shortcut: k("library.refresh"),
        run: () => void commands.rescanLibrary(),
      },
      {
        id: "books.rebuild",
        group: "Books",
        label: "Rebuild the library index…",
        icon: RefreshCw,
        run: () => void rebuild(),
        edit: true,
      },
      {
        id: "view.grid",
        group: "View",
        label: "Show as grid",
        icon: LayoutGrid,
        shortcut: k("view.grid"),
        run: () => setView("grid"),
      },
      {
        id: "view.list",
        group: "View",
        label: "Show as list",
        icon: List,
        shortcut: k("view.list"),
        run: () => setView("list"),
      },
      {
        id: "view.shelf",
        group: "View",
        label: "Show as shelves",
        icon: BookOpen,
        shortcut: k("view.shelf"),
        run: () => setView("shelf"),
      },
      {
        id: "view.details",
        group: "View",
        label: "Show or hide details",
        icon: Info,
        shortcut: k("details.toggle"),
        run: toggleDetails,
      },
      {
        id: "view.sidebar",
        group: "View",
        label: "Show or hide sidebar",
        icon: PanelLeft,
        shortcut: k("sidebar.toggle"),
        run: ui.toggleSidebar,
      },
      {
        id: "view.split",
        group: "View",
        label: "Split view",
        icon: Columns2,
        shortcut: k("tabs.split"),
        run: () => tabs.toggleSplit(),
      },
      {
        id: "view.theme",
        group: "View",
        label: "Switch theme",
        icon: Moon,
        shortcut: k("theme.toggle"),
        run: cycleTheme,
      },
      {
        id: "profile.switch",
        group: "Profile",
        label: session.profile.hasPin ? "Lock" : "Switch profile",
        icon: session.profile.hasPin ? Lock : UserRound,
        shortcut: k("profile.switch"),
        run: () => void lock(),
      },
      {
        id: "library.importForeign",
        group: "Library",
        label: "Import from Calibre, Zotero, Mendeley, Goodreads…",
        icon: Upload,
        shortcut: k("library.importForeign"),
        run: () => openForeign(),
        edit: true,
      },
      {
        id: "library.importArchive",
        group: "Library",
        label: "Import a Libreri archive…",
        icon: Upload,
        shortcut: k("library.importArchive"),
        run: () => void pickArchiveToImport(),
        owner: true,
      },
      {
        id: "library.backup",
        group: "Library",
        label: "Back up the library now",
        icon: Archive,
        shortcut: k("library.backup"),
        run: () => void backUpNow(),
        owner: true,
      },
      {
        id: "library.health",
        group: "Library",
        label: "Check library health…",
        icon: HeartPulse,
        shortcut: k("library.health"),
        run: () => setHealth(true),
        edit: true,
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
        shortcut: k("library.close"),
        run: () => void close(),
      },
    ];
    return list.filter((a) => (!a.edit || session.canEditLibrary) && (!a.owner || isOwner));
  }, [
    isOwner,
    backUpNow,
    setHealth,
    openExport,
    openForeign,
    createNew,
    openExisting,
    close,
    cycleTheme,
    rebuild,
    setView,
    toggleDetails,
    goHome,
    lock,
    session,
    ui,
    tabs,
  ]);

  const home =
    nav.kind === "notes" ? (
      <NotesHub />
    ) : nav.kind === "search" ? (
      <SearchView key={nav.query ?? ""} />
    ) : nav.kind === "organize" ? (
      <OrganizeView />
    ) : (
      <LibraryView />
    );

  return (
    <div className="flex h-full flex-col">
      {/* Tab strip */}
      <div className="flex h-10 shrink-0 items-end gap-2 border-b bg-sidebar px-3">
        <TabStrip onMoveToWindow={(id) => void windows.moveTabToWindow(id)} />
        <button
          type="button"
          onClick={() => ui.setPaletteOpen(true)}
          className="mb-1.5 flex h-7 w-64 shrink-0 items-center gap-2 rounded-md border bg-background px-2.5 text-muted-foreground"
        >
          <Search className="size-3.5" aria-hidden />
          <span className="flex-1 truncate text-left">Search or run a command…</span>
          <Kbd action="palette.open" />
        </button>
        <div className="mb-1 flex items-center gap-1.5">
          <ThemeMenu />
          <ProfileMenu
            session={session}
            onSwitch={() => void lock()}
            onOpenSettings={() => ui.openSettings()}
            lockShortcut={shortcutFor("profile.switch")}
          />
        </div>
      </div>

      {!session.keepsData && (
        <div className="flex h-8 shrink-0 items-center justify-center gap-3 border-b bg-muted text-[12.5px]">
          <UserRound className="size-3.5" aria-hidden />
          You are a guest. Nothing you read, highlight or change here is kept.
          <button
            type="button"
            className="font-medium underline underline-offset-2"
            onClick={() => void lock()}
          >
            Leave
          </button>
        </div>
      )}

      <div className="relative min-h-0 flex-1">
        <ShortcutScope active={activeTab === null && ui.settings === null}>
          <div className={cn("absolute inset-0 flex", activeTab !== null && "hidden")}>
            <nav
              aria-label="Library"
              className={cn(
                "flex shrink-0 flex-col gap-2 border-r bg-sidebar pt-2 transition-[width]",
                ui.sidebarCollapsed ? "w-14 items-center" : "w-60",
              )}
            >
              <div className={cn("px-2.5", ui.sidebarCollapsed && "px-0")}>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Show or hide sidebar"
                  title="Show or hide sidebar"
                  onClick={ui.toggleSidebar}
                >
                  <PanelLeft />
                </Button>
              </div>
              <LibrarySidebar library={library} collapsed={ui.sidebarCollapsed} />
            </nav>
            <main className="min-w-0 flex-1">{home}</main>
          </div>
        </ShortcutScope>
        <div className={cn("absolute inset-0 flex", activeTab === null && "hidden")}>
          {openTabs.map((t) => {
            const isActive = activeTab === t.bookId;
            const inSplit =
              split !== null && (activeTab === split.left || activeTab === split.right);
            const isPartner =
              inSplit && !isActive && (t.bookId === split.left || t.bookId === split.right);
            if (!visited.has(t.bookId) && !isActive && !isPartner) return null;
            return (
              <ShortcutScope key={t.bookId} active={isActive && ui.settings === null}>
                <main
                  aria-label={t.title}
                  className={cn(
                    "relative min-w-0 flex-1",
                    !isActive && !isPartner && "hidden",
                    inSplit && t.bookId === split.right && "border-l",
                  )}
                  style={{ order: inSplit && t.bookId === split.right ? 1 : 0 }}
                  onPointerDownCapture={() => isPartner && tabs.activate(t.bookId)}
                >
                  <ReaderView tab={t} active={isActive} />
                </main>
              </ShortcutScope>
            );
          })}
        </div>
        {ui.settings && (
          <div className="absolute inset-0 z-20 bg-background">
            <SettingsPage
              library={library}
              session={session}
              section={ui.settings}
              onSection={(s) => ui.openSettings(s)}
              onClose={ui.closeSettings}
            />
          </div>
        )}
      </div>

      <CommandPalette
        open={ui.paletteOpen}
        onOpenChange={ui.setPaletteOpen}
        actions={actions}
        onSearch={(q) => goHome("search", q)}
      />
      <ShortcutsSheet open={ui.shortcutsOpen} onOpenChange={ui.setShortcutsOpen} />
      <BulkEditDialog />
      <FindDetailsDialog />
      <ExportDialog />
      <CitationDialog />
      <ImportArchiveDialog />
      <HealthDialog />
      <ForeignImportDialog />
      <HelperDialog />
      <OcrDialog />
      <SaveCollectionDialog />
      <ImportDialog />
      <DropOverlay />
    </div>
  );
}
