import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  Bookmark,
  Camera,
  BookmarkCheck,
  GitCompare,
  Headphones,
  History,
  Mic,
  Volume2,
  NotebookPen,
  PanelLeft,
  PenLine,
  Search,
  Loader2,
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
  createRenderer,
  isAudio,
  isPaged,
  PAGE_THEMES,
  pageTheme,
  parseLocator,
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
  savePosition,
  useAnnotations,
  useDeleteAnnotation,
  usePosition,
  useSaveAnnotation,
} from "../api";
import { useAppDark } from "../hooks/useAppDark";
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
import { useListening } from "../listening/store";
import { timeAt } from "../listening/sync";
import { ReadAloudBar } from "../speech/ReadAloudBar";
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
  const [notebookOpen, setNotebookOpenRaw] = useState(false);
  // A formula shown as LaTeX (clicked, or rebuilt from selected text).
  const [math, setMath] = useState<{
    latex: string;
    rect: DOMRect;
    from: "book" | "text" | "picture";
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
  // Jumps made from the app (contents, marks, links) can be undone with Back.
  const history = useRef<{ back: Locator[]; forward: Locator[] }>({ back: [], forward: [] });
  const [zoom, setZoom] = useState<ZoomValue>(1);
  const [pageLayout, setPageLayout] = useState<PageLayout | null>(null);
  // Bumped to open the book again (after installing a helper).
  const [attempt, setAttempt] = useState(0);
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
              setSelection(sel);
              if (sel) setMenu(null);
            },
            annotationClick: (id, rect) => setMenu({ id, rect, edit: false }),
            mathClick: (latex, rect) => setMath({ latex, rect, from: "book" }),
            externalLink: (href) => linkRef.current(href),
            formChanged: (dirty) => setFormDirty(dirty),
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

  const r = () => rendererRef.current;
  const isPdf = isPaged(fileType);
  const readAloud = useReadAloud(rendererRef);
  const toggleReadAloud = () =>
    readAloud.status === "off" ? void readAloud.start() : readAloud.stop();

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
    enabled: status === "ready",
  });
  /** Opens the linked audiobook at the place being read. */
  const listenHere = async () => {
    const audio = audiobooks[0];
    if (!audio) return;
    const [link, info] = await Promise.all([
      unwrap(commands.getAudioLink(audio.id)),
      unwrap(commands.audioInfo(audio.id)),
    ]).catch(() => [null, null] as const);
    const pts = (link?.points ?? []).map((p) => ({ t: p.t ?? 0, progress: p.progress ?? 0 }));
    const t = info?.duration ? timeAt(pts, location?.progress ?? 0, info.duration) : 0;
    useListening.getState().requestPlay(audio.id, t);
    useTabs.getState().openBeside({
      bookId: audio.id,
      title: audio.metadata.title ?? "Audiobook",
      fileType: audio.fileType,
    });
  };

  // Markup mode (fixed pages): drawings kept like highlights.
  const markup = useMarkup({
    bookId,
    enabled: isPdf,
    ready: status === "ready",
    isPdf: fileType === "pdf",
    renderer: rendererRef,
    annotations,
    pages: location?.pages ?? 0,
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

  // Voice notes: about the selected text, or about the place being read.
  const voice = useVoiceNote(book?.metadata.language);
  const voiceAt = useRef<{
    locator: Locator;
    quote: TextQuote | null;
    label: string | null;
    position: number;
  } | null>(null);
  const startVoice = (fromSelection: boolean) => {
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
    if (!rect) return;
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

  // Paper notes: photographed pages saved as a PDF, linked to this place.
  const [capturing, setCapturing] = useState(false);
  const captureAt = useRef<typeof voiceAt.current>(null);
  const [viewing, setViewing] = useState<{ path: string; title: string } | null>(null);
  const startCapture = () => {
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
  useShortcut("reader.markup", () => markup.available && markup.setActive(!markup.active));
  useShortcut("reader.readAloud", toggleReadAloud);
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
              aria-pressed={readAloud.status !== "off"}
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
            onClick={() => void listenHere()}
          >
            <Headphones />
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
      </div>

      <VoiceNoteBar
        v={voice}
        onDone={saveVoice}
        className="h-10 shrink-0 border-b bg-muted/40 px-3"
      />
      {!focusMode && !comparing && !editing && (
        <ReadAloudBar r={readAloud} lang={book?.metadata.language ?? undefined} />
      )}
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

      {markup.active && !focusMode && !editing && !comparing && (
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
      <div className={cn("flex min-h-0 flex-1", (editing || comparing) && "hidden")}>
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
            onOpenCapture={(path, title) => setViewing({ path, title })}
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
              key={findKey}
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
            lang={book?.metadata.language}
          />
        )}
        {canvasOpen && !focusMode && (
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
          onClose={() => setMath(null)}
          onNotebook={(block) => {
            setMath(null);
            setNotebookOpen(true);
            setNotebookInsert(block);
          }}
        />
      )}
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
          "flex h-7 shrink-0 items-center gap-3 border-t px-3 text-[11.5px] text-muted-foreground",
          (focusMode || comparing) && "hidden",
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
          onVoice={() => startVoice(true)}
          onLatex={() => {
            setMath({
              latex: textToLatex(selection.quote.exact ?? ""),
              rect: selection.rect,
              from: "text",
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
    </div>
  );
}
