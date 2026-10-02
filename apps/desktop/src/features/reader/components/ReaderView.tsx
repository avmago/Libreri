import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { PageSkeleton } from "@/components/Placeholders";
import { setReadingFullscreen, toggleReadingFullscreen, useFullscreen } from "@/lib/fullscreen";
import {
  AlertTriangle,
  Bookmark,
  BookPlus,
  Camera,
  BookmarkCheck,
  GitCompare,
  Headphones,
  Podcast,
  Printer,
  ZoomIn,
  ZoomOut,
  History,
  Mic,
  Volume2,
  NotebookPen,
  PanelLeft,
  PenLine,
  Search,
  Loader2,
  Link2,
  Maximize,
  Minimize,
  SquareSigma,
} from "lucide-react";
import { toast } from "sonner";
import { useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { useBook, useLibraryView } from "@/features/library";
import { useHelperDialog } from "@/features/helpers";
import { usePermissions, useProfilePrefs } from "@/features/profiles";
import { VoiceNoteBar, useVoiceNote, type RecordedVoice } from "@/features/speech";
import { CanvasPanel } from "@/features/canvas";
import { bookUrl, commands, type Annotation, type HighlightColor, type TextQuote } from "@/lib/ipc";
import { keysLabel, platform, shortcutFor, useShortcut, type ActionId } from "@/lib/shortcuts";
import { useTabs, type BookTab } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import {
  canBionic,
  createRenderer,
  isAudio,
  lineAt,
  isPaged,
  PAGE_THEMES,
  pageTheme,
  parseLocator,
  layoutToLatex,
  textToLatex,
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
  positionKey,
  savePosition,
  useAnnotations,
  useDeleteAnnotation,
  usePosition,
  useSaveAnnotation,
} from "../api";
import { useAppDark } from "../hooks/useAppDark";
import { usePinchZoom } from "../hooks/usePinchZoom";
import { useWide } from "@/lib/useWide";
import { PrintDialog } from "../print/PrintDialog";
import { pageJump, parseBookLink } from "../links";
import { useReaderPrefs } from "../prefs";
import { AppearanceMenu } from "./AppearanceMenu";
import { ContentsPanel, type LeftPanel } from "./ContentsPanel";
import { FindBar } from "./FindBar";
import { NotebookPanel } from "./NotebookPanel";
import { AnnotationMenu, SelectionMenu } from "./Popovers";
import { MathPopover } from "./MathPopover";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import { drawList, markupModel } from "@/readers";
import { useMarkup } from "../markup/useMarkup";
import { MarkupToolbar } from "../markup/MarkupToolbar";
import { MarkupPanel } from "../markup/MarkupPanel";
import {
  CalibrateDialog,
  ExportMarkupDialog,
  NewStampDialog,
  NotePopover,
  SignatureDialog,
} from "../markup/MarkupDialogs";
import { pickPicture } from "../markup/pickPicture";
import { PageEditor } from "../edit/PageEditor";
import { VersionsDialog } from "../edit/EditDialogs";
import { CompareView } from "../compare/CompareView";
import { CompareDialog } from "../compare/CompareDialog";
import { useReadAloud } from "../speech/useReadAloud";
import { AudiobookView } from "../listening/AudiobookView";
import { CaptureDialog, CaptureViewer } from "../capture";
import { mathsFromPicture, useMathsSettings } from "../maths/api";
import { AddLinkDialog, CopyViewer, LinksPanel, linkOf, type LinkInfo } from "../weblinks";
import { useListening } from "../listening/store";
import { AddToLibraryDialog, openBook } from "@/features/feeds";
import { usePlayer, openPodcastLink } from "@/features/podcasts";
import { TimerButton, TodayReading, onBreak, reportReading } from "@/features/study";
import { ListenBar, type ListenMode } from "../listening/ListenBar";
import { FocusOverlay } from "../adhd/FocusOverlay";
import { useAdhd, useAdhdPause } from "../adhd/state";
import { pdfAnnots } from "@/readers";
import { useQuery } from "@tanstack/react-query";
import { unwrap } from "@/lib/ipc";
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

/** A bookmark in a book without pages is at the place shown: the same
 * chapter position or text offset (or, failing that, very nearly the same
 * point in the book, so a neighbour's bookmark is not taken for it). */
function samePlace(saved: Locator | null, position: number | null, here: ReaderLocation) {
  const now = here.locator;
  if (saved && saved.type === now.type) {
    if (saved.type === "cfi" && now.type === "cfi") return saved.cfi === now.cfi;
    if (saved.type === "text" && now.type === "text") return saved.start === now.start;
  }
  return Math.abs((position ?? 0) - here.progress) < 0.0002;
}
const NO_TOC: TocItem[] = [];

function countLabel(list: Annotation[]): string {
  const h = list.filter((a) => a.kind === "highlight").length;
  const b = list.filter((a) => a.kind === "bookmark").length;
  const m = list.filter((a) => a.kind === "markup").length;
  const v = list.filter((a) => a.kind === "voice").length;
  const c = list.filter((a) => a.kind === "capture").length;
  const part = (n: number, one: string) => (n ? `${n} ${one}${n === 1 ? "" : "s"}` : "");
  return [
    part(h, "highlight"),
    part(b, "bookmark"),
    part(m, "mark"),
    part(v, "voice note"),
    part(c, "paper note"),
  ]
    .filter(Boolean)
    .join(" · ");
}

/** One open book: the audiobook player, or the reader. */
export function ReaderView({ tab, active }: { tab: BookTab; active: boolean }) {
  return isAudio(tab.fileType) ? (
    <AudiobookView tab={tab} active={active} />
  ) : (
    <BookReader tab={tab} active={active} />
  );
}

/** One open book: toolbar, contents and marks, the page, notebook. */
function BookReader({ tab, active }: { tab: BookTab; active: boolean }) {
  const { bookId } = tab;
  // A download from Feeds, read before it is added to the library: the
  // reader and its appearance, reading aloud and maths, but no marks.
  const doc = tab.feed ?? null;
  const { data: book, error: bookError } = useBook(doc ? null : bookId);
  const [addingDoc, setAddingDoc] = useState(false);
  const [printing, setPrinting] = useState(false);
  const notForDocs = () =>
    toast("Add it to your library to highlight and keep notes", {
      action: { label: "Add to library…", onClick: () => setAddingDoc(true) },
    });
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
  // Bumped to open the book again (after installing a helper, undoing a
  // form, or a new version of the file).
  const [attempt, setAttempt] = useState(0);
  // How the latest opening went. Kept with the opening it belongs to, so
  // the reader counts as "loading" again while a new one is under way.
  const openKey = `${bookId}:${attempt}`;
  const [opened, setOpened] = useState<{
    key: string;
    status: "ready" | "error";
    toc: TocItem[];
    error: string | null;
  } | null>(null);
  const current = opened?.key === openKey ? opened : null;
  const status = current?.status ?? "loading";
  const error = current?.error ?? null;
  const toc = current?.toc ?? NO_TOC;
  const [location, setLocation] = useState<ReaderLocation | null>(null);
  const [selection, setSelection] = useState<SelectionInfo | null>(null);
  const [menu, setMenu] = useState<{ id: string; rect: DOMRect; edit: boolean } | null>(null);
  // Narrow windows start with the page alone.
  const roomy = useWide(900);
  const [left, setLeft] = useState<LeftPanel | null>(() =>
    window.matchMedia("(min-width: 900px)").matches ? "contents" : null,
  );
  const [lastLeft, setLastLeft] = useState<LeftPanel>("contents");
  const [notebookOpen, setNotebookOpenRaw] = useState(false);
  // A formula shown as LaTeX (clicked, or rebuilt from selected text).
  const [math, setMath] = useState<{
    latex: string;
    rect: DOMRect;
    from: "book" | "text" | "picture";
    /** Where the maths is on the page, to read it from the picture. */
    area?: DOMRect;
  } | null>(null);
  const [canvasOpen, setCanvasOpenRaw] = useState(false);
  // The notebook and the canvas share the right side: one at a time.
  const setNotebookOpen = (v: boolean | ((o: boolean) => boolean)) =>
    setNotebookOpenRaw((o) => {
      const next = typeof v === "function" ? v(o) : v;
      if (next) setCanvasOpenRaw(false);
      return next;
    });
  const setCanvasOpen = (v: boolean | ((o: boolean) => boolean)) =>
    setCanvasOpenRaw((o) => {
      const next = typeof v === "function" ? v(o) : v;
      if (next) setNotebookOpenRaw(false);
      return next;
    });
  // Clipping a figure: a box drawn over the page.
  const [clipping, setClipping] = useState<{
    done: (r: DOMRect | null) => void;
    what: string;
  } | null>(null);
  const [clipBox, setClipBox] = useState<{ x0: number; y0: number; x1: number; y1: number } | null>(
    null,
  );
  const [notebookInsert, setNotebookInsert] = useState<string | null>(null);
  const [findOpen, setFindOpen] = useState(false);
  const [findStep, setFindStep] = useState<{ backwards: boolean; seq: number }>({
    backwards: false,
    seq: 0,
  });
  const [lastQuery, setLastQuery] = useState("");
  const [focusMode, setFocusMode] = useState(false);
  // Full-screen reading: only the page; toolbar and page bar come back at
  // the top and bottom edges of the screen.
  const immersive = useFullscreen((s) => s.reading) && active;
  const bare = focusMode || immersive;
  const [edge, setEdge] = useState<"top" | "bottom" | null>(null);
  useEffect(() => {
    if (!immersive) return;
    const menuOpen = () => !!document.querySelector("[data-radix-popper-content-wrapper]");
    const onMove = (e: PointerEvent) => {
      const h = window.innerHeight;
      setEdge((now) => {
        if (e.clientY <= 4) return "top";
        if (e.clientY >= h - 4) return "bottom";
        if (menuOpen()) return now;
        if (now === "top" && e.clientY > 76) return null;
        if (now === "bottom" && e.clientY < h - 56) return null;
        return now;
      });
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      const t = e.target as HTMLElement | null;
      if (t?.closest("input, textarea, select, [contenteditable='true']")) return;
      if (document.querySelector("[role='dialog'], [data-radix-popper-content-wrapper]")) return;
      setReadingFullscreen(false);
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("keydown", onKey);
      setEdge(null);
    };
  }, [immersive]);
  // The full-screen button shows while the pointer moves over the page.
  const [pointerActive, setPointerActive] = useState(false);
  const pointerTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const pointerMoved = () => {
    setPointerActive(true);
    clearTimeout(pointerTimer.current);
    pointerTimer.current = setTimeout(() => setPointerActive(false), 2500);
  };
  useEffect(() => () => clearTimeout(pointerTimer.current), []);
  // Pointer moves over pages in frames (EPUB) reach these through the renderer.
  const pointerMovedRef = useRef(pointerMoved);
  useEffect(() => {
    pointerMovedRef.current = pointerMoved;
  });
  const pointerSink = useRef<((x: number, y: number) => void) | null>(null);
  const setPointerSink = useCallback((look: ((x: number, y: number) => void) | null) => {
    pointerSink.current = look;
  }, []);
  // Jumps made from the app (contents, marks, links) can be undone with Back.
  const history = useRef<{ back: Locator[]; forward: Locator[] }>({ back: [], forward: [] });
  const [zoom, setZoom] = useState<ZoomValue>(1);
  const [pageLayout, setPageLayout] = useState<PageLayout | null>(null);
  const openHelper = useHelperDialog((s) => s.open);
  const [pageInput, setPageInput] = useState("");
  // Edit pages mode (PDF), filled-in forms, version history.
  const [editing, setEditing] = useState(false);
  const [formDirty, setFormDirty] = useState(false);
  const [savingForm, setSavingForm] = useState(false);
  const [versionsOpen, setVersionsOpen] = useState(false);
  const [compareOpen, setCompareOpen] = useState(false);
  const comparing = tab.compare;
  const setCompare = useTabs((s) => s.setCompare);
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
    const locator = JSON.stringify(loc.locator);
    // Keep the cached place current, so opening again comes back here.
    qc.setQueryData(positionKey(bookId), locator);
    void savePosition(bookId, locator, loc.progress)
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
          if (link.page) void rendererRef.current?.goTo({ type: "pdf", page: link.page });
          const a = annotationsRef.current.find((x) => x.id === link.annotation);
          if (a) void rendererRef.current?.showAnnotation(a);
        } else {
          openTab({
            bookId: link.bookId,
            title: "Book",
            fileType: "pdf",
            jumpTo: link.page ? pageJump(link.page) : link.annotation,
          });
        }
      } else if (openPodcastLink(href)) {
        // An episode's moment plays in the podcast player.
      } else if (/^(https?:|mailto:)/i.test(href)) {
        void commands.openExternalUrl(href).then((r) => {
          if (r.status === "error") toast.error(r.error.message);
        });
      }
    },
    [bookId, openTab],
  );

  // Create the renderer once the book and its saved position are known.
  const relPath = doc ? doc.file : book?.relPath;
  const fileType = doc ? tab.fileType : book?.fileType;
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
    const key = `${bookId}:${attempt}`;
    // The start page shown while the saved place is restored is not saved.
    openedRef.current = false;
    void (async () => {
      try {
        renderer = await createRenderer(
          fileType,
          {
            relocate: (loc) => {
              // A renderer being closed may still report a move: ignore it.
              if (cancelled) return;
              setLocation(loc);
              useListening.getState().setPlace(bookId, {
                locator: JSON.stringify(loc.locator),
                progress: loc.progress,
                label: loc.shortLabel,
              });
              // Ignore the start page shown while the saved place is restored.
              if (!openedRef.current) return;
              pending.current = loc;
              clearTimeout(saveTimer.current);
              saveTimer.current = setTimeout(flushPosition, 1200);
            },
            selection: (sel) => {
              if (cancelled) return;
              setSelection(sel);
              if (sel) setMenu(null);
            },
            annotationClick: (id, rect) => setMenu({ id, rect, edit: false }),
            mathClick: (latex, rect) => setMath({ latex, rect, from: "book" }),
            externalLink: (href) => linkRef.current(href),
            formChanged: (dirty) => setFormDirty(dirty),
            pointer: (x, y) => {
              pointerSink.current?.(x, y);
              pointerMovedRef.current();
            },
          },
          bookId,
        );
        if (cancelled) return;
        const resume = useProfilePrefs.getState().prefs.reader.resume;
        // The latest saved place (the query is read only once).
        const saved = qc.getQueryData<string | null>(positionKey(bookId)) ?? position.data;
        await renderer.open(host, bookUrl(relPath), resume ? parseLocator(saved) : null);
        if (cancelled) {
          renderer.destroy();
          return;
        }
        rendererRef.current = renderer;
        openedRef.current = true;
        const { reader } = useProfilePrefs.getState().prefs;
        renderer.setLineHeight(reader.lineHeight);
        if (!renderer.paged && reader.fontScale !== 100) renderer.setZoom(reader.fontScale / 100);
        setZoom(renderer.zoom());
        setPageLayout(renderer.layoutOptions?.() ?? null);
        setOpened({ key, status: "ready", toc: renderer.toc(), error: null });
      } catch (e) {
        if (!cancelled) {
          const error = e instanceof Error ? e.message : String(e);
          setOpened({ key, status: "error", toc: [], error });
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

  // A theme change repaints the book's pages, which is slow for big PDFs:
  // tabs in the background catch up when they are shown, and the shown
  // one repaints after the app has switched colours.
  const themed = useRef<string | null>(null);
  useEffect(() => {
    if (status !== "ready" || !active) return;
    // Keyed by the open renderer too: a reopened book is themed afresh.
    const key = `${current?.key}|${JSON.stringify(theme)}|${prefs.pdfMode}`;
    if (themed.current === key) return;
    const id = requestAnimationFrame(() => {
      themed.current = key;
      rendererRef.current?.setTheme(theme, prefs.pdfMode);
    });
    return () => cancelAnimationFrame(id);
  }, [theme, prefs.pdfMode, status, active, current?.key]);

  // Jump to an annotation when the tab was opened from a link.
  useEffect(() => {
    if (status !== "ready" || !tab.jumpTo) return;
    const page = /^page:(\d+)$/.exec(tab.jumpTo);
    if (page) {
      void rendererRef.current?.goTo({ type: "pdf", page: Number(page[1]) });
      clearJump(bookId);
      return;
    }
    if (!annotations.length) return;
    const a = annotations.find((x) => x.id === tab.jumpTo);
    if (a) void rendererRef.current?.showAnnotation(a);
    clearJump(bookId);
  }, [status, tab.jumpTo, annotations, bookId, clearJump]);

  // Open at a search match: go where it was found, then find the words.
  const [findKey, setFindKey] = useState(0);
  useEffect(() => {
    const f = tab.findText;
    if (status !== "ready" || !f) return;
    clearJump(bookId);
    const renderer = rendererRef.current;
    if (!renderer) return;
    void (async () => {
      renderer.clearFind();
      try {
        await renderer.prepareFind?.({ page: f.page, section: f.section });
      } catch {
        /* the place may be gone; find from here */
      }
      setLastQuery(f.query);
      setFindKey((k) => k + 1);
      setFindOpen(true);
      setFindStep((s) => ({ backwards: false, seq: s.seq + 1 }));
    })();
  }, [status, tab.findText, bookId, clearJump]);

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

  // ADHD reading (Settings › Reader), unless paused for now. Only the tab
  // shown is changed; the others catch up when shown.
  const adhd = useAdhd();
  const setAdhdPaused = useAdhdPause((s) => s.setPaused);
  const bionicHere = canBionic(fileType);
  const bionic = adhd.live && adhd.bionic && bionicHere;
  useEffect(() => {
    if (status !== "ready" || !active) return;
    rendererRef.current?.setBionic?.(bionic ? { fixation: adhd.fixation, fade: adhd.fade } : null);
  }, [bionic, adhd.fixation, adhd.fade, status, active, current?.key]);
  const findLine = useCallback((x: number, y: number) => {
    const renderer = rendererRef.current;
    if (!renderer) return null;
    return renderer.lineAt ? renderer.lineAt(x, y) : lineAt(document, x, y);
  }, []);

  const r = () => rendererRef.current;
  const isPdf = isPaged(fileType);
  const readAloud = useReadAloud(rendererRef);
  // Study: where reading is, for the timer and reading goals.
  const studyTitle = book?.metadata.title ?? tab.title;
  useEffect(() => {
    if (!active || !location) return;
    reportReading({
      bookId,
      title: studyTitle,
      page: location.page ?? null,
      pages: location.pages ?? null,
      progress: location.progress,
    });
  }, [active, location, bookId, studyTitle]);
  useEffect(() => {
    if (!active) return;
    return () => reportReading(null);
  }, [active]);
  // A focus break pauses reading aloud.
  const readStatus = readAloud.status;
  const pauseReading = readAloud.pause;
  useEffect(
    () =>
      onBreak(() => {
        if (active && readStatus === "playing") pauseReading();
      }),
    [active, readStatus, pauseReading],
  );
  // The floating player: read aloud, or the linked audiobook.
  const [listen, setListen] = useState<ListenMode | null>(null);
  // Read aloud stopped by itself (the end, no text, a voice that failed):
  // let the podcast bar, if any, come back.
  const [seenStatus, setSeenStatus] = useState(readAloud.status);
  if (seenStatus !== readAloud.status) {
    setSeenStatus(readAloud.status);
    if (readAloud.status === "off" && listen === "read") setListen(null);
  }
  const toggleReadAloud = () => {
    if (listen === "read" && readAloud.status !== "off") {
      readAloud.stop();
      setListen(null);
    } else {
      setListen("read");
      void readAloud.start();
    }
  };
  const listenWith = (m: ListenMode) => {
    if (m === "read") {
      // One voice at a time.
      const p = usePlayer.getState();
      if (p.playing) p.toggle();
      void readAloud.start();
    } else readAloud.stop();
    setListen(m);
  };
  const podcastLoaded = usePlayer((s) => s.episode !== null);
  // A podcast playing shows in the book's floating player by itself (the
  // window's own podcast player hides over books); the toolbar button
  // hides it.
  const [podcastHidden, setPodcastHidden] = useState(false);
  const mode: ListenMode | null = listen ?? (podcastLoaded && !podcastHidden ? "podcast" : null);
  const stopListening = () => {
    readAloud.stop();
    setListen(null);
  };
  const showListenBar =
    mode === "audio" ||
    (mode === "podcast" && podcastLoaded) ||
    (mode === "read" && readAloud.status !== "off");

  // An audiobook of this book is playing: follow it.
  const followTo = useListening((s) => s.follow[bookId]);
  useEffect(() => {
    if (!followTo || status !== "ready" || readAloud.status !== "off") return;
    void rendererRef.current?.goToFraction?.(followTo.progress);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [followTo, status]);
  const { data: audiobooks = [] } = useQuery({
    queryKey: ["lib", "reader", bookId, "audiobooks"],
    queryFn: () => unwrap(commands.audiobooksFor(bookId)),
    enabled: status === "ready" && !doc,
  });
  /** Plays the linked audiobook from the place being read, in the
   * floating player (the full player opens from there). */
  const listenHere = () => {
    if (audiobooks.length) listenWith("audio");
  };

  // Markup mode (fixed pages): drawings kept like highlights.
  const markup = useMarkup({
    bookId,
    enabled: isPdf && !doc,
    ready: status === "ready",
    isPdf: fileType === "pdf",
    renderer: rendererRef,
    annotations,
    pages: location?.pages ?? 0,
    tabActive: active,
    save: (a) =>
      saveAnnotation.mutate(a, {
        onError: (e) => toast.error("Could not save the markup", { description: String(e) }),
      }),
    remove: (id) => deleteAnnotation.mutate(id),
  });
  const [signatureOpen, setSignatureOpen] = useState(false);
  const [stampOpen, setStampOpen] = useState(false);
  const [exportOpen, setExportOpen] = useState(false);
  const [exporting, setExporting] = useState(false);
  const updateProfilePrefs = useProfilePrefs((s) => s.update);
  const { editLibrary } = usePermissions();
  const markupPrefs = useProfilePrefs((s) => s.prefs.markup);
  const pickImage = async () => {
    const p = await pickPicture();
    if (!p) return;
    markup.setStyle({ image: { ...p, signature: false } });
    markup.setTool("image");
    toast("Click the page to place the picture");
  };
  const { needImage, clearNeedImage } = markup;
  useEffect(() => {
    if (!needImage) return;
    clearNeedImage();
    void pickImage();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [needImage]);
  const exportMarkedUp = async (addToLibrary: boolean) => {
    const title = (book?.metadata.title ?? tab.title).replace(/[\\/:*?"<>|]+/g, " ").trim();
    const dest = await saveDialog({
      defaultPath: `${title} (marked up).pdf`,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!dest) return;
    setExporting(true);
    try {
      const pages = drawList(markup.visibleMarks(), markup.pageAspect);
      const r = await commands.exportMarkedUp(bookId, dest, pages, addToLibrary);
      if (r.status === "error") throw new Error(r.error.message);
      setExportOpen(false);
      toast.success("Marked-up copy saved", {
        description: dest,
        action: { label: "Show", onClick: () => void commands.revealPath(dest) },
      });
    } catch (e) {
      toast.error("The copy could not be made", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setExporting(false);
    }
  };
  /** The file changed (a new version): the tab follows the book's new id. */
  const fileChanged = (next: { id: string }) => {
    setEditing(false);
    setFormDirty(false);
    // Save the place first, so the book reopens where it was.
    flushPosition();
    void qc.invalidateQueries({ queryKey: ["lib"] });
    if (next.id !== bookId) useTabs.getState().replaceBook(bookId, next.id);
    else setAttempt((a) => a + 1);
  };
  const saveIntoPdf = async () => {
    setExporting(true);
    try {
      const annots = pdfAnnots(markup.visibleMarks(), markup.pageAspect);
      const saved = await unwrap(commands.saveMarkupIntoPdf(bookId, annots));
      setExportOpen(false);
      toast.success("Markup saved into the PDF", {
        description: "Other PDF apps now show it. The earlier file is kept in Version history.",
      });
      fileChanged(saved);
    } catch (e) {
      toast.error("The markup could not be saved into the PDF", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setExporting(false);
    }
  };
  const saveForm = async () => {
    const renderer = r();
    if (!renderer?.saveForm) return;
    setSavingForm(true);
    try {
      const bytes = await renderer.saveForm();
      let binary = "";
      for (let i = 0; i < bytes.length; i += 0x8000)
        binary += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
      const saved = await unwrap(commands.saveFilledForm(bookId, btoa(binary)));
      toast.success("Form saved", { description: "The earlier file is kept in Version history." });
      fileChanged(saved);
    } catch (e) {
      toast.error("The form could not be saved", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setSavingForm(false);
    }
  };
  const { data: ocrPages = [] } = useQuery({
    queryKey: ["lib", "reader", bookId, "ocr-pages"],
    queryFn: () => unwrap(commands.ocrPages(bookId)),
    enabled: editing,
  });
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
  // Pinch on a trackpad or touch screen, or Ctrl/⌘ + wheel: zoom pages,
  // or change the text size of reflowing books.
  usePinchZoom(hostRef, status === "ready" && !editing && !comparing, (factor, x, y) => {
    const renderer = r();
    if (!renderer) return;
    if (renderer.zoomBy) renderer.zoomBy(factor, x, y);
    else {
      const cur = renderer.zoom();
      const n = typeof cur === "number" ? cur : 1;
      const [lo, hi] = isPdf ? [0.25, 5] : [0.5, 3];
      renderer.setZoom(Math.round(Math.min(hi, Math.max(lo, n * factor)) * 100) / 100);
    }
    setZoom(renderer.zoom());
  });

  const bookmarkHere = annotations.find(
    (a) =>
      a.kind === "bookmark" &&
      location &&
      (location.page !== undefined
        ? parseLocator(a.locator)?.type === "pdf" &&
          (parseLocator(a.locator) as { page: number }).page === location.page
        : samePlace(parseLocator(a.locator), a.position, location)),
  );

  const toggleBookmark = () => {
    if (doc) return notForDocs();
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
    if (doc) return notForDocs();
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

  // Voice notes: about the selected text, or about the place being read.
  const voice = useVoiceNote(book?.metadata.language);
  const voiceAt = useRef<{
    locator: Locator;
    quote: TextQuote | null;
    label: string | null;
    position: number;
  } | null>(null);
  const startVoice = (fromSelection: boolean) => {
    if (doc) return notForDocs();
    const sel = fromSelection ? selection : null;
    if (sel) {
      voiceAt.current = {
        locator: sel.locator,
        quote: sel.quote,
        label: sel.label || location?.shortLabel || null,
        position: sel.position,
      };
      r()?.clearSelection();
      setSelection(null);
    } else if (location) {
      voiceAt.current = {
        locator:
          location.locator.type === "pdf"
            ? { type: "pdf", page: location.page ?? 1 }
            : location.locator,
        quote: null,
        label:
          location.section && location.page === undefined ? location.section : location.shortLabel,
        position: location.progress,
      };
    } else return;
    void voice.start();
  };
  const saveVoice = (rec: RecordedVoice) => {
    const at = voiceAt.current;
    if (!at) return;
    saveAnnotation.mutate(
      {
        id: newId(),
        bookId,
        kind: "voice",
        color: null,
        locator: JSON.stringify({
          ...at.locator,
          audio: rec.path,
          duration: Math.round(rec.duration * 10) / 10,
        }),
        quote: at.quote,
        note: rec.transcript,
        label: at.label,
        position: at.position,
        createdAt: "",
        modifiedAt: "",
      },
      {
        onSuccess: () => toast.success("Voice note saved"),
        onError: (e) => toast.error("Could not save the voice note", { description: String(e) }),
      },
    );
  };

  useEffect(() => {
    if (!clipping) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      e.preventDefault();
      setClipping(null);
      setClipBox(null);
      clipping.done(null);
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [clipping]);

  /** Lets the person draw a box on the page and clips the figure in it. */
  const clipFromPage = async () => {
    const renderer = r();
    if (!renderer?.clipPicture) return null;
    const rect = await new Promise<DOMRect | null>((done) =>
      setClipping({ done, what: "the figure" }),
    );
    if (!rect) return null;
    const clip = await renderer.clipPicture(rect);
    if (!clip) {
      toast("Nothing to clip there", { description: "Draw the box over a page." });
      return null;
    }
    const box = clip.box.map((v) => Math.round(v * 10000) / 10000).join(",");
    return {
      clip,
      link: `libreri://book/${bookId}#page=${clip.page}&rect=${box}`,
      label: `${book?.metadata.title ?? "Book"}, p. ${clip.page}`,
    };
  };

  /** Reads maths from a box drawn on the page, with the model turned on
   * in Settings › Writing. */
  const { data: mathsModel } = useMathsSettings();
  const [readingMaths, setReadingMaths] = useState(false);
  const mathsFromPage = async () => {
    const renderer = r();
    if (!renderer?.clipPicture) return;
    const rect = await new Promise<DOMRect | null>((done) =>
      setClipping({ done, what: "the formula" }),
    );
    if (rect) await readMathsIn(rect);
  };
  /** Reads the maths in a part of the page with the maths model. */
  const readMathsIn = async (rect: DOMRect) => {
    const renderer = r();
    if (!renderer?.clipPicture) return;
    const clip = await renderer.clipPicture(rect);
    if (!clip) {
      toast("No maths there", { description: "Draw the box over a page." });
      return;
    }
    setReadingMaths(true);
    try {
      const latex = await mathsFromPicture(clip.dataUrl);
      if (!latex.trim()) toast("No maths was found in the box");
      else setMath({ latex, rect, from: "picture" });
    } catch (e) {
      toast.error("Could not read the maths", { description: String((e as Error).message ?? e) });
    } finally {
      setReadingMaths(false);
    }
  };

  // Links to web pages, videos, files and other books (Phase 8c).
  /** Adding a link: where it goes ("p. 4", or the selected words). */
  const [linking, setLinking] = useState<string | null>(null);
  const linkAt = useRef<typeof voiceAt.current>(null);
  const [playing, setPlaying] = useState<Annotation | null>(null);
  const [copyView, setCopyView] = useState<{ copy: string; url: string; title: string } | null>(
    null,
  );
  const links = annotations.filter((a) => a.kind === "link");
  const startLink = (fromSelection: boolean) => {
    if (doc) return notForDocs();
    const sel = fromSelection ? selection : null;
    if (sel) {
      linkAt.current = {
        locator: sel.locator,
        quote: sel.quote,
        label: sel.label || location?.shortLabel || null,
        position: sel.position,
      };
      r()?.clearSelection();
      setSelection(null);
    } else if (location) {
      linkAt.current = {
        locator:
          location.locator.type === "pdf"
            ? { type: "pdf", page: location.page ?? 1 }
            : location.locator,
        quote: null,
        label:
          location.section && location.page === undefined ? location.section : location.shortLabel,
        position: location.progress,
      };
    } else return;
    const q = linkAt.current.quote?.exact;
    setLinking(
      q ? `“${q.slice(0, 60)}${q.length > 60 ? "…" : ""}”` : (linkAt.current.label ?? "this place"),
    );
  };
  const savedLink = (link: LinkInfo, note: string | null) => {
    setLinking(null);
    const at = linkAt.current;
    if (!at) return;
    const a: Annotation = {
      id: newId(),
      bookId,
      kind: "link",
      color: null,
      locator: JSON.stringify({ ...at.locator, link }),
      quote: at.quote,
      note,
      label: at.label,
      position: at.position,
      createdAt: "",
      modifiedAt: "",
    };
    saveAnnotation.mutate(a, {
      onSuccess: () => {
        toast.success("Link added", { description: link.title });
        setLeft("links");
        setLastLeft("links");
      },
      onError: (e) => toast.error("Could not add the link", { description: String(e) }),
    });
  };
  const linkAction = (a: Annotation, what: "play" | "open" | "copy") => {
    const link = linkOf(a.locator);
    if (!link) return;
    if (what === "play") {
      setPlaying(a);
      setLeft("links");
      setLastLeft("links");
    } else if (what === "copy" && link.copy) {
      setCopyView({ copy: link.copy, url: link.url, title: link.title });
    } else if (link.kind === "book") handleLink(link.url);
    else if (link.url)
      void commands.openExternalUrl(link.url).then((r) => {
        if (r.status === "error") toast.error(r.error.message);
      });
  };
  const pageLinks =
    location?.page !== undefined
      ? links.filter((a) => {
          try {
            return (JSON.parse(a.locator) as { page?: number }).page === location.page;
          } catch {
            return false;
          }
        }).length
      : 0;

  // Paper notes: photographed pages saved as a PDF, linked to this place.
  const [capturing, setCapturing] = useState(false);
  const captureAt = useRef<typeof voiceAt.current>(null);
  const [viewing, setViewing] = useState<{ path: string; title: string } | null>(null);
  const startCapture = () => {
    if (doc) return notForDocs();
    if (!location) return;
    captureAt.current = {
      locator:
        location.locator.type === "pdf"
          ? { type: "pdf", page: location.page ?? 1 }
          : location.locator,
      quote: null,
      label:
        location.section && location.page === undefined ? location.section : location.shortLabel,
      position: location.progress,
    };
    setCapturing(true);
  };
  const savedCapture = (saved: { path: string; pages: number; text: string }, title: string) => {
    setCapturing(false);
    const at = captureAt.current;
    if (!at) return;
    saveAnnotation.mutate(
      {
        id: newId(),
        bookId,
        kind: "capture",
        color: null,
        locator: JSON.stringify({ ...at.locator, capture: saved.path, pages: saved.pages, title }),
        quote: null,
        note: saved.text.slice(0, 4000) || null,
        label: at.label,
        position: at.position,
        createdAt: "",
        modifiedAt: "",
      },
      {
        onSuccess: () =>
          toast.success(`${saved.pages} ${saved.pages === 1 ? "page" : "pages"} saved`, {
            description: saved.path,
            action: { label: "Show", onClick: () => setViewing({ path: saved.path, title }) },
          }),
        onError: (e) => toast.error("Could not link the pages", { description: String(e) }),
      },
    );
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
  // The highlight last gone to with next/previous highlight.
  const lastHighlight = useRef<string | null>(null);
  const nextHighlight = (dir: 1 | -1) => {
    const here = location?.progress ?? 0;
    const list = annotations
      .filter((a) => a.kind === "highlight")
      .sort((a, b) => (a.position ?? 0) - (b.position ?? 0));
    // Still at the highlight gone to last (showing it can stop a little
    // before it): step from it in the list, not from the place shown.
    const near = location?.pages ? 1 / location.pages : 0.01;
    const last = list.findIndex((a) => a.id === lastHighlight.current);
    const target =
      last >= 0 && Math.abs((list[last]!.position ?? 0) - here) < near
        ? list[last + dir]
        : dir > 0
          ? list.find((a) => (a.position ?? 0) > here + 0.0005)
          : [...list].reverse().find((a) => (a.position ?? 0) < here - 0.0005);
    if (target) {
      lastHighlight.current = target.id;
      jump(() => r()?.showAnnotation(target));
    } else toast(dir > 0 ? "No more highlights after this page" : "No highlights before this page");
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
  useShortcut("reader.print", () => fileType === "pdf" && status === "ready" && setPrinting(true));
  useShortcut("reader.bookmark", toggleBookmark);
  useShortcut("reader.contents", () => setLeft((p) => (p ? null : lastLeft)));
  useShortcut("reader.notebook", () => (doc ? notForDocs() : setNotebookOpen((o) => !o)));
  useShortcut("reader.themeNext", cycleTheme);
  useShortcut("reader.pdfModeNext", cyclePdfMode);
  useShortcut("reader.focusMode", () => setFocusMode((f) => !f));
  useShortcut("reader.markup", () => markup.available && markup.setActive(!markup.active));
  useShortcut("reader.readAloud", toggleReadAloud);
  useShortcut("reader.details", () => {
    if (doc) return;
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
  useShortcut("reader.addLink", () => startLink(!!selection));
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
    <div className="relative flex h-full min-h-0 flex-col">
      {/* Toolbar */}
      <div
        className={cn(
          "flex h-11 shrink-0 items-center gap-1 border-b bg-background px-2",
          focusMode && "hidden",
          immersive &&
            "absolute inset-x-0 top-0 z-40 shadow-md transition-[translate,opacity] duration-150",
          immersive && edge !== "top" && "pointer-events-none -translate-y-full opacity-0",
        )}
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
            {doc
              ? `${doc.source} · from Feeds, not in your library yet`
              : (location?.section ?? book?.metadata.authors.join(", "))}
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
        {isPdf && status === "ready" && !comparing && (
          <div
            className="flex shrink-0 items-center rounded-md border"
            role="group"
            aria-label="Zoom"
            title="Zoom: pinch, or Ctrl/⌘ + scroll"
          >
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              aria-label={`Zoom out (${keys("reader.zoomOut")})`}
              onClick={() => changeZoom(-1)}
            >
              <ZoomOut />
            </Button>
            <button
              type="button"
              className="w-12 text-center text-[12px] tabular-nums hover:text-foreground"
              aria-label={`${zoomLabel}: fit automatically (${keys("reader.zoomReset")})`}
              title="Fit automatically"
              onClick={() => changeZoom(0)}
            >
              {typeof zoom === "number" ? `${Math.round(zoom * 100)}%` : "Fit"}
            </button>
            <Button
              variant="ghost"
              size="icon"
              className="size-7"
              aria-label={`Zoom in (${keys("reader.zoomIn")})`}
              onClick={() => changeZoom(1)}
            >
              <ZoomIn />
            </Button>
          </div>
        )}
        {markup.available && (
          <div className="mr-1 flex rounded-md border p-0.5" role="radiogroup" aria-label="Mode">
            {(
              [
                ["read", "Read", "Read"],
                [
                  "markup",
                  "Markup",
                  `Markup: draw and write on the pages (${keys("reader.markup")})`,
                ],
                ...(fileType === "pdf" && editLibrary
                  ? ([
                      ["edit", "Edit pages", "Edit pages: reorder, turn, crop, redact, correct"],
                    ] as const)
                  : []),
              ] as const
            ).map(([id, label, title]) => {
              const on = id === "edit" ? editing : !editing && (id === "markup") === markup.active;
              return (
                <button
                  key={id}
                  type="button"
                  role="radio"
                  aria-checked={on}
                  title={title}
                  disabled={status !== "ready"}
                  onClick={() => {
                    if (id === "edit") {
                      markup.setActive(false);
                      setEditing(true);
                    } else {
                      setEditing(false);
                      markup.setActive(id === "markup");
                    }
                  }}
                  className={cn(
                    "h-6 rounded px-2.5 text-[12.5px]",
                    on
                      ? "bg-muted font-medium text-foreground"
                      : "text-muted-foreground hover:text-foreground",
                  )}
                >
                  {label}
                </button>
              );
            })}
          </div>
        )}
        {status === "ready" &&
          fileType &&
          !["cbz", "cbr", "cb7", "cbt", "cba"].includes(fileType) && (
            <Button
              variant="ghost"
              size="icon"
              aria-label="Read aloud"
              aria-pressed={listen === "read" && readAloud.status !== "off"}
              title={`Read aloud from here (${keys("reader.readAloud")})`}
              onClick={toggleReadAloud}
            >
              <Volume2 />
            </Button>
          )}
        {audiobooks.length > 0 && (
          <Button
            variant="ghost"
            size="icon"
            aria-label="Listen from here"
            title={`Listen from here: ${audiobooks[0]!.metadata.title}`}
            aria-pressed={listen === "audio"}
            onClick={() => (listen === "audio" ? stopListening() : listenHere())}
          >
            <Headphones />
          </Button>
        )}
        {podcastLoaded && (
          <Button
            variant="ghost"
            size="icon"
            aria-label="Podcast"
            title="Show the podcast here"
            aria-pressed={mode === "podcast"}
            onClick={() => {
              if (mode === "podcast") {
                setPodcastHidden(true);
                setListen(null);
              } else {
                setPodcastHidden(false);
                listenWith("podcast");
              }
            }}
          >
            <Podcast />
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon"
          aria-label="Find in book"
          title={`Find in book (${keys("reader.find")})`}
          onClick={() => setFindOpen(true)}
        >
          <Search />
        </Button>
        {fileType === "pdf" && status === "ready" && !comparing && (
          <Button
            variant="ghost"
            size="icon"
            aria-label="Print"
            title={`Print (${keys("reader.print")})`}
            onClick={() => setPrinting(true)}
          >
            <Printer />
          </Button>
        )}
        <TimerButton />
        <AppearanceMenu
          isPdf={isPdf}
          zoomLabel={zoomLabel}
          onZoom={changeZoom}
          pageLayout={pageLayout}
          onPageLayout={changeLayout}
          bionicHere={bionicHere}
        />
        {!doc && (
          <>
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
            {(fileType === "pdf" || fileType === "djvu") && (
              <Button
                variant="ghost"
                size="icon"
                aria-label="Compare"
                aria-pressed={Boolean(comparing)}
                title="Compare with an earlier version, another book or a file"
                onClick={() => (comparing ? setCompare(bookId, null) : setCompareOpen(true))}
              >
                <GitCompare />
              </Button>
            )}
            {fileType === "pdf" && (
              <Button
                variant="ghost"
                size="icon"
                aria-label="Version history"
                title="Version history"
                onClick={() => setVersionsOpen(true)}
              >
                <History />
              </Button>
            )}
            <Button
              variant="ghost"
              size="icon"
              aria-label="Record a voice note here"
              title="Record a voice note about this page (select text first to note a passage)"
              disabled={!location || voice.phase !== "idle"}
              onClick={() => startVoice(false)}
            >
              <Mic />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Capture paper notes"
              title="Capture paper notes with the camera, your phone or pictures"
              disabled={!location}
              onClick={startCapture}
            >
              <Camera />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Add a link here"
              title={`Link a web page, video or recording to this page (${keys("reader.addLink")})`}
              disabled={!location}
              onClick={() => startLink(false)}
            >
              <Link2 />
            </Button>
          </>
        )}
        {mathsModel?.on && mathsModel.downloaded && (fileType === "pdf" || isPaged(fileType)) && (
          <Button
            variant="ghost"
            size="icon"
            aria-label="Read maths from the page"
            title="Read maths from the page as LaTeX: draw a box around a formula"
            disabled={readingMaths || !!clipping}
            onClick={() => void mathsFromPage()}
          >
            {readingMaths ? <Loader2 className="animate-spin" /> : <SquareSigma />}
          </Button>
        )}
        {doc ? (
          editLibrary && (
            <Button
              size="sm"
              className="ml-1"
              onClick={() => setAddingDoc(true)}
              title="Add it to your library, to highlight and keep notes"
            >
              <BookPlus /> Add to library…
            </Button>
          )
        ) : (
          <>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Canvas"
              aria-pressed={canvasOpen}
              title="Canvas: write and draw by hand"
              onClick={() => setCanvasOpen((o) => !o)}
            >
              <PenLine />
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
          </>
        )}
      </div>

      <VoiceNoteBar
        v={voice}
        onDone={saveVoice}
        className="h-10 shrink-0 border-b bg-muted/40 px-3"
      />
      {comparing && (
        <CompareView
          key={JSON.stringify(comparing)}
          request={comparing}
          title={book?.metadata.title ?? tab.title}
          onSwap={() => setCompare(bookId, { a: comparing.b, b: comparing.a })}
          onClose={() => setCompare(bookId, null)}
        />
      )}
      {formDirty && !editing && !comparing && (
        <div className="flex h-10 shrink-0 items-center gap-3 border-b bg-primary/5 px-3 text-[13px]">
          <span className="flex-1">
            You filled in the form. Save it into the PDF, or undo your changes.
          </span>
          <Button
            variant="ghost"
            size="sm"
            onClick={() => {
              setFormDirty(false);
              setAttempt((a) => a + 1);
            }}
          >
            Undo changes
          </Button>
          {editLibrary && (
            <Button size="sm" disabled={savingForm} onClick={() => void saveForm()}>
              {savingForm ? "Saving…" : "Save form"}
            </Button>
          )}
        </div>
      )}

      {markup.active && !bare && !editing && !comparing && (
        <MarkupToolbar
          m={markup}
          onPickImage={() => void pickImage()}
          onNewSignature={() => setSignatureOpen(true)}
          onNewStamp={() => setStampOpen(true)}
          onExport={() => setExportOpen(true)}
          layersOpen={left === "markup"}
          onLayers={() => {
            if (left === "markup") setLeft(null);
            else {
              setLeft("markup");
              setLastLeft("markup");
            }
          }}
        />
      )}

      {editing && book && !comparing && (
        <PageEditor
          book={book}
          hasOcr={ocrPages.length > 0}
          canEdit={editLibrary}
          onClose={() => setEditing(false)}
          onSaved={fileChanged}
        />
      )}
      <div className={cn("relative flex min-h-0 flex-1", (editing || comparing) && "hidden")}>
        {left && !bare && (
          // A narrow window: contents and marks lie over the page.
          <div
            className={cn(
              "flex",
              !roomy && "absolute inset-y-0 left-0 z-30 max-w-[85%] shadow-2xl",
            )}
          >
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
              onOpenCapture={(path, title) => setViewing({ path, title })}
              linkCount={links.length}
              links={
                <LinksPanel
                  links={links}
                  page={location?.page ?? null}
                  playing={playing && links.some((l) => l.id === playing.id) ? playing : null}
                  onStop={() => setPlaying(null)}
                  onAdd={() => startLink(false)}
                  onPlay={(a) => linkAction(a, "play")}
                  onOpen={(a) => linkAction(a, "open")}
                  onCopy={(a) => linkAction(a, "copy")}
                  onShow={(a) => jump(() => r()?.showAnnotation(a))}
                  onDelete={(a) => {
                    if (playing?.id === a.id) setPlaying(null);
                    deleteAnnotation.mutate(a.id);
                  }}
                />
              }
              markup={
                markup.available ? (
                  <MarkupPanel
                    m={markup}
                    onShow={(mk) =>
                      jump(async () => {
                        await r()?.goTo({
                          type: "pdf",
                          page: mk.page,
                          top: Math.max(0, markupModel.bounds(mk.item)[1] - 0.1),
                        });
                        markup.selectMark(mk.id);
                      })
                    }
                  />
                ) : undefined
              }
            />
          </div>
        )}

        <div
          className="relative min-w-0 flex-1 @container"
          style={{ background: theme.surround }}
          onPointerMove={pointerMoved}
        >
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
          {adhd.live &&
            (adhd.line || adhd.mask) &&
            status === "ready" &&
            !editing &&
            !comparing &&
            !markup.active && (
              <FocusOverlay
                line={adhd.line}
                mask={adhd.mask}
                maskHeight={adhd.maskHeight}
                findLine={findLine}
                onSink={setPointerSink}
                refresh={location}
              />
            )}
          {clipping && (
            <div
              className="absolute inset-0 z-20 cursor-crosshair bg-black/10"
              role="application"
              aria-label="Draw a box around what to clip"
              onPointerDown={(e) => {
                e.currentTarget.setPointerCapture(e.pointerId);
                setClipBox({ x0: e.clientX, y0: e.clientY, x1: e.clientX, y1: e.clientY });
              }}
              onPointerMove={(e) =>
                setClipBox((b) => (b ? { ...b, x1: e.clientX, y1: e.clientY } : b))
              }
              onPointerUp={() => {
                const b = clipBox;
                setClipBox(null);
                const done = clipping.done;
                setClipping(null);
                if (!b || Math.abs(b.x1 - b.x0) < 8 || Math.abs(b.y1 - b.y0) < 8) return done(null);
                done(
                  new DOMRect(
                    Math.min(b.x0, b.x1),
                    Math.min(b.y0, b.y1),
                    Math.abs(b.x1 - b.x0),
                    Math.abs(b.y1 - b.y0),
                  ),
                );
              }}
            >
              <div className="pointer-events-none absolute top-3 left-1/2 -translate-x-1/2 rounded-md bg-foreground px-3 py-1.5 text-[12.5px] text-background shadow">
                Draw a box around {clipping.what} · Esc to cancel
              </div>
              {clipBox && (
                <div
                  className="pointer-events-none fixed border-2 border-dashed border-blue-600 bg-blue-500/10"
                  style={{
                    left: Math.min(clipBox.x0, clipBox.x1),
                    top: Math.min(clipBox.y0, clipBox.y1),
                    width: Math.abs(clipBox.x1 - clipBox.x0),
                    height: Math.abs(clipBox.y1 - clipBox.y0),
                  }}
                />
              )}
            </div>
          )}
          {showListenBar && !comparing && !editing && status === "ready" && (
            <ListenBar
              mode={mode ?? "read"}
              onMode={listenWith}
              onClose={stopListening}
              readAloud={readAloud}
              lang={book?.metadata.language ?? undefined}
              bookId={bookId}
              audiobooks={audiobooks}
              progress={location?.progress ?? 0}
            />
          )}
          {!comparing && !editing && status === "ready" && (
            <button
              type="button"
              aria-label={immersive ? "Leave full screen" : "Read full screen"}
              title={`${immersive ? "Leave full screen" : "Read full screen"} (${keys("app.fullscreen")}${immersive ? " or Esc" : ""})`}
              onClick={toggleReadingFullscreen}
              className={cn(
                "absolute right-5 bottom-5 z-20 flex size-9 items-center justify-center rounded-full border bg-background/90 text-foreground shadow-md backdrop-blur-sm transition-opacity duration-300 hover:opacity-100 focus-visible:opacity-100 [&_svg]:size-4",
                pointerActive ? "opacity-80" : "opacity-0",
                immersive && edge === "bottom" && "bottom-14",
              )}
            >
              {immersive ? <Minimize /> : <Maximize />}
            </button>
          )}
          {focusMode && !immersive && (
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
            <div className="absolute inset-0 bg-muted/40">
              <PageSkeleton />
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
                        setAttempt((a) => a + 1);
                      })
                    }
                  >
                    Install {/DjVuLibre/.test(error) ? "DjVuLibre" : "unar"}…
                  </Button>
                )}
                <Button
                  variant="outline"
                  onClick={() =>
                    void (doc
                      ? commands.feedOpenFile(doc.file)
                      : commands.openBookExternally(bookId))
                  }
                >
                  Open in another app
                </Button>
              </div>
            </div>
          )}
          {findOpen && (
            <FindBar
              key={findKey}
              step={findStep}
              initialQuery={lastQuery}
              onQuery={setLastQuery}
              onFind={(q, back) => r()?.find(q, back) ?? Promise.resolve({ current: 0, total: 0 })}
              onClose={() => {
                setFindOpen(false);
                // Opened again later, the bar must not repeat an old F3.
                setFindStep({ backwards: false, seq: 0 });
                r()?.clearFind();
              }}
            />
          )}
        </div>

        {notebookOpen && !bare && (
          <NotebookPanel
            bookId={bookId}
            insert={notebookInsert}
            onInserted={() => setNotebookInsert(null)}
            onLink={handleLink}
            onClose={() => setNotebookOpen(false)}
            lang={book?.metadata.language}
          />
        )}
        {canvasOpen && !bare && (
          <CanvasPanel
            bookId={bookId}
            dark={appDark}
            lang={book?.metadata.language}
            onLink={handleLink}
            onClose={() => setCanvasOpen(false)}
            clip={fileType === "pdf" || isPaged(fileType) ? clipFromPage : undefined}
          />
        )}
      </div>

      {active && math && (
        <MathPopover
          key={math.latex}
          latex={math.latex}
          rect={math.rect}
          from={math.from}
          onPicture={
            math.area && mathsModel?.on && mathsModel.downloaded
              ? () => {
                  const a = math.area!;
                  setMath(null);
                  // A little room around the selection, for limits and roots.
                  const pad = Math.max(6, a.height * 0.6);
                  void readMathsIn(
                    new DOMRect(a.x - pad, a.y - pad, a.width + 2 * pad, a.height + 2 * pad),
                  );
                }
              : undefined
          }
          onClose={() => setMath(null)}
          onNotebook={(block) => {
            setMath(null);
            if (doc) return notForDocs();
            setNotebookOpen(true);
            setNotebookInsert(block);
          }}
        />
      )}
      <AddLinkDialog
        key={linking ?? "no-link"}
        open={linking !== null}
        bookId={bookId}
        where={linking ?? ""}
        onClose={() => setLinking(null)}
        onSave={savedLink}
      />
      <CopyViewer
        copy={copyView?.copy ?? null}
        url={copyView?.url ?? ""}
        title={copyView?.title ?? ""}
        onClose={() => setCopyView(null)}
      />
      <CaptureDialog
        key={capturing ? "capturing" : "idle"}
        open={capturing}
        defaultTitle={`${book?.metadata.title ?? "Notes"}, ${location?.shortLabel ?? ""}`.replace(
          /, $/,
          "",
        )}
        onClose={() => setCapturing(false)}
        onSaved={savedCapture}
      />
      <CaptureViewer
        path={viewing?.path ?? null}
        title={viewing?.title ?? ""}
        onClose={() => setViewing(null)}
      />

      {/* Status bar */}
      <div
        className={cn(
          "flex h-7 shrink-0 items-center gap-3 border-t bg-background px-3 text-[11.5px] text-muted-foreground",
          ((focusMode && !immersive) || comparing) && "hidden",
          immersive &&
            "absolute inset-x-0 bottom-0 z-40 h-9 shadow-[0_-4px_12px_rgba(0,0,0,0.08)] transition-[translate,opacity] duration-150",
          immersive && edge !== "bottom" && "pointer-events-none translate-y-full opacity-0",
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
        <TodayReading
          onOpen={() => {
            useTabs.getState().activate(null);
            useLibraryView.getState().setNav({ kind: "calendar" });
          }}
        />
        {adhd.enabled && (
          <button
            type="button"
            className={cn("hover:text-foreground", adhd.live && "text-foreground")}
            aria-pressed={adhd.live}
            title={
              adhd.live
                ? "Pause ADHD reading for now (Settings › Reader keeps it on)"
                : "Turn ADHD reading back on"
            }
            onClick={() => setAdhdPaused(adhd.live)}
          >
            {!adhd.live
              ? "ADHD reading: Paused"
              : adhd.line || adhd.mask || bionicHere
                ? "ADHD reading: On"
                : "ADHD reading: not available for this book"}
          </button>
        )}
        {pageLinks > 0 && (
          <button
            type="button"
            className="flex items-center gap-1 hover:text-foreground"
            onClick={() => {
              setLeft("links");
              setLastLeft("links");
            }}
          >
            <Link2 className="size-3.5" aria-hidden />
            {pageLinks === 1 ? "1 link on this page" : `${pageLinks} links on this page`}
          </button>
        )}
        {annotations.length > 0 && (
          <button type="button" className="hover:text-foreground" onClick={() => setLeft("marks")}>
            {countLabel(annotations)}
          </button>
        )}
      </div>

      {active && selection && !menu && (
        <SelectionMenu
          rect={selection.rect}
          notes={!doc}
          onHighlight={(c) => highlightFromSelection(c)}
          onComment={() =>
            highlightFromSelection(profilePrefs.notes.defaultColor, (saved) =>
              setMenu({ id: saved.id, rect: selection.rect, edit: true }),
            )
          }
          onNotebook={() => highlightFromSelection(profilePrefs.notes.defaultColor, addToNotebook)}
          onVoice={() => startVoice(true)}
          onLink={() => startLink(true)}
          onLatex={() => {
            // PDFs and scans: rebuilt from where the characters sit, so
            // limits, scripts and fractions come through.
            const laid = selection.glyphs ? layoutToLatex(selection.glyphs()) : "";
            setMath({
              latex: laid || textToLatex(selection.quote.exact ?? ""),
              rect: selection.rect,
              from: "text",
              area: selection.glyphs ? selection.rect : undefined,
            });
            r()?.clearSelection();
            setSelection(null);
          }}
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
          lang={book?.metadata.language}
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
          onLink={(a, what) => {
            setMenu(null);
            linkAction(a, what);
          }}
        />
      )}
      {markup.noteEdit && (
        <NotePopover
          key={markup.noteEdit.mark.id}
          mark={markup.noteEdit.mark}
          rect={markup.noteEdit.rect}
          onSave={(text) => markup.saveNote(markup.noteEdit!.mark, text)}
          onDelete={() => {
            markup.deleteMark(markup.noteEdit!.mark.id);
            markup.closeNote();
          }}
          onClose={markup.closeNote}
        />
      )}
      <CalibrateDialog calibration={markup.calibration} onDone={markup.finishCalibration} />
      <SignatureDialog
        open={signatureOpen}
        onClose={() => setSignatureOpen(false)}
        onSaved={(sig) => {
          setSignatureOpen(false);
          updateProfilePrefs({
            markup: {
              signatures: [{ id: crypto.randomUUID(), ...sig }, ...markupPrefs.signatures].slice(
                0,
                5,
              ),
            },
          });
          markup.setStyle({ image: { ...sig, signature: true } });
          markup.setTool("image");
          toast("Click the page to place your signature");
        }}
      />
      <NewStampDialog
        open={stampOpen}
        onClose={() => setStampOpen(false)}
        onSaved={(text) => {
          setStampOpen(false);
          updateProfilePrefs({
            markup: { stamps: [...markupPrefs.stamps.filter((t) => t !== text), text].slice(-10) },
          });
          markup.setStyle({ stamp: text });
          markup.setTool("stamp");
        }}
      />
      <ExportMarkupDialog
        open={exportOpen}
        count={exportOpen ? markup.visibleMarks().length : 0}
        canAdd={editLibrary}
        canSaveInto={editLibrary && fileType === "pdf"}
        busy={exporting}
        onClose={() => setExportOpen(false)}
        onExport={(add) => void exportMarkedUp(add)}
        onSaveInto={() => void saveIntoPdf()}
      />
      {versionsOpen && (
        <VersionsDialog
          open
          bookId={bookId}
          title={book?.metadata.title ?? tab.title}
          canEdit={editLibrary}
          onClose={() => setVersionsOpen(false)}
          onRestored={(b) => {
            setVersionsOpen(false);
            fileChanged(b);
          }}
          onCompare={(version) => {
            setVersionsOpen(false);
            setEditing(false);
            setCompare(bookId, {
              a: { kind: "version", book: bookId, version },
              b: { kind: "book", id: bookId },
            });
          }}
        />
      )}
      <CompareDialog
        key={`compare-${compareOpen}`}
        open={compareOpen}
        bookId={bookId}
        onClose={() => setCompareOpen(false)}
        onCompare={(r) => {
          setCompareOpen(false);
          setEditing(false);
          setCompare(bookId, r);
        }}
      />
      {fileType === "pdf" && relPath && (
        <PrintDialog
          open={printing}
          url={bookUrl(relPath)}
          title={book?.metadata.title ?? tab.title}
          page={location?.page ?? 1}
          pages={location?.pages ?? 1}
          onClose={() => setPrinting(false)}
        />
      )}
      {doc && (
        <AddToLibraryDialog
          space={doc.space}
          item={addingDoc ? { id: doc.id, title: tab.title, file: doc.file } : null}
          onClose={() => setAddingDoc(false)}
          onAdded={(added) => {
            // The book takes the download's place.
            useTabs.getState().detach(bookId);
            void openBook(added);
          }}
        />
      )}
    </div>
  );
}
