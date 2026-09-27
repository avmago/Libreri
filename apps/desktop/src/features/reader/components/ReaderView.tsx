import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  Bookmark,
  BookmarkCheck,
  NotebookPen,
  PanelLeft,
  Search,
} from "lucide-react";
import { toast } from "sonner";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { useBook, useLibraryView } from "@/features/library";
import { useHelperDialog } from "@/features/helpers";
import { useProfilePrefs } from "@/features/profiles";
import { bookUrl, commands, type Annotation, type HighlightColor } from "@/lib/ipc";
import { keysLabel, platform, shortcutFor, useShortcut, type ActionId } from "@/lib/shortcuts";
import { useTabs, type BookTab } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import {
  createRenderer,
  isPaged,
  PAGE_THEMES,
  pageTheme,
  parseLocator,
  type Locator,
  type PageLayout,
  type PdfDarkMode,
  type ReaderLocation,
  type Renderer,
  type SelectionInfo,
  type TocItem,
  type ZoomValue,
} from "@/readers";
import {
  savePosition,
  useAnnotations,
  useDeleteAnnotation,
  usePosition,
  useSaveAnnotation,
} from "../api";
import { useAppDark } from "../hooks/useAppDark";
import { parseBookLink } from "../links";
import { useReaderPrefs } from "../prefs";
import { AppearanceMenu } from "./AppearanceMenu";
import { ContentsPanel, type LeftPanel } from "./ContentsPanel";
import { FindBar } from "./FindBar";
import { NotebookPanel } from "./NotebookPanel";
import { AnnotationMenu, SelectionMenu } from "./Popovers";
import "@/readers/reader.css";
import "katex/dist/katex.min.css";

const keys = (id: ActionId) => keysLabel(shortcutFor(id), platform);

const PDF_STEPS = [0.5, 0.67, 0.8, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4];
const TEXT_STEPS = [0.8, 0.9, 1, 1.1, 1.2, 1.35, 1.5, 1.7, 2];

function stepZoom(current: number, steps: number[], dir: 1 | -1): number {
  if (dir > 0) return steps.find((s) => s > current + 0.01) ?? steps[steps.length - 1]!;
  return [...steps].reverse().find((s) => s < current - 0.01) ?? steps[0]!;
}

function quoteBlock(
  a: Pick<Annotation, "quote" | "label" | "note" | "id">,
  bookId: string,
  fallback: string,
) {
  const text = (a.quote?.exact ?? "").trim().replace(/\n+/g, " ");
  const label = a.label || fallback || "Link";
  const lines = [`> ${text}`, ">", `> — [${label}](libreri://book/${bookId}#annotation=${a.id})`];
  if (a.note) lines.push("", a.note);
  return lines.join("\n");
}

const newId = () => crypto.randomUUID();

function countLabel(list: Annotation[]): string {
  const h = list.filter((a) => a.kind === "highlight").length;
  const b = list.length - h;
  const part = (n: number, one: string) => (n ? `${n} ${one}${n === 1 ? "" : "s"}` : "");
  return [part(h, "highlight"), part(b, "bookmark")].filter(Boolean).join(" · ");
}

