import { ResizablePanel } from "@/components/ResizablePanel";
import { ThumbsSkeleton } from "@/components/Placeholders";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  ChevronDown,
  Copy,
  Crop,
  EyeOff,
  FilePlus2,
  History,
  ImagePlus,
  Loader2,
  Minimize2,
  MoreHorizontal,
  Redo2,
  RotateCcw,
  RotateCw,
  ScanText,
  Smartphone,
  SquarePen,
  Trash2,
  Undo2,
  Camera,
  BookPlus,
  File,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DropdownMenu,
  menuContent,
  menuItem,
  menuLabel,
  menuSeparator,
} from "@/components/ui/menu";
import { bookUrl, commands, unwrap, type BookDto, type Correction } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { pickPicture } from "../markup/pickPicture";
import {
  CameraPagesDialog,
  CompressDialog,
  NewBooksDialog,
  PdfPagesDialog,
  PhonePagesDialog,
  VersionsDialog,
  type NewBooksRequest,
} from "./EditDialogs";
import * as M from "./model";
import { PageDetail, type DetailMode } from "./PageDetail";
import { findBoxes, PdfPages, type PageText } from "./pdfPages";

const THUMB = 150;

interface History {
  past: M.EditState[];
  present: M.EditState;
  future: M.EditState[];
}

type Dialog =
  | "file"
  | "book"
  | "camera"
  | "phone"
  | "compress"
  | "versions"
  | "copy"
  | "extract"
  | "split"
  | null;

/**
 * Edit pages (board 4c): a grid of the book's pages to reorder, turn,
 * delete, insert, crop, redact and correct, then save as a new version
 * (the old file is kept) or as new books.
 */
