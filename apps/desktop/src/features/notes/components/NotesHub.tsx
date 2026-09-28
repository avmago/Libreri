import { useDeferredValue, useMemo, useRef, useState } from "react";
import {
  BookOpen,
  ClipboardCopy,
  FilePlus2,
  NotebookText,
  PenLine,
  Search,
  SearchX,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { NativeSelect } from "@/components/ui/input";
import {
  CanvasEditor,
  useCanvases,
  useCreateCanvas,
  useDeleteCanvas,
  type Paper,
} from "@/features/canvas";
import { CaptureViewer, CopyViewer, parseBookLink, useAppDark } from "@/features/reader";
import { commands, type HighlightColor, type NoteDto } from "@/lib/ipc";
import { useShortcut } from "@/lib/shortcuts";
import { useTabs } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import { HIGHLIGHT_COLORS } from "@/readers";
import { useAllNotes, useCreateNote, useNotebookList, useRemoveNote, useUpdateNote } from "../api";
import { byBook, filterNotes, notesToMarkdown, relativeDate, type NoteKind } from "../model";
import { NoteCard } from "./NoteCard";
import { NoteEditor } from "./NoteEditor";

const KINDS: { id: NoteKind; label: string }[] = [
  { id: "all", label: "All" },
  { id: "highlights", label: "Highlights" },
  { id: "comments", label: "With comments" },
  { id: "voice", label: "Voice notes" },
  { id: "captures", label: "Paper notes" },
  { id: "links", label: "Links" },
  { id: "bookmarks", label: "Bookmarks" },
];

const SWATCH: Record<HighlightColor, string> = {
  yellow: "#facc15",
  green: "#4ade80",
  blue: "#60a5fa",
  pink: "#f472b6",
};

/** Board 12: every highlight, comment, bookmark and notebook in one place. */
export function NotesHub() {
  const [tab, setTab] = useState<"marks" | "notebooks" | "canvases">("marks");
  const [search, setSearch] = useState("");
  const deferred = useDeferredValue(search);
  const searchRef = useRef<HTMLInputElement>(null);
  useShortcut("notes.search", () => searchRef.current?.focus());

  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex flex-col gap-3 px-6 pt-4 pb-3">
        <div className="flex items-center gap-3">
          <h1 className="text-xl font-semibold tracking-tight">Notes</h1>
          <div className="ml-2 flex rounded-md border p-0.5" role="tablist" aria-label="Notes">
            {(
              [
                ["marks", "Highlights and comments"],
                ["notebooks", "Notebooks"],
                ["canvases", "Canvases"],
              ] as const
            ).map(([id, label]) => (
              <button
                key={id}
                type="button"
                role="tab"
                aria-selected={tab === id}
                onClick={() => setTab(id)}
                className={cn(
                  "h-7 rounded px-3 text-[13px]",
                  tab === id
                    ? "bg-muted font-medium"
                    : "text-muted-foreground hover:text-foreground",
                )}
              >
                {label}
              </button>
            ))}
          </div>
          <div className="flex-1" />
          <div className="relative w-72 min-w-40 shrink">
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden
            />
            <input
              ref={searchRef}
              type="search"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setSearch("");
                  e.currentTarget.blur();
                }
              }}
              placeholder={
                tab === "marks" ? "Search quotes, comments, books…" : "Search notebooks…"
              }
              aria-label="Search notes"
              className="h-8 w-full rounded-md border border-input bg-background pr-14 pl-8 text-[13px] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/20"
            />
            <Kbd
              action="notes.search"
              className="pointer-events-none absolute top-1/2 right-2 -translate-y-1/2"
            />
          </div>
        </div>
      </header>
      {tab === "marks" ? (
        <Marks search={deferred} />
      ) : tab === "canvases" ? (
        <Canvases search={deferred} />
      ) : (
        <Notebooks search={deferred} />
      )}
    </div>
  );
}

function openAt(n: NoteDto) {
  useTabs.getState().open({
    bookId: n.annotation.bookId,
    title: n.bookTitle,
    fileType: n.fileType,
    jumpTo: n.annotation.id,
  });
}