/** One open book: toolbar, contents and marks, the page, notebook. */
export function ReaderView({ tab, active }: { tab: BookTab; active: boolean }) {
  const { bookId } = tab;
  const { data: book, error: bookError } = useBook(bookId);
  const position = usePosition(bookId);
  const { data: annotations = [] } = useAnnotations(bookId);
  const saveAnnotation = useSaveAnnotation(bookId);
  const deleteAnnotation = useDeleteAnnotation(bookId);
  const prefs = useReaderPrefs();
  const profilePrefs = useProfilePrefs((s) => s.prefs);
  const appDark = useAppDark();
  const openTab = useTabs((s) => s.open);
  const clearJump = useTabs((s) => s.clearJump);

  const hostRef = useRef<HTMLDivElement>(null);
  const rendererRef = useRef<Renderer | null>(null);
  const [status, setStatus] = useState<"loading" | "ready" | "error">("loading");
  const [error, setError] = useState<string | null>(null);
  const [location, setLocation] = useState<ReaderLocation | null>(null);
  const [toc, setToc] = useState<TocItem[]>([]);
  const [selection, setSelection] = useState<SelectionInfo | null>(null);
  const [menu, setMenu] = useState<{ id: string; rect: DOMRect; edit: boolean } | null>(null);
  const [left, setLeft] = useState<LeftPanel | null>("contents");
  const [lastLeft, setLastLeft] = useState<LeftPanel>("contents");
  const [notebookOpen, setNotebookOpen] = useState(false);
  const [notebookInsert, setNotebookInsert] = useState<string | null>(null);
  const [findOpen, setFindOpen] = useState(false);
  const [findStep, setFindStep] = useState<{ backwards: boolean; seq: number }>({
    backwards: false,
    seq: 0,
  });
  const [lastQuery, setLastQuery] = useState("");
  const [focusMode, setFocusMode] = useState(false);
  // Jumps made from the app (contents, marks, links) can be undone with Back.
  const history = useRef<{ back: Locator[]; forward: Locator[] }>({ back: [], forward: [] });
  const [zoom, setZoom] = useState<ZoomValue>(1);
  const [pageLayout, setPageLayout] = useState<PageLayout | null>(null);
  // Bumped to open the book again (after installing a helper).
  const [attempt, setAttempt] = useState(0);
  const openHelper = useHelperDialog((s) => s.open);
  const [pageInput, setPageInput] = useState("");
  const pageInputRef = useRef<HTMLInputElement>(null);

  const theme = useMemo(() => {
    const chosen = pageTheme(prefs.theme);
    return prefs.followApp && appDark && !chosen.dark ? pageTheme("night") : chosen;
  }, [prefs.theme, prefs.followApp, appDark]);

  // Save the reading position a moment after the reader stops moving.
  const pending = useRef<ReaderLocation | null>(null);
  const openedRef = useRef(false);
  const saveTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const qc = useQueryClient();
  const savedOnce = useRef(false);
  const flushPosition = useCallback(() => {
    clearTimeout(saveTimer.current);
    const loc = pending.current;
    pending.current = null;
    if (!loc) return;
    void savePosition(bookId, JSON.stringify(loc.locator), loc.progress)
      .then(() => {
        // Opening a book can mark it "Reading": refresh the library once.
        if (savedOnce.current) return;
        savedOnce.current = true;
        void qc.invalidateQueries({
          predicate: (q) => q.queryKey[0] === "lib" && q.queryKey[1] !== "reader",
        });
      })
      .catch(() => {});
  }, [bookId, qc]);

  const annotationsRef = useRef(annotations);
  useEffect(() => {
    annotationsRef.current = annotations;
  }, [annotations]);

  const handleLink = useCallback(
    (href: string) => {
      const link = parseBookLink(href);
      if (link) {
        if (link.bookId === bookId) {
          const a = annotationsRef.current.find((x) => x.id === link.annotation);
          if (a) void rendererRef.current?.showAnnotation(a);
        } else {
          openTab({ bookId: link.bookId, title: "Book", fileType: "pdf", jumpTo: link.annotation });
        }
      } else if (/^(https?:|mailto:)/i.test(href)) {
        void commands.openExternalUrl(href).then((r) => {
          if (r.status === "error") toast.error(r.error.message);
        });
      }
    },
    [bookId, openTab],
  );

  // Create the renderer once the book and its saved position are known.
  const relPath = book?.relPath;
  const fileType = book?.fileType;
  const ready = position.isSuccess || position.isError;
  const linkRef = useRef(handleLink);
  useEffect(() => {
    linkRef.current = handleLink;
  }, [handleLink]);
  useEffect(() => {
    if (!relPath || !fileType || !ready || !hostRef.current) return;
    let cancelled = false;
    let renderer: Renderer | null = null;
    const host = hostRef.current;
    void (async () => {
      try {
        renderer = await createRenderer(
          fileType,
          {
            relocate: (loc) => {
              setLocation(loc);
              // Ignore the start page shown while the saved place is restored.
              if (!openedRef.current) return;
              pending.current = loc;
              clearTimeout(saveTimer.current);
              saveTimer.current = setTimeout(flushPosition, 1200);
            },
            selection: (sel) => {
              setSelection(sel);
              if (sel) setMenu(null);
            },
            annotationClick: (id, rect) => setMenu({ id, rect, edit: false }),
            externalLink: (href) => linkRef.current(href),
          },
          bookId,
        );
        if (cancelled) return;
        const resume = useProfilePrefs.getState().prefs.reader.resume;
        await renderer.open(host, bookUrl(relPath), resume ? parseLocator(position.data) : null);
        if (cancelled) {
          renderer.destroy();
          return;
        }
        rendererRef.current = renderer;
        openedRef.current = true;
        const { reader } = useProfilePrefs.getState().prefs;
        renderer.setLineHeight(reader.lineHeight);
        if (!renderer.paged && reader.fontScale !== 100) renderer.setZoom(reader.fontScale / 100);
        setToc(renderer.toc());
        setZoom(renderer.zoom());
        setPageLayout(renderer.layoutOptions?.() ?? null);
        setStatus("ready");
      } catch (e) {
        if (!cancelled) {
          setError(e instanceof Error ? e.message : String(e));
          setStatus("error");
        }
      }
    })();
    return () => {
      cancelled = true;
      flushPosition();
      rendererRef.current = null;
      renderer?.destroy();
    };
    // Open once per book; later moves of the file do not reload the page.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [Boolean(relPath), fileType, ready, attempt]);

  useEffect(() => {
    if (status === "ready") rendererRef.current?.setAnnotations(annotations);
  }, [annotations, status]);

  useEffect(() => {
    if (status === "ready") rendererRef.current?.setTheme(theme, prefs.pdfMode);
  }, [theme, prefs.pdfMode, status]);

  // Jump to an annotation when the tab was opened from a link.
  useEffect(() => {
    if (status !== "ready" || !tab.jumpTo || !annotations.length) return;
    const a = annotations.find((x) => x.id === tab.jumpTo);
    if (a) void rendererRef.current?.showAnnotation(a);
    clearJump(bookId);
  }, [status, tab.jumpTo, annotations, bookId, clearJump]);

  // Keep the tab title in step with the book's details.
  const rename = useTabs((s) => s.rename);
  useEffect(() => {
    if (book && (book.metadata.title !== tab.title || book.fileType !== tab.fileType)) {
      rename(bookId, book.metadata.title, book.fileType);
    }
  }, [book, bookId, rename, tab.title, tab.fileType]);

  // Line spacing follows Settings › Reader.
  useEffect(() => {
    if (status === "ready") rendererRef.current?.setLineHeight(profilePrefs.reader.lineHeight);
  }, [profilePrefs.reader.lineHeight, status]);

  const r = () => rendererRef.current;
  const isPdf = isPaged(fileType);
  const changeLayout = (change: Partial<PageLayout>) => {
    r()?.setLayout?.(change);
    setPageLayout(r()?.layoutOptions?.() ?? null);
    setZoom(r()?.zoom() ?? 1);
  };
  // Manga: the arrow keys turn pages the other way.
  const forward = () => (r()?.rightToLeft ? r()?.prev() : r()?.next());
  const backward = () => (r()?.rightToLeft ? r()?.next() : r()?.prev());

  const changeZoom = (dir: 1 | -1 | 0) => {
    const renderer = r();
    if (!renderer) return;
    let next: ZoomValue;
    if (dir === 0) next = isPdf ? "auto" : 1;
    else {
      const cur = renderer.zoom();
      const n = typeof cur === "number" ? cur : 1;
      next = stepZoom(n, isPdf ? PDF_STEPS : TEXT_STEPS, dir);
    }
    renderer.setZoom(next);
    setZoom(renderer.zoom());
  };

  const bookmarkHere = annotations.find(
    (a) =>
      a.kind === "bookmark" &&
      location &&
      (location.page !== undefined
        ? parseLocator(a.locator)?.type === "pdf" &&
          (parseLocator(a.locator) as { page: number }).page === location.page
        : Math.abs((a.position ?? 0) - location.progress) < 0.004),
  );

  const toggleBookmark = () => {
    if (!location) return;
    if (bookmarkHere) {
      deleteAnnotation.mutate(bookmarkHere.id);
      return;
    }
    const locator =
      location.locator.type === "pdf"
        ? { type: "pdf" as const, page: location.page ?? 1 }
        : location.locator;
    saveAnnotation.mutate({
      id: newId(),
      bookId,
      kind: "bookmark",
      color: null,
      locator: JSON.stringify(locator),
      quote: null,
      note: null,
      label:
        location.section && location.page === undefined ? location.section : location.shortLabel,
      position: location.progress,
      createdAt: "",
      modifiedAt: "",
    });
  };

  const highlightFromSelection = (color: HighlightColor, then?: (a: Annotation) => void) => {
    const sel = selection;
    if (!sel) return;
    const a: Annotation = {
      id: newId(),
      bookId,
      kind: "highlight",
      color,
      locator: JSON.stringify(sel.locator),
      quote: sel.quote,
      note: null,
      label: sel.label || location?.shortLabel || null,
      position: sel.position,
      createdAt: "",
      modifiedAt: "",
    };
    saveAnnotation.mutate(a, {
      onSuccess: (saved) => then?.(saved),
      onError: (e) => toast.error("Could not save the highlight", { description: String(e) }),
    });
    r()?.clearSelection();
    setSelection(null);
  };

  const addToNotebook = (a: Annotation) => {
    setNotebookOpen(true);
    const block = quoteBlock(a, bookId, location?.shortLabel ?? "");
    setNotebookInsert(
      profilePrefs.notes.linkQuotes
        ? block
        : block
            .split("\n")
            .filter((l) => !l.startsWith("> —"))
            .join("\n")
            .replace(/>\n\n/, "\n"),
    );
  };

  /** Goes somewhere from the app, remembering where we were for Back. */
  const jump = (go: () => Promise<void> | void) => {
    if (location) {
      history.current.back.push(location.locator);
      history.current.forward = [];
    }
    void go();
  };
  const travel = (from: "back" | "forward") => {
    const h = history.current;
    const target = h[from].pop();
    if (!target) return;
    if (location) h[from === "back" ? "forward" : "back"].push(location.locator);
    void r()?.goTo(target);
  };

  const flatToc = useMemo(() => {
    const out: TocItem[] = [];
    const walk = (items: TocItem[]) =>
      items.forEach((i) => {
        out.push(i);
        walk(i.children);
      });
    walk(toc);
    return out;
  }, [toc]);
  const chapter = (dir: 1 | -1) => {
    if (!flatToc.length) return dir > 0 ? r()?.next() : r()?.prev();
    const i = flatToc.findIndex((t) => t.label === location?.section);
    const next = flatToc[i < 0 ? (dir > 0 ? 0 : flatToc.length - 1) : i + dir];
    if (next) jump(() => r()?.goTo(next.target));
  };
  const nextHighlight = (dir: 1 | -1) => {
    const here = location?.progress ?? 0;
    const list = annotations
      .filter((a) => a.kind === "highlight")
      .sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
    const target =
      dir > 0
        ? list.find((a) => (a.position ?? 0) > here + 0.0005)
        : [...list].reverse().find((a) => (a.position ?? 0) < here - 0.0005);
    if (target) jump(() => r()?.showAnnotation(target));
    else toast(dir > 0 ? "No more highlights after this page" : "No highlights before this page");
  };
  const cycleTheme = () => {
    const ids = PAGE_THEMES.map((t) => t.id);
    const next = ids[(ids.indexOf(prefs.theme) + 1) % ids.length]!;
    prefs.set({ theme: next, followApp: false });
    toast(`Page theme: ${pageTheme(next).name}`);
  };
  const cyclePdfMode = () => {
    const modes: PdfDarkMode[] = ["recolour", "invert", "dim", "off"];
    prefs.set({ pdfMode: modes[(modes.indexOf(prefs.pdfMode) + 1) % modes.length]! });
  };

  const goToPage = () => {
    const v = pageInput.trim();
    if (v) void r()?.goTo(v);
    setPageInput("");
    pageInputRef.current?.blur();
  };

  // Shortcuts (only while this tab is showing; see ShortcutScope).
  useShortcut("reader.next", forward);
  useShortcut("reader.previous", backward);
  useShortcut("reader.pageDown", () => r()?.next());
  useShortcut("reader.pageUp", () => r()?.prev());
  useShortcut("reader.space", () => r()?.next());
  useShortcut("reader.spaceBack", () => r()?.prev());
  useShortcut("reader.scrollDown", () => r()?.scrollBy(1));
  useShortcut("reader.scrollUp", () => r()?.scrollBy(-1));
  useShortcut("reader.start", () => jump(() => r()?.start()));
  useShortcut("reader.end", () => jump(() => r()?.end()));
  useShortcut("reader.nextChapter", () => chapter(1));
  useShortcut("reader.previousChapter", () => chapter(-1));
  useShortcut("reader.back", () => travel("back"));
  useShortcut("reader.forward", () => travel("forward"));
  useShortcut("reader.find", () => setFindOpen(true));
  const findAgain = (backwards: boolean) => {
    setFindOpen(true);
    setFindStep((s) => ({ backwards, seq: s.seq + 1 }));
  };
  useShortcut("reader.findNext", () => findAgain(false));
  useShortcut("reader.findPrevious", () => findAgain(true));
  useShortcut("reader.zoomIn", () => changeZoom(1));
  useShortcut("reader.zoomOut", () => changeZoom(-1));
  useShortcut("reader.zoomReset", () => changeZoom(0));
  useShortcut("reader.fitWidth", () => {
    if (!isPdf) return;
    r()?.setZoom("page-width");
    setZoom(r()?.zoom() ?? 1);
  });
  useShortcut("reader.fitPage", () => {
    if (!isPdf) return;
    r()?.setZoom("page-fit");
    setZoom(r()?.zoom() ?? 1);
  });
  useShortcut("reader.goToPage", () => pageInputRef.current?.focus());
  useShortcut("reader.bookmark", toggleBookmark);
  useShortcut("reader.contents", () => setLeft((p) => (p ? null : lastLeft)));
  useShortcut("reader.notebook", () => setNotebookOpen((o) => !o));
  useShortcut("reader.themeNext", cycleTheme);
  useShortcut("reader.pdfModeNext", cyclePdfMode);
  useShortcut("reader.focusMode", () => setFocusMode((f) => !f));
  useShortcut("reader.details", () => {
    useTabs.getState().activate(null);
    useLibraryView.getState().setSelection([bookId]);
    useLibraryView.getState().setDetailsOpen(true);
  });
  useShortcut("reader.highlight", () => highlightFromSelection(profilePrefs.notes.defaultColor));
  useShortcut("reader.comment", () => {
    const sel = selection;
    if (sel) {
      highlightFromSelection(profilePrefs.notes.defaultColor, (saved) =>
        setMenu({ id: saved.id, rect: sel.rect, edit: true }),
      );
    }
  });
  useShortcut("reader.addToNotebook", () =>
    highlightFromSelection(profilePrefs.notes.defaultColor, addToNotebook),
  );
  useShortcut("reader.nextHighlight", () => nextHighlight(1));
  useShortcut("reader.previousHighlight", () => nextHighlight(-1));

  // Save the place as soon as the user switches to another tab.
  useEffect(() => {
    if (!active) flushPosition();
  }, [active, flushPosition]);

  const menuAnnotation = menu ? annotations.find((a) => a.id === menu.id) : undefined;
  const zoomLabel = typeof zoom === "number" ? `${Math.round(zoom * 100)}%` : "Automatic";

  if (bookError) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 text-center">
        <AlertTriangle className="size-6 text-destructive" aria-hidden />
        <p>This book is no longer in the library.</p>
        <Button variant="outline" onClick={() => useTabs.getState().close(bookId)}>
          Close tab
        </Button>
      </div>
    );
  }

  return (
    <div className="flex h-full min-h-0 flex-col">
      {/* Toolbar */}
      <div
        className={cn("flex h-11 shrink-0 items-center gap-1 border-b px-2", focusMode && "hidden")}
      >
        <Button
          variant="ghost"
          size="icon"
          aria-label="Contents and marks"
          aria-pressed={left !== null}
          title={`Contents and marks (${keys("reader.contents")})`}
          onClick={() => setLeft((p) => (p ? null : lastLeft))}
        >
          <PanelLeft />
        </Button>
        <div className="flex min-w-0 flex-1 flex-col px-2 leading-tight">
          <span className="truncate font-medium">{book?.metadata.title ?? tab.title}</span>
          <span className="truncate text-[11.5px] text-muted-foreground">
            {location?.section ?? book?.metadata.authors.join(", ")}
          </span>
        </div>
        {isPdf && location?.pages ? (
          <form
            className="flex items-center gap-1.5 text-muted-foreground"
            onSubmit={(e) => {
              e.preventDefault();
              goToPage();
            }}
          >
            <input
              ref={pageInputRef}
              value={pageInput}
              onChange={(e) => setPageInput(e.target.value)}
              onKeyDown={(e) => e.key === "Escape" && pageInputRef.current?.blur()}
              placeholder={String(location.page ?? "")}
              aria-label={`Go to page (${keys("reader.goToPage")})`}
              title={`Go to page (${keys("reader.goToPage")})`}
              className="h-7 w-12 rounded-md border border-input bg-background px-1.5 text-center text-foreground tabular-nums outline-none placeholder:text-foreground focus:placeholder:text-muted-foreground focus-visible:border-ring"
            />
            <span className="tabular-nums">/ {location.pages}</span>
          </form>
        ) : null}
        <Button
          variant="ghost"
          size="icon"
          aria-label="Find in book"
          title={`Find in book (${keys("reader.find")})`}
          onClick={() => setFindOpen(true)}
        >
          <Search />
        </Button>
        <AppearanceMenu
          isPdf={isPdf}
          zoomLabel={zoomLabel}
          onZoom={changeZoom}
          pageLayout={pageLayout}
          onPageLayout={changeLayout}
        />
        <Button
          variant="ghost"
          size="icon"
          aria-label={bookmarkHere ? "Remove bookmark" : "Add bookmark"}
          aria-pressed={Boolean(bookmarkHere)}
          title={`${bookmarkHere ? "Remove bookmark" : "Bookmark this page"} (${keys("reader.bookmark")})`}
          onClick={toggleBookmark}
          disabled={!location}
        >
          {bookmarkHere ? <BookmarkCheck className="fill-current" /> : <Bookmark />}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Notebook"
          aria-pressed={notebookOpen}
          title={`Notebook (${keys("reader.notebook")})`}
          onClick={() => setNotebookOpen((o) => !o)}
        >
          <NotebookPen />
        </Button>
      </div>

      <div className="flex min-h-0 flex-1">
        {left && !focusMode && (
          <ContentsPanel
            panel={left}
            setPanel={(p) => {
              setLeft(p);
              setLastLeft(p);
            }}
            toc={toc}
            section={location?.section}
            annotations={annotations}
            onGo={(target) => jump(() => r()?.goTo(target))}
            onShow={(a) => jump(() => r()?.showAnnotation(a))}
            onDelete={(a) => deleteAnnotation.mutate(a.id)}
          />
        )}

        <div className="relative min-w-0 flex-1" style={{ background: theme.surround }}>
          <div
            ref={hostRef}
            className="absolute inset-0"
            style={
              profilePrefs.reader.brightness !== 100 || profilePrefs.reader.contrast !== 100
                ? {
                    filter: `brightness(${profilePrefs.reader.brightness / 100}) contrast(${profilePrefs.reader.contrast / 100})`,
                  }
                : undefined
            }
          />
          {focusMode && (
            <Button
              variant="outline"
              size="sm"
              className="absolute top-3 right-4 z-10 opacity-40 hover:opacity-100 focus-visible:opacity-100"
              onClick={() => setFocusMode(false)}
            >
              Show toolbar
            </Button>
          )}
          {status === "loading" && (
            <div className="absolute inset-0 flex items-center justify-center text-muted-foreground">
              Opening…
            </div>
          )}
          {status === "error" && (
            <div className="absolute inset-0 flex flex-col items-center justify-center gap-3 bg-background p-6 text-center">
              <AlertTriangle className="size-6 text-destructive" aria-hidden />
              <p className="max-w-md">Libreri could not open this book.</p>
              <p className="max-w-md font-mono text-[12px] text-muted-foreground">{error}</p>
              <div className="flex gap-2">
                {error && /DjVuLibre|unar/.test(error) && (
                  <Button
                    onClick={() =>
                      openHelper(/DjVuLibre/.test(error) ? "djvulibre" : "unar", () => {
                        setError(null);
                        setStatus("loading");
                        setAttempt((a) => a + 1);
                      })
                    }
                  >
                    Install {/DjVuLibre/.test(error) ? "DjVuLibre" : "unar"}…
                  </Button>
                )}
                <Button variant="outline" onClick={() => void commands.openBookExternally(bookId)}>
                  Open in another app
                </Button>
              </div>
            </div>
          )}
          {findOpen && (
            <FindBar
              step={findStep}
              initialQuery={lastQuery}
              onQuery={setLastQuery}
              onFind={(q, back) => r()?.find(q, back) ?? Promise.resolve({ current: 0, total: 0 })}
              onClose={() => {
                setFindOpen(false);
                r()?.clearFind();
              }}
            />
          )}
        </div>

        {notebookOpen && !focusMode && (
          <NotebookPanel
            bookId={bookId}
            insert={notebookInsert}
            onInserted={() => setNotebookInsert(null)}
            onLink={handleLink}
            onClose={() => setNotebookOpen(false)}
          />
        )}
      </div>

      {/* Status bar */}
      <div
        className={cn(
          "flex h-7 shrink-0 items-center gap-3 border-t px-3 text-[11.5px] text-muted-foreground",
          focusMode && "hidden",
        )}
      >
        <span className="tabular-nums">{location?.label ?? ""}</span>
        <div className="h-1 max-w-64 flex-1 overflow-hidden rounded-full bg-muted" aria-hidden>
          <div
            className="h-full bg-primary/70"
            style={{ width: `${Math.round((location?.progress ?? 0) * 100)}%` }}
          />
        </div>
        <span className="tabular-nums">{Math.round((location?.progress ?? 0) * 100)}%</span>
        <span className="flex-1" />
        {annotations.length > 0 && (
          <button type="button" className="hover:text-foreground" onClick={() => setLeft("marks")}>
            {countLabel(annotations)}
          </button>
        )}
      </div>

      {active && selection && !menu && (
        <SelectionMenu
          rect={selection.rect}
          onHighlight={(c) => highlightFromSelection(c)}
          onComment={() =>
            highlightFromSelection(profilePrefs.notes.defaultColor, (saved) =>
              setMenu({ id: saved.id, rect: selection.rect, edit: true }),
            )
          }
          onNotebook={() => highlightFromSelection(profilePrefs.notes.defaultColor, addToNotebook)}
          onCopy={() => {
            void navigator.clipboard.writeText(selection.quote.exact ?? "");
            r()?.clearSelection();
            setSelection(null);
          }}
          onClose={() => setSelection(null)}
        />
      )}
      {active && menu && menuAnnotation && (
        <AnnotationMenu
          key={menuAnnotation.id}
          annotation={menuAnnotation}
          rect={menu.rect}
          startEditing={menu.edit}
          onChange={(a) => saveAnnotation.mutate(a)}
          onNotebook={(a) => {
            setMenu(null);
            addToNotebook(a);
          }}
          onDelete={(a) => {
            setMenu(null);
            deleteAnnotation.mutate(a.id);
          }}
          onClose={() => setMenu(null)}
        />
      )}
    </div>
  );
}