export function PageEditor({
  book,
  hasOcr,
  canEdit,
  onClose,
  onSaved,
}: {
  book: BookDto;
  /** The book has saved OCR text (offer to write it into the PDF). */
  hasOcr: boolean;
  canEdit: boolean;
  onClose: () => void;
  /** The book's file changed: its id did too. */
  onSaved: (book: BookDto) => void;
}) {
  const [pdf, setPdf] = useState<PdfPages | null>(null);
  const [others, setOthers] = useState<Map<string, PdfPages>>(new Map());
  const [error, setError] = useState<string | null>(null);
  const [h, setH] = useState<History | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [anchor, setAnchor] = useState<string | null>(null);
  const [dialog, setDialog] = useState<Dialog>(null);
  const [detail, setDetail] = useState<{
    mode: DetailMode;
    uid: string;
    picture: string;
    aspect: number;
    points: [number, number];
    text: PageText | null;
  } | null>(null);
  const [saving, setSaving] = useState(false);
  const [dragging, setDragging] = useState<string[] | null>(null);
  const [dropAt, setDropAt] = useState<number | null>(null);
  const grid = useRef<HTMLDivElement>(null);

  // Open the book's PDF for thumbnails.
  useEffect(() => {
    let live = true;
    let doc: PdfPages | null = null;
    PdfPages.open(bookUrl(book.relPath))
      .then((d) => {
        doc = d;
        if (!live) return d.close();
        setPdf(d);
        setH({ past: [], present: M.initial(d.count), future: [] });
      })
      .catch((e: unknown) => live && setError(String(e)));
    return () => {
      live = false;
      doc?.close();
    };
  }, [book.relPath]);
  // Other PDFs added: closed when the editor closes (the map is replaced as
  // files are added, but the documents in it stay open).
  const othersRef = useRef(others);
  const closedRef = useRef(false);
  useEffect(() => {
    othersRef.current = others;
  }, [others]);
  useEffect(() => {
    closedRef.current = false;
    return () => {
      closedRef.current = true;
      othersRef.current.forEach((d) => d.close());
    };
  }, []);

  const s = h?.present;
  const pageCount = pdf?.count ?? 0;
  const changes = useMemo(() => (s ? M.summary(s, pageCount) : []), [s, pageCount]);

  const apply = useCallback((f: (s: M.EditState) => M.EditState) => {
    setH((cur) => {
      if (!cur) return cur;
      const next = f(cur.present);
      if (next === cur.present) return cur;
      return { past: [...cur.past, cur.present].slice(-100), present: next, future: [] };
    });
  }, []);
  const undo = () =>
    setH((c) =>
      c && c.past.length
        ? {
            past: c.past.slice(0, -1),
            present: c.past[c.past.length - 1]!,
            future: [c.present, ...c.future],
          }
        : c,
    );
  const redo = () =>
    setH((c) =>
      c && c.future.length
        ? { past: [...c.past, c.present], present: c.future[0]!, future: c.future.slice(1) }
        : c,
    );

  const sel = selected.filter((u) => s?.pages.some((p) => p.uid === u));
  const insertIndex = () => {
    if (!s) return 0;
    const idx = s.pages.map((p, i) => (sel.includes(p.uid) ? i : -1)).filter((i) => i >= 0);
    return idx.length ? Math.max(...idx) + 1 : s.pages.length;
  };

  const click = (uid: string, e: React.MouseEvent) => {
    if (!s) return;
    if (e.shiftKey && anchor) {
      const a = s.pages.findIndex((p) => p.uid === anchor);
      const b = s.pages.findIndex((p) => p.uid === uid);
      const [from, to] = a < b ? [a, b] : [b, a];
      setSelected(s.pages.slice(from, to + 1).map((p) => p.uid));
    } else if (e.metaKey || e.ctrlKey) {
      setSelected((list) => (list.includes(uid) ? list.filter((x) => x !== uid) : [...list, uid]));
      setAnchor(uid);
    } else {
      setSelected([uid]);
      setAnchor(uid);
    }
  };

  const onKey = (e: React.KeyboardEvent) => {
    const mod = e.metaKey || e.ctrlKey;
    if ((e.key === "Delete" || e.key === "Backspace") && sel.length) {
      e.preventDefault();
      apply((x) => M.remove(x, sel));
    } else if (mod && e.key.toLowerCase() === "a") {
      e.preventDefault();
      setSelected(s?.pages.map((p) => p.uid) ?? []);
    } else if (mod && e.key.toLowerCase() === "z") {
      e.preventDefault();
      if (e.shiftKey) redo();
      else undo();
    } else if (mod && e.key.toLowerCase() === "y") {
      e.preventDefault();
      redo();
    } else if (e.key === "Escape") {
      setSelected([]);
    }
  };

  // ----- inserting -----

  const addFilePages = (file: M.OtherFile, pages: number[]) => {
    const at = insertIndex();
    apply((x) => {
      const [i, next] = M.addFile(x, file);
      return M.insert(
        next,
        at,
        pages.map((page) => ({ kind: "file" as const, file: i, page })),
      );
    });
    if (file.url && !others.has(file.ref)) {
      void PdfPages.open(file.url).then((d) => {
        if (closedRef.current) return d.close();
        setOthers((m) => new Map(m).set(file.ref, d));
      });
    }
    setDialog(null);
  };
  const addPicture = (src: string) =>
    apply((x) => M.insert(x, insertIndex(), [{ kind: "picture", src }]));
  const addBlank = (size: [number, number]) =>
    apply((x) => M.insert(x, insertIndex(), [{ kind: "blank", width: size[0], height: size[1] }]));

  // ----- page tools -----

  const openDetail = async (mode: DetailMode, uid?: string) => {
    if (!s || !pdf) return;
    const target = s.pages.find((p) => p.uid === (uid ?? sel[0]));
    if (!target) {
      toast("Select a page first");
      return;
    }
    const src = target.source;
    if (mode !== "crop" && src.kind !== "page") {
      toast("Redact and correct work on the book’s own pages");
      return;
    }
    let doc: PdfPages | undefined;
    let page = 0;
    if (src.kind === "page") {
      doc = pdf;
      page = src.page;
    } else if (src.kind === "file") {
      doc = others.get(s.files[src.file]!.ref);
      page = src.page;
    }
    if (!doc) {
      toast("Pages from files outside the library cannot be cropped here");
      return;
    }
    // Crop shows the page turned as it will be; redact and correct as it is.
    const extra = mode === "crop" ? target.rotate : 0;
    const [picture, aspect, points, text] = await Promise.all([
      doc.picture(page, 1100, extra),
      doc.aspect(page, extra),
      doc.size(page, extra),
      mode === "crop" ? Promise.resolve(null) : doc.text(page).catch(() => null),
    ]);
    let pageText = text;
    if (mode !== "crop" && (!text || !text.items.length)) {
      // Scans: OCR words stand in for the text.
      const words = await unwrap(commands.pageWords(book.id, page)).catch(() => []);
      pageText = words.length
        ? { items: words.map((w) => ({ text: w.text, box: w.rect as M.Box })) }
        : null;
    }
    setSelected((cur) => (cur.includes(target.uid) ? cur : [target.uid]));
    setDetail({ mode, uid: target.uid, picture, aspect, points, text: pageText });
  };

  const findEverywhere = async (query: string) => {
    if (!pdf || !s) return;
    const t = toast.loading("Finding on every page…");
    let total = 0;
    let pages = 0;
    const found: [number, M.Box[]][] = [];
    for (let p = 1; p <= pdf.count; p++) {
      let text = await pdf.text(p).catch(() => null);
      if (!text?.items.length) {
        const words = await unwrap(commands.pageWords(book.id, p)).catch(() => []);
        text = { items: words.map((w) => ({ text: w.text, box: w.rect as M.Box })) };
      }
      const boxes = findBoxes(text, query);
      if (boxes.length) {
        found.push([p, boxes]);
        total += boxes.length;
        pages++;
      }
    }
    toast.dismiss(t);
    if (!total) {
      toast(`“${query}” was not found`);
      return;
    }
    apply((x) =>
      found.reduce(
        (acc, [p, boxes]) => M.setRedactions(acc, p, [...(acc.redactions[p] ?? []), ...boxes]),
        x,
      ),
    );
    toast.success(
      `${total} ${total === 1 ? "place" : "places"} on ${pages} ${pages === 1 ? "page" : "pages"} will be blacked out`,
    );
    setDetail(null);
  };

  // ----- saving -----

  const save = async () => {
    if (!s || !changes.length) return onClose();
    if (!s.pages.length) {
      toast.error("A PDF needs at least one page");
      return;
    }
    const redacting = Object.values(s.redactions).some((boxes) => boxes.length > 0);
    if (
      redacting &&
      !(await ask(
        "The hidden text will be gone for good: Libreri does not keep the unredacted file, and this book's earlier versions are deleted too, so the text is not left in Version history or in backups.",
        { title: "Save the redactions?", kind: "warning", okLabel: "Redact and save" },
      ))
    )
      return;
    setSaving(true);
    try {
      const r = await unwrap(commands.editPages(book.id, M.toPlan(s)));
      if (r.flattened.length)
        toast.warning(
          `Page${r.flattened.length === 1 ? "" : "s"} ${r.flattened.join(", ")} became ${r.flattened.length === 1 ? "a picture" : "pictures"}`,
          {
            description:
              "Something under a redaction could not be removed exactly, so the whole page was turned into a picture. Its text can be made searchable again with OCR.",
            duration: 12000,
          },
        );
      for (const w of r.warnings) toast(w);
      toast.success(
        redacting
          ? "Saved. The redacted text is gone, and no earlier versions are kept."
          : "Saved. The earlier version is kept in Version history.",
      );
      onSaved(r.book);
    } catch (e) {
      toast.error("The changes could not be saved", { description: String(e) });
    } finally {
      setSaving(false);
    }
  };

  const saveNewBooks = async (req: NewBooksRequest) => {
    if (!s) return;
    const parts =
      req.kind === "all"
        ? [s.pages.map((p) => p.uid)]
        : req.kind === "selected"
          ? [s.pages.filter((p) => sel.includes(p.uid)).map((p) => p.uid)]
          : req.kind === "every"
            ? M.splitEvery(s, req.size)
            : M.splitAt(s, sel);
    setSaving(true);
    try {
      for (const [i, part] of parts.entries()) {
        const name = parts.length > 1 ? `${req.name} (part ${i + 1})` : req.name;
        await unwrap(commands.savePagesAsBook(book.id, M.toPlan(s, part), name));
      }
      toast.success(
        parts.length > 1
          ? `${parts.length} new books are being added`
          : "The new book is being added",
      );
      setDialog(null);
    } catch (e) {
      toast.error("The new book could not be made", { description: String(e) });
    } finally {
      setSaving(false);
    }
  };

  const discard = async () => {
    if (changes.length) {
      const ok = await ask("Your changes to the pages are thrown away.", {
        title: "Discard the changes?",
        okLabel: "Discard",
        kind: "warning",
      });
      if (!ok) return;
    }
    onClose();
  };

  // ----- drag and drop -----

  const dropIndex = (e: React.DragEvent): number => {
    const cards = Array.from(grid.current?.querySelectorAll<HTMLElement>("[data-card]") ?? []);
    for (const [i, c] of cards.entries()) {
      const r = c.getBoundingClientRect();
      if (e.clientY < r.top) return i;
      if (e.clientY <= r.bottom && e.clientX < r.left + r.width / 2) return i;
    }
    return cards.length;
  };

  if (error)
    return (
      <div className="flex flex-1 items-center justify-center p-6 text-center text-destructive">
        The pages could not be shown: {error}
      </div>
    );
  if (!s || !pdf)
    return (
      <div className="flex-1">
        <ThumbsSkeleton count={12} />
      </div>
    );

  const detailPage = detail ? s.pages.find((p) => p.uid === detail.uid) : undefined;
  const detailBookPage = detailPage?.source.kind === "page" ? detailPage.source.page : 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {/* Tools */}
      <div
        className="flex h-10 shrink-0 items-center gap-0.5 border-b bg-muted/30 px-2"
        role="toolbar"
        aria-label="Edit pages"
      >
        <Button
          variant="ghost"
          size="icon"
          title="Undo (Ctrl+Z)"
          aria-label="Undo"
          disabled={!h?.past.length}
          onClick={undo}
        >
          <Undo2 />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title="Redo"
          aria-label="Redo"
          disabled={!h?.future.length}
          onClick={redo}
        >
          <Redo2 />
        </Button>
        <span className="mx-1 h-5 w-px bg-border" />
        <Button
          variant="ghost"
          size="icon"
          title="Turn left"
          aria-label="Turn left"
          disabled={!sel.length}
          onClick={() => apply((x) => M.rotate(x, sel, -90))}
        >
          <RotateCcw />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title="Turn right"
          aria-label="Turn right"
          disabled={!sel.length}
          onClick={() => apply((x) => M.rotate(x, sel, 90))}
        >
          <RotateCw />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title="Delete pages (Delete)"
          aria-label="Delete pages"
          disabled={!sel.length}
          onClick={() => apply((x) => M.remove(x, sel))}
        >
          <Trash2 />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          title="Duplicate pages"
          aria-label="Duplicate pages"
          disabled={!sel.length}
          onClick={() => apply((x) => M.duplicate(x, sel))}
        >
          <Copy />
        </Button>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger asChild>
            <Button variant="ghost" size="sm">
              <FilePlus2 /> Insert <ChevronDown className="size-3.5" />
            </Button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Portal>
            <DropdownMenu.Content className={menuContent} sideOffset={4}>
              <div className={menuLabel}>After the selected page</div>
              {M.PAPER.map((p) => (
                <DropdownMenu.Item
                  key={p.id}
                  className={menuItem}
                  onSelect={() => addBlank(p.size)}
                >
                  <File /> Blank page ({p.label})
                </DropdownMenu.Item>
              ))}
              <div className={menuSeparator} />
              <DropdownMenu.Item className={menuItem} onSelect={() => setDialog("file")}>
                <FilePlus2 /> Pages from a PDF file…
              </DropdownMenu.Item>
              <DropdownMenu.Item className={menuItem} onSelect={() => setDialog("book")}>
                <BookPlus /> Pages from a book in the library…
              </DropdownMenu.Item>
              <div className={menuSeparator} />
              <DropdownMenu.Item
                className={menuItem}
                onSelect={() => void pickPicture().then((p) => p && addPicture(p.src))}
              >
                <ImagePlus /> Picture or scan…
              </DropdownMenu.Item>
              <DropdownMenu.Item className={menuItem} onSelect={() => setDialog("camera")}>
                <Camera /> Photo with the camera…
              </DropdownMenu.Item>
              <DropdownMenu.Item className={menuItem} onSelect={() => setDialog("phone")}>
                <Smartphone /> Photos from your phone…
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
        <span className="mx-1 h-5 w-px bg-border" />
        <Button
          variant="ghost"
          size="sm"
          disabled={sel.length === 0}
          onClick={() => void openDetail("crop")}
        >
          <Crop /> Crop
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={sel.length === 0}
          onClick={() => void openDetail("redact")}
        >
          <EyeOff /> Redact
        </Button>
        <Button
          variant="ghost"
          size="sm"
          disabled={sel.length === 0}
          onClick={() => void openDetail("correct")}
        >
          <SquarePen /> Correct text
        </Button>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger asChild>
            <Button variant="ghost" size="icon" aria-label="More">
              <MoreHorizontal />
            </Button>
          </DropdownMenu.Trigger>
          <DropdownMenu.Portal>
            <DropdownMenu.Content className={menuContent} sideOffset={4}>
              <DropdownMenu.Item
                className={menuItem}
                disabled={!sel.length}
                onSelect={() => setDialog("extract")}
              >
                <BookPlus /> Extract selected pages as a new book…
              </DropdownMenu.Item>
              <DropdownMenu.Item className={menuItem} onSelect={() => setDialog("split")}>
                <Copy /> Split into several books…
              </DropdownMenu.Item>
              <div className={menuSeparator} />
              <DropdownMenu.Item className={menuItem} onSelect={() => setDialog("compress")}>
                <Minimize2 /> Make the PDF smaller…
              </DropdownMenu.Item>
              <DropdownMenu.CheckboxItem
                className={menuItem}
                disabled={!hasOcr}
                checked={s.embedOcr}
                onCheckedChange={(v) => apply((x) => ({ ...x, embedOcr: v }))}
              >
                <ScanText /> Write the OCR text into the PDF
              </DropdownMenu.CheckboxItem>
              <div className={menuSeparator} />
              <DropdownMenu.Item className={menuItem} onSelect={() => setDialog("versions")}>
                <History /> Version history…
              </DropdownMenu.Item>
            </DropdownMenu.Content>
          </DropdownMenu.Portal>
        </DropdownMenu.Root>
        <span className="flex-1" />
        <span className="px-2 text-[12px] text-muted-foreground tabular-nums">
          {sel.length ? `${sel.length} of ${s.pages.length} selected` : `${s.pages.length} pages`}
        </span>
      </div>

      {detail && detailPage ? (
        <PageDetail
          key={detail.uid + detail.mode}
          mode={detail.mode}
          picture={detail.picture}
          aspect={detail.aspect}
          pagePoints={detail.points}
          text={detail.text}
          title={cardLabel(detailPage, s)}
          book={detailBookPage}
          initialBoxes={
            detail.mode === "crop"
              ? detailPage.crop
                ? [detailPage.crop]
                : []
              : (s.redactions[detailBookPage] ?? [])
          }
          initialCorrections={s.corrections.filter((c) => c.page === detailBookPage)}
          selectedCount={sel.length}
          onFindEverywhere={detail.mode === "redact" ? (q) => void findEverywhere(q) : undefined}
          onCancel={() => setDetail(null)}
          onDone={({ boxes, corrections, allSelected }) => {
            if (detail.mode === "crop")
              apply((x) => M.setCrop(x, allSelected ? sel : [detail.uid], boxes[0] ?? null));
            else if (detail.mode === "redact")
              apply((x) => M.setRedactions(x, detailBookPage, boxes));
            else apply((x) => M.setCorrections(x, detailBookPage, corrections as Correction[]));
            setDetail(null);
          }}
        />
      ) : (
        <div className="flex min-h-0 flex-1">
          <div
            ref={grid}
            tabIndex={0}
            onKeyDown={onKey}
            onClick={(e) => e.target === e.currentTarget && setSelected([])}
            onDragOver={(e) => {
              if (!dragging) return;
              e.preventDefault();
              setDropAt(dropIndex(e));
            }}
            onDragLeave={(e) => e.target === e.currentTarget && setDropAt(null)}
            onDrop={(e) => {
              e.preventDefault();
              if (dragging && dropAt !== null) apply((x) => M.move(x, dragging, dropAt));
              setDragging(null);
              setDropAt(null);
            }}
            className="flex min-w-0 flex-1 flex-wrap content-start gap-x-4 gap-y-5 overflow-auto p-5 outline-none"
            aria-label="Pages"
            role="listbox"
            aria-multiselectable
          >
            {s.pages.map((p, i) => (
              <PageCard
                key={p.uid}
                page={p}
                index={i}
                state={s}
                pdf={pdf}
                others={others}
                selected={sel.includes(p.uid)}
                dropBefore={dropAt === i}
                dropAfter={dropAt === s.pages.length && i === s.pages.length - 1}
                onClick={(e) => click(p.uid, e)}
                onOpen={() => void openDetail(p.source.kind === "page" ? "redact" : "crop", p.uid)}
                onDragStart={() => {
                  const list = sel.includes(p.uid) ? sel : [p.uid];
                  if (!sel.includes(p.uid)) setSelected([p.uid]);
                  setDragging(list);
                }}
                onDragEnd={() => {
                  setDragging(null);
                  setDropAt(null);
                }}
              />
            ))}
          </div>
          <ResizablePanel
            id="edit.changes"
            initial={240}
            min={200}
            max={480}
            side="right"
            label="changes"
          >
            <aside className="flex min-w-0 flex-1 flex-col gap-2 overflow-auto border-l p-4 text-[13px]">
              <h3 className="font-semibold">Changes</h3>
              {changes.length ? (
                <ul className="flex list-disc flex-col gap-1 pl-4">
                  {changes.map((c) => (
                    <li key={c}>{c[0]!.toUpperCase() + c.slice(1)}</li>
                  ))}
                </ul>
              ) : (
                <p className="text-muted-foreground">
                  None yet. Drag pages to reorder them; select pages to turn, delete, crop, redact
                  or correct them.
                </p>
              )}
              <p className="mt-2 text-[12px] text-muted-foreground">
                Saving keeps the current file as an earlier version. Notes, highlights and markup
                move with their pages.
              </p>
            </aside>
          </ResizablePanel>
        </div>
      )}

      {/* Save */}
      <div className="flex h-12 shrink-0 items-center gap-2 border-t px-3">
        <Button variant="ghost" onClick={() => void discard()} disabled={saving}>
          {changes.length ? "Discard changes" : "Close"}
        </Button>
        <span className="flex-1" />
        <Button
          variant="outline"
          disabled={saving || !changes.length}
          onClick={() => setDialog("copy")}
        >
          Save as a new book…
        </Button>
        {canEdit && (
          <Button disabled={saving || !changes.length || !!detail} onClick={() => void save()}>
            {saving && <Loader2 className="animate-spin" />} Save
          </Button>
        )}
      </div>

      <PdfPagesDialog
        key={`file-${dialog === "file"}`}
        open={dialog === "file"}
        source="file"
        onClose={() => setDialog(null)}
        onPick={addFilePages}
      />
      <PdfPagesDialog
        key={`book-${dialog === "book"}`}
        open={dialog === "book"}
        source="book"
        onClose={() => setDialog(null)}
        onPick={addFilePages}
      />
      <CameraPagesDialog
        open={dialog === "camera"}
        onClose={() => setDialog(null)}
        onPhoto={addPicture}
      />
      <PhonePagesDialog
        open={dialog === "phone"}
        onClose={() => setDialog(null)}
        onPhoto={addPicture}
      />
      <CompressDialog
        key={`compress-${dialog === "compress"}`}
        open={dialog === "compress"}
        value={s.compress}
        onClose={() => setDialog(null)}
        onChoose={(q) => {
          apply((x) => ({ ...x, compress: q }));
          setDialog(null);
        }}
      />
      {(dialog === "copy" || dialog === "extract" || dialog === "split") && (
        <NewBooksDialog
          open
          mode={dialog}
          title={book.metadata.title ?? ""}
          selected={sel.length}
          busy={saving}
          onClose={() => setDialog(null)}
          onSave={(r) => void saveNewBooks(r)}
        />
      )}
      <VersionsDialog
        open={dialog === "versions"}
        bookId={book.id}
        title={book.metadata.title ?? ""}
        canEdit={canEdit}
        onClose={() => setDialog(null)}
        onRestored={(b) => {
          setDialog(null);
          onSaved(b);
        }}
      />
    </div>
  );
}