function Marks({ search }: { search: string }) {
  const { data: notes = [], isPending } = useAllNotes();
  const update = useUpdateNote();
  const remove = useRemoveNote();
  const [kind, setKind] = useState<NoteKind>("all");
  const [colors, setColors] = useState<HighlightColor[]>([]);
  const [bookId, setBookId] = useState<string | null>(null);
  const [grouped, setGrouped] = useState(true);
  const [viewing, setViewing] = useState<{ path: string; title: string } | null>(null);
  const [copyView, setCopyView] = useState<{ copy: string; url: string; title: string } | null>(
    null,
  );

  const books = useMemo(
    () => byBook(notes).map((g) => ({ id: g.bookId, title: g.title })),
    [notes],
  );
  const shown = useMemo(
    () => filterNotes(notes, { search, kind, colors, bookId }),
    [notes, search, kind, colors, bookId],
  );

  const copy = () => {
    void navigator.clipboard
      .writeText(notesToMarkdown(shown))
      .then(() =>
        toast.success(
          `Copied ${shown.length} ${shown.length === 1 ? "note" : "notes"} as Markdown`,
        ),
      );
  };
  useShortcut("notes.copyMarkdown", copy);

  const card = (n: NoteDto, showBook: boolean) => (
    <NoteCard
      key={n.annotation.id}
      note={n}
      showBook={showBook}
      onOpen={() => openAt(n)}
      onChange={(a) => update.mutate(a)}
      onOpenCapture={(path, title) => setViewing({ path, title })}
      onOpenCopy={(copy, url, title) => setCopyView({ copy, url, title })}
      onDelete={() =>
        remove.mutate(n.annotation, {
          onSuccess: () =>
            toast("Note deleted", {
              action: { label: "Undo", onClick: () => update.mutate(n.annotation) },
            }),
        })
      }
    />
  );

  return (
    <>
      <CaptureViewer
        path={viewing?.path ?? null}
        title={viewing?.title ?? ""}
        onClose={() => setViewing(null)}
      />
      <CopyViewer
        copy={copyView?.copy ?? null}
        url={copyView?.url ?? ""}
        title={copyView?.title ?? ""}
        onClose={() => setCopyView(null)}
      />
      <div className="flex flex-wrap items-center gap-2 border-b px-6 pb-3">
        <div className="flex rounded-md border p-0.5" role="group" aria-label="Show">
          {KINDS.map((k) => (
            <button
              key={k.id}
              type="button"
              aria-pressed={kind === k.id}
              onClick={() => setKind(k.id)}
              className={cn(
                "h-7 rounded px-2.5 text-[12.5px]",
                kind === k.id
                  ? "bg-muted font-medium"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {k.label}
            </button>
          ))}
        </div>
        <div className="flex items-center gap-1.5 px-2" role="group" aria-label="Colours">
          {HIGHLIGHT_COLORS.map((c) => (
            <button
              key={c}
              type="button"
              aria-pressed={colors.includes(c)}
              aria-label={`Only ${c}`}
              title={`Only ${c}`}
              onClick={() =>
                setColors(colors.includes(c) ? colors.filter((x) => x !== c) : [...colors, c])
              }
              className={cn(
                "size-5 rounded-full border border-black/10",
                colors.includes(c) && "ring-2 ring-ring ring-offset-2 ring-offset-background",
              )}
              style={{ background: SWATCH[c] }}
            />
          ))}
        </div>
        <NativeSelect
          aria-label="Book"
          value={bookId ?? ""}
          onChange={(e) => setBookId(e.target.value || null)}
          className="w-56"
        >
          <option value="">All books</option>
          {books.map((b) => (
            <option key={b.id} value={b.id}>
              {b.title}
            </option>
          ))}
        </NativeSelect>
        <label className="flex items-center gap-2 px-2 text-[12.5px] text-muted-foreground">
          <input
            type="checkbox"
            checked={grouped}
            onChange={(e) => setGrouped(e.target.checked)}
            className="size-4 accent-primary"
          />
          Group by book
        </label>
        <div className="flex-1" />
        <span className="text-[12px] text-muted-foreground">
          {shown.length} of {notes.length}
        </span>
        <Button variant="outline" size="sm" onClick={copy} disabled={!shown.length}>
          <ClipboardCopy /> Copy as Markdown
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto bg-sidebar/40">
        {isPending ? null : notes.length === 0 ? (
          <Empty
            title="No highlights yet"
            text="Select text while reading to highlight it or add a comment. Everything you mark shows up here, across all your books."
          />
        ) : shown.length === 0 ? (
          <Empty icon={SearchX} title="Nothing matches" text="Try other words or fewer filters." />
        ) : grouped ? (
          <div className="mx-auto flex max-w-4xl flex-col gap-6 px-6 py-5">
            {byBook(shown).map((g) => (
              <section key={g.bookId} aria-label={g.title} className="flex flex-col gap-2.5">
                <h2 className="flex items-baseline gap-2 text-[15px] font-semibold">
                  {g.title}
                  <span className="text-[12px] font-normal text-muted-foreground">
                    {g.notes.length}
                  </span>
                </h2>
                {[...g.notes]
                  .sort((a, b) => (a.annotation.position ?? 0) - (b.annotation.position ?? 0))
                  .map((n) => card(n, false))}
              </section>
            ))}
          </div>
        ) : (
          <div className="mx-auto flex max-w-4xl flex-col gap-2.5 px-6 py-5">
            {shown.map((n) => card(n, true))}
          </div>
        )}
      </div>
    </>
  );
}

function openLink(href: string) {
  const link = parseBookLink(href);
  if (link) {
    useTabs.getState().open({
      bookId: link.bookId,
      title: "Book",
      fileType: "pdf",
      jumpTo: link.page ? `page:${link.page}` : link.annotation,
    });
  } else if (/^(https?:|mailto:)/i.test(href)) {
    void commands.openExternalUrl(href);
  }
}

/** Every canvas of the profile, drawn in place. */
function Canvases({ search }: { search: string }) {
  const { data: list = [], isPending } = useCanvases(null);
  const create = useCreateCanvas();
  const remove = useDeleteCanvas();
  const dark = useAppDark();
  const [selected, setSelected] = useState<string | null>(null);
  const q = search.trim().toLowerCase();
  const shown = list.filter((c) => !q || c.title.toLowerCase().includes(q));
  const current = list.find((c) => c.relPath === selected) ?? shown[0] ?? null;
  const newCanvas = () =>
    create.mutate(
      { title: "Canvas", book: null, paper: "plain" },
      {
        onSuccess: (rel) => setSelected(rel),
        onError: (e) => toast.error("Could not start a canvas", { description: e.message }),
      },
    );

  return (
    <div className="flex min-h-0 flex-1 border-t">
      <div className="flex w-72 shrink-0 flex-col border-r bg-sidebar">
        <div className="flex items-center justify-between px-3 py-2">
          <span className="text-[11px] font-semibold tracking-wide text-muted-foreground">
            {list.length} {list.length === 1 ? "CANVAS" : "CANVASES"}
          </span>
          <Button variant="ghost" size="sm" onClick={newCanvas}>
            <PenLine /> New canvas
          </Button>
        </div>
        <ul className="min-h-0 flex-1 overflow-y-auto px-2 pb-3" aria-label="Canvases">
          {shown.map((c) => (
            <li key={c.relPath} className="group relative">
              <button
                type="button"
                onClick={() => setSelected(c.relPath)}
                aria-current={current?.relPath === c.relPath ? "true" : undefined}
                className={cn(
                  "flex w-full flex-col gap-1 rounded-md px-2.5 py-2 text-left",
                  current?.relPath === c.relPath ? "bg-muted" : "hover:bg-muted/60",
                )}
              >
                <span className="flex items-center gap-2 font-medium">
                  <PenLine className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
                  <span className="truncate">{c.title}</span>
                </span>
                <span className="text-[11px] text-muted-foreground">
                  {relativeDate(c.modified ?? 0)} · {c.elements}{" "}
                  {c.elements === 1 ? "drawing" : "drawings"}
                  {c.bookId ? "" : " · not linked to a book"}
                </span>
              </button>
              <button
                type="button"
                aria-label={`Delete ${c.title}`}
                className="absolute top-2 right-2 hidden rounded p-1 text-muted-foreground group-hover:block hover:bg-background hover:text-destructive focus-visible:block"
                onClick={() =>
                  remove.mutate(c.relPath, {
                    onSuccess: () => toast(`“${c.title}” was moved to the trash`),
                    onError: (e) => toast.error(e.message),
                  })
                }
              >
                <Trash2 className="size-3.5" />
              </button>
            </li>
          ))}
          {!isPending && list.length === 0 && (
            <p className="px-2.5 py-4 text-[12.5px] text-muted-foreground">
              Canvases are for writing and drawing by hand. Start one here, or from a book with the
              pen button in the reader. They are Excalidraw files in your Notes folder.
            </p>
          )}
        </ul>
      </div>
      {current ? (
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-11 shrink-0 items-center gap-2 border-b px-4">
            <span className="truncate font-medium">{current.title}</span>
            <span className="truncate font-mono text-[10.5px] text-muted-foreground">
              {current.relPath}
            </span>
            <div className="flex-1" />
            {current.bookId && (
              <Button
                variant="outline"
                size="sm"
                onClick={() =>
                  useTabs
                    .getState()
                    .open({ bookId: current.bookId!, title: current.title, fileType: "pdf" })
                }
              >
                <BookOpen /> Open book
              </Button>
            )}
          </div>
          <CanvasEditor
            key={current.relPath}
            relPath={current.relPath}
            paper={current.paper as Paper}
            dark={dark}
            onLink={openLink}
          />
        </div>
      ) : (
        <Empty title="No canvas selected" text="Choose a canvas on the left, or start one." />
      )}
    </div>
  );
}

function Notebooks({ search }: { search: string }) {
  const { data: list = [], isPending } = useNotebookList();
  const create = useCreateNote();
  const [selected, setSelected] = useState<string | null>(null);
  const q = search.trim().toLowerCase();
  const shown = list.filter(
    (n) => !q || n.title.toLowerCase().includes(q) || n.excerpt.toLowerCase().includes(q),
  );
  const current = list.find((n) => n.relPath === selected) ?? shown[0] ?? null;

  const newNote = () =>
    create.mutate("Untitled note", {
      onSuccess: (rel) => setSelected(rel),
      onError: (e) => toast.error("Could not create the note", { description: String(e) }),
    });
  useShortcut("notes.new", newNote);

  const onLink = openLink;

  return (
    <div className="flex min-h-0 flex-1 border-t">
      <div className="flex w-80 shrink-0 flex-col border-r bg-sidebar">
        <div className="flex items-center justify-between px-3 py-2">
          <span className="text-[11px] font-semibold tracking-wide text-muted-foreground">
            {list.length} {list.length === 1 ? "NOTEBOOK" : "NOTEBOOKS"}
          </span>
          <Button variant="ghost" size="sm" onClick={newNote} title="New note">
            <FilePlus2 /> New note
          </Button>
        </div>
        <ul className="min-h-0 flex-1 overflow-y-auto px-2 pb-3" aria-label="Notebooks">
          {shown.map((n) => (
            <li key={n.relPath}>
              <button
                type="button"
                onClick={() => setSelected(n.relPath)}
                aria-current={current?.relPath === n.relPath ? "true" : undefined}
                className={cn(
                  "flex w-full flex-col gap-1 rounded-md px-2.5 py-2 text-left",
                  current?.relPath === n.relPath ? "bg-muted" : "hover:bg-muted/60",
                )}
              >
                <span className="flex items-center gap-2 font-medium">
                  <NotebookText className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
                  <span className="truncate">{n.title}</span>
                </span>
                <span className="line-clamp-2 text-[12px] text-muted-foreground">
                  {n.excerpt || "Empty"}
                </span>
                <span className="text-[11px] text-muted-foreground">
                  {relativeDate(n.modified ?? 0)} · {n.words} {n.words === 1 ? "word" : "words"}
                  {n.bookId ? "" : " · not linked to a book"}
                </span>
              </button>
            </li>
          ))}
          {!isPending && list.length === 0 && (
            <p className="px-2.5 py-4 text-[12.5px] text-muted-foreground">
              Open a book and its notebook panel, or start a note here. Notes are Markdown files in
              your Notes folder.
            </p>
          )}
        </ul>
      </div>
      {current ? (
        <NoteEditor
          key={current.relPath}
          relPath={current.relPath}
          title={current.title}
          bookTitle={current.bookId ? current.title : null}
          onOpenBook={
            current.bookId
              ? () =>
                  useTabs.getState().open({
                    bookId: current.bookId!,
                    title: current.title,
                    fileType: "pdf",
                  })
              : undefined
          }
          onLink={onLink}
        />
      ) : (
        <Empty title="No notebook selected" text="Choose a notebook on the left." />
      )}
    </div>
  );
}

function Empty({
  title,
  text,
  icon: Icon = NotebookText,
}: {
  title: string;
  text: string;
  icon?: typeof NotebookText;
}) {
  return (
    <div className="flex flex-1 flex-col items-center justify-center gap-3 px-6 py-20 text-center">
      <div className="flex size-12 items-center justify-center rounded-xl bg-muted">
        <Icon className="size-6 text-muted-foreground" aria-hidden />
      </div>
      <h2 className="text-base font-semibold">{title}</h2>
      <p className="max-w-sm text-muted-foreground">{text}</p>
    </div>
  );
}