function cardLabel(p: M.EditPage, s: M.EditState): string {
  switch (p.source.kind) {
    case "page":
      return `Page ${p.source.page}`;
    case "file":
      return `${s.files[p.source.file]?.name ?? "PDF"}, p. ${p.source.page}`;
    case "blank":
      return "Blank page";
    case "picture":
      return "Picture";
  }
}

function PageCard({
  page,
  index,
  state,
  pdf,
  others,
  selected,
  dropBefore,
  dropAfter,
  onClick,
  onOpen,
  onDragStart,
  onDragEnd,
}: {
  page: M.EditPage;
  index: number;
  state: M.EditState;
  pdf: PdfPages;
  others: Map<string, PdfPages>;
  selected: boolean;
  dropBefore: boolean;
  dropAfter: boolean;
  onClick: (e: React.MouseEvent) => void;
  onOpen: () => void;
  onDragStart: () => void;
  onDragEnd: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [seen, setSeen] = useState(false);
  const [thumb, setThumb] = useState<{ src: string; aspect: number } | null>(null);
  const src = page.source;
  const doc =
    src.kind === "page"
      ? pdf
      : src.kind === "file"
        ? others.get(state.files[src.file]?.ref ?? "")
        : undefined;
  const pageNo = src.kind === "page" || src.kind === "file" ? src.page : 0;

  // Draw the thumbnail once the card scrolls into view.
  useEffect(() => {
    const el = ref.current;
    if (!el || seen) return;
    const io = new IntersectionObserver(
      (list) => list.some((e) => e.isIntersecting) && setSeen(true),
      { rootMargin: "400px" },
    );
    io.observe(el);
    return () => io.disconnect();
  }, [seen]);
  useEffect(() => {
    if (!seen || !doc || !pageNo) return;
    let live = true;
    void Promise.all([doc.picture(pageNo, THUMB), doc.aspect(pageNo)])
      .then(([u, a]) => live && setThumb({ src: u, aspect: a }))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [seen, doc, pageNo]);

  const aspect = src.kind === "blank" ? src.height / src.width : (thumb?.aspect ?? 1.414);
  const turned = page.rotate % 180 !== 0;
  // The box the card shows, after turning.
  const shownAspect = turned ? 1 / aspect : aspect;
  const W = THUMB;
  const H = Math.min(THUMB * 1.6, W * shownAspect);
  const redactions = src.kind === "page" ? (state.redactions[src.page]?.length ?? 0) : 0;
  const fixes =
    src.kind === "page" ? state.corrections.filter((c) => c.page === src.page).length : 0;

  return (
    <div
      ref={ref}
      data-card
      role="option"
      aria-selected={selected}
      aria-label={cardLabel(page, state)}
      draggable
      onDragStart={(e) => {
        e.dataTransfer.effectAllowed = "move";
        e.dataTransfer.setData("text/plain", page.uid);
        onDragStart();
      }}
      onDragEnd={onDragEnd}
      onClick={onClick}
      onDoubleClick={onOpen}
      className="relative flex w-[150px] flex-col items-center gap-1.5"
    >
      {dropBefore && <span className="absolute top-0 -left-2.5 h-full w-1 rounded bg-primary" />}
      {dropAfter && <span className="absolute top-0 -right-2.5 h-full w-1 rounded bg-primary" />}
      <div
        className={cn(
          "relative flex items-center justify-center overflow-hidden rounded-sm bg-white shadow-sm ring-1 ring-border",
          selected && "ring-2 ring-primary ring-offset-2 ring-offset-background",
        )}
        style={{ width: W, height: H }}
      >
        <div
          className="relative shrink-0"
          style={{
            width: turned ? H : W,
            height: turned ? W : H,
            transform: `rotate(${page.rotate}deg)`,
          }}
        >
          {src.kind === "picture" ? (
            <img src={src.src} alt="" className="size-full object-contain" draggable={false} />
          ) : thumb ? (
            <img src={thumb.src} alt="" className="size-full" draggable={false} />
          ) : src.kind === "blank" ? null : (
            <div className="flex size-full items-center justify-center p-2 text-center text-[11px] text-muted-foreground">
              {src.kind === "file" && !doc ? cardLabel(page, state) : ""}
            </div>
          )}
          {src.kind === "page" &&
            state.redactions[src.page]?.map((b, i) => (
              <span
                key={i}
                className="absolute bg-black"
                style={{
                  left: `${b[0] * 100}%`,
                  top: `${b[1] * 100}%`,
                  width: `${b[2] * 100}%`,
                  height: `${b[3] * 100}%`,
                }}
              />
            ))}
        </div>
        {page.crop && (
          <span
            className="pointer-events-none absolute outline outline-1 outline-primary"
            style={{
              left: `${page.crop[0] * 100}%`,
              top: `${page.crop[1] * 100}%`,
              width: `${page.crop[2] * 100}%`,
              height: `${page.crop[3] * 100}%`,
              boxShadow: "0 0 0 999px rgba(0,0,0,0.35)",
            }}
          />
        )}
      </div>
      <span className="flex items-center gap-1 text-[11.5px] text-muted-foreground tabular-nums">
        {index + 1}
        {src.kind !== "page" || src.page !== index + 1 ? (
          <span className="truncate">· {cardLabel(page, state)}</span>
        ) : null}
        {redactions > 0 && <EyeOff className="size-3" aria-label="Redacted" />}
        {fixes > 0 && <SquarePen className="size-3" aria-label="Corrected" />}
      </span>
    </div>
  );
}
