import { useDeferredValue, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import {
  BookOpen,
  ChevronDown,
  FileSearch,
  Highlighter,
  Loader2,
  ScanText,
  Search,
  SearchX,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { Kbd } from "@/components/ui/kbd";
import { BookCover, toView, useBooks, useLibraryView, type BookView } from "@/features/library";
import { filterNotes, useAllNotes } from "@/features/notes";
import { usePermissions } from "@/features/profiles";
import type { BookDto, Hit, NoteDto, SnippetPart, TextMatchDto } from "@/lib/ipc";
import { useTabs } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import { useBookHits, useBooksWithoutText, useIndexStatus, useTextSearch } from "../api";
import { hitPlace, markWords } from "../model";
import { useIndexing, useOcrDialog } from "../store";

type Scope = "all" | "text" | "details" | "notes";

const SCOPES: { id: Scope; label: string }[] = [
  { id: "all", label: "Everything" },
  { id: "text", label: "Inside books" },
  { id: "details", label: "Titles and details" },
  { id: "notes", label: "Notes and highlights" },
];

function Marked({ parts }: { parts: { text: string; hit: boolean }[] }) {
  return (
    <>
      {parts.map((p, i) =>
        p.hit ? (
          <mark
            key={i}
            className="rounded-sm bg-amber-200/80 px-0.5 text-inherit dark:bg-amber-500/35"
          >
            {p.text}
          </mark>
        ) : (
          <span key={i}>{p.text}</span>
        ),
      )}
    </>
  );
}

function openBook(
  book: { id: string; metadata: { title: string }; fileType: BookDto["fileType"] },
  query?: string,
  hit?: Hit,
) {
  useTabs.getState().open({
    bookId: book.id,
    title: book.metadata.title,
    fileType: book.fileType,
    findText: query ? { query, page: hit?.page, section: hit?.section } : undefined,
  });
}

function Section({
  icon: Icon,
  title,
  count,
  children,
  action,
}: {
  icon: typeof Search;
  title: string;
  count?: number;
  children: ReactNode;
  action?: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2">
      <header className="flex items-center gap-2 border-b pb-1.5">
        <Icon className="size-4 text-muted-foreground" aria-hidden />
        <h2 className="text-[13px] font-semibold">{title}</h2>
        {count !== undefined && (
          <span className="text-[12px] text-muted-foreground tabular-nums">{count}</span>
        )}
        <div className="flex-1" />
        {action}
      </header>
      {children}
    </section>
  );
}

function Snippet({ parts }: { parts: SnippetPart[] }) {
  return (
    <span className="line-clamp-2 text-[12.5px] leading-relaxed text-muted-foreground">
      <Marked parts={parts.map((p) => ({ text: p.text, hit: p.hit }))} />
    </span>
  );
}

function HitButton({ hit, onOpen }: { hit: Hit; onOpen: () => void }) {
  return (
    <button
      type="button"
      onClick={onOpen}
      className="flex w-full flex-col items-start gap-0.5 rounded-md px-2 py-1.5 text-left hover:bg-muted focus-visible:bg-muted focus-visible:outline-none"
    >
      <span className="text-[11.5px] font-medium">{hitPlace(hit)}</span>
      <Snippet parts={hit.snippet} />
    </button>
  );
}

function TextMatch({ match, query }: { match: TextMatchDto; query: string }) {
  const book = useMemo(() => toView(match.book), [match.book]);
  const [all, setAll] = useState(false);
  const { data: more, isFetching } = useBookHits(book.id, query, all);
  const hits = all && more ? more : match.hits;
  return (
    <div className="flex gap-3 py-2">
      <button
        type="button"
        onClick={() => openBook(book)}
        className="w-10 shrink-0 self-start pt-0.5"
        aria-label={`Open ${book.metadata.title}`}
      >
        <BookCover book={book} className="w-10" />
      </button>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <div className="flex items-baseline gap-2">
          <button
            type="button"
            onClick={() => openBook(book)}
            className="truncate text-left text-[13.5px] font-medium hover:underline"
          >
            {book.metadata.title}
          </button>
          <span className="truncate text-[12px] text-muted-foreground">
            {book.metadata.authors.join(", ")}
          </span>
          <span className="ml-auto shrink-0 text-[11.5px] text-muted-foreground tabular-nums">
            {match.total === 1 ? "1 match" : `${match.total} matches`}
          </span>
        </div>
        <div className="-mx-2 flex flex-col">
          {hits.map((h, i) => (
            <HitButton key={i} hit={h} onOpen={() => openBook(book, query, h)} />
          ))}
        </div>
        {match.total > match.hits.length && !all && (
          <button
            type="button"
            onClick={() => setAll(true)}
            className="flex items-center gap-1 self-start text-[12px] font-medium text-muted-foreground hover:text-foreground"
          >
            <ChevronDown className="size-3.5" aria-hidden />
            Show all {match.total} in this book
          </button>
        )}
        {all && isFetching && (
          <Loader2 className="size-3.5 animate-spin text-muted-foreground" aria-hidden />
        )}
      </div>
    </div>
  );
}

function BookRow({ book, query }: { book: BookView; query: string }) {
  return (
    <button
      type="button"
      onClick={() => openBook(book)}
      className="flex items-center gap-3 rounded-md px-2 py-1.5 text-left hover:bg-muted"
    >
      <BookCover book={book} className="w-8 shrink-0" />
      <span className="flex min-w-0 flex-col">
        <span className="truncate text-[13px] font-medium">
          <Marked parts={markWords(book.metadata.title, query)} />
        </span>
        <span className="truncate text-[12px] text-muted-foreground">
          <Marked parts={markWords(book.metadata.authors.join(", "), query)} />
        </span>
      </span>
    </button>
  );
}

function NoteRow({ note, query }: { note: NoteDto; query: string }) {
  const a = note.annotation;
  const text = a.quote?.exact || a.label || "Bookmark";
  return (
    <button
      type="button"
      onClick={() =>
        useTabs
          .getState()
          .open({ bookId: a.bookId, title: note.bookTitle, fileType: note.fileType, jumpTo: a.id })
      }
      className="flex flex-col items-start gap-0.5 rounded-md px-2 py-1.5 text-left hover:bg-muted"
    >
      <span className="text-[11.5px] font-medium">
        {note.bookTitle}
        {a.label ? ` · ${a.label}` : ""}
      </span>
      <span className="line-clamp-2 text-[12.5px]">
        <Marked parts={markWords(text, query)} />
      </span>
      {a.note && (
        <span className="line-clamp-2 text-[12px] text-muted-foreground">
          <Marked parts={markWords(a.note, query)} />
        </span>
      )}
    </button>
  );
}

/** Books whose pages are scans without text, with a way to fix them. */
function WithoutText() {
  const { data: missing = [] } = useBooksWithoutText();
  const { data: books = [] } = useBooks({});
  const { editLibrary } = usePermissions();
  const openOcr = useOcrDialog((s) => s.open);
  const [open, setOpen] = useState(false);
  if (!missing.length) return null;
  const ids = new Set(missing.map((m) => m.id));
  const list = books.filter((b) => ids.has(b.id));
  return (
    <Section
      icon={ScanText}
      title="Scanned books that cannot be searched yet"
      count={missing.length}
      action={
        editLibrary && (
          <Button size="sm" variant="outline" onClick={() => openOcr(missing.map((m) => m.id))}>
            <ScanText /> Make all searchable…
          </Button>
        )
      }
    >
      <p className="text-[12.5px] text-muted-foreground">
        Their pages are pictures of text. OCR reads them so their words can be found.
      </p>
      {open ? (
        <div className="flex flex-col">
          {list.map((b) => (
            <div key={b.id} className="flex items-center gap-2">
              <div className="flex-1">
                <BookRow book={b} query="" />
              </div>
              {editLibrary && (
                <Button size="sm" variant="ghost" onClick={() => openOcr([b.id])}>
                  Make searchable…
                </Button>
              )}
            </div>
          ))}
        </div>
      ) : (
        <button
          type="button"
          onClick={() => setOpen(true)}
          className="self-start text-[12px] font-medium text-muted-foreground hover:text-foreground"
        >
          Show them
        </button>
      )}
    </Section>
  );
}

function IndexLine() {
  const { running, done, total } = useIndexing();
  const { data } = useIndexStatus();
  if (running && total > 0)
    return (
      <span className="flex items-center gap-1.5">
        <Loader2 className="size-3 animate-spin" aria-hidden />
        Reading books for search: {done} of {total}
      </span>
    );
  if (!data) return null;
  const n = data.counts.withText + data.counts.partial;
  return <span>Searching the text of {n === 1 ? "1 book" : `${n} books`}</span>;
}

/** Board 15: one search over the words inside books, their details and notes. */
export function SearchView() {
  const initial = useLibraryView((s) => (s.nav.kind === "search" ? (s.nav.query ?? "") : ""));
  const [query, setQuery] = useState(initial);
  const [scope, setScope] = useState<Scope>("all");
  const deferred = useDeferredValue(query);
  const q = deferred.trim();
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);

  const wantText = q.length >= 2 && (scope === "all" || scope === "text");
  const text = useTextSearch(wantText ? q : "");
  const wantDetails = q.length >= 2 && (scope === "all" || scope === "details");
  const details = useBooks({ search: q }, wantDetails);
  const { data: notes = [] } = useAllNotes();
  const noteHits = useMemo(
    () =>
      q.length >= 2 && (scope === "all" || scope === "notes")
        ? filterNotes(notes, { search: q, kind: "all", colors: [], bookId: null })
        : [],
    [notes, q, scope],
  );
  const bookHits = wantDetails ? (details.data ?? []) : [];
  // A switched-off search keeps its last results (placeholder): not shown.
  const textHits = wantText ? (text.data ?? []) : [];
  const limit = scope === "all" ? 6 : 200;
  const nothing =
    q.length >= 2 &&
    !(wantText && text.isFetching) &&
    !details.isFetching &&
    !textHits.length &&
    !bookHits.length &&
    !noteHits.length;

  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex flex-col gap-3 border-b px-6 pt-4 pb-3">
        <div className="flex items-center gap-3">
          <h1 className="text-xl font-semibold tracking-tight">Search</h1>
          <div className="flex-1" />
          <span className="text-[12px] text-muted-foreground">
            <IndexLine />
          </span>
        </div>
        <div className="relative">
          <Search
            className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <input
            ref={input}
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => e.key === "Escape" && setQuery("")}
            placeholder='Words inside books, titles, authors, notes…   "a phrase"   -leave-out'
            aria-label="Search"
            className="h-10 w-full rounded-lg border border-input bg-background pr-16 pl-9 text-[14px] outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-2 focus-visible:ring-ring/20"
          />
          <Kbd
            action="go.search"
            className="pointer-events-none absolute top-1/2 right-3 -translate-y-1/2"
          />
        </div>
        <div className="flex gap-1" role="tablist" aria-label="Search in">
          {SCOPES.map((s) => (
            <button
              key={s.id}
              type="button"
              role="tab"
              aria-selected={scope === s.id}
              onClick={() => setScope(s.id)}
              className={cn(
                "h-7 rounded-md px-3 text-[12.5px]",
                scope === s.id
                  ? "bg-muted font-medium"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {s.label}
            </button>
          ))}
        </div>
      </header>

      <div className="flex-1 overflow-auto px-6 py-4">
        <div className="mx-auto flex max-w-3xl flex-col gap-6">
          {q.length < 2 && (
            <>
              <div className="flex flex-col items-center gap-2 py-10 text-center text-muted-foreground">
                <FileSearch className="size-8" aria-hidden />
                <p className="text-[13px]">
                  Type at least two letters. Use quotes for a phrase and a minus sign to leave a
                  word out.
                </p>
              </div>
              <WithoutText />
            </>
          )}

          {nothing && (
            <div className="flex flex-col items-center gap-2 py-10 text-center text-muted-foreground">
              <SearchX className="size-8" aria-hidden />
              <p className="text-[13px]">Nothing found for “{q}”.</p>
            </div>
          )}

          {bookHits.length > 0 && (
            <Section
              icon={BookOpen}
              title="Titles and details"
              count={bookHits.length}
              action={
                <button
                  type="button"
                  className="text-[12px] font-medium text-muted-foreground hover:text-foreground"
                  onClick={() => {
                    const v = useLibraryView.getState();
                    v.setNav({ kind: "all" });
                    v.setSearch(q);
                  }}
                >
                  Show in the library
                </button>
              }
            >
              <div className="grid grid-cols-1 gap-0.5 sm:grid-cols-2">
                {bookHits.slice(0, scope === "all" ? 6 : 200).map((b) => (
                  <BookRow key={b.id} book={b} query={q} />
                ))}
              </div>
            </Section>
          )}

          {(scope === "all" || scope === "text") &&
            q.length >= 2 &&
            (textHits.length > 0 || text.isFetching) && (
              <Section icon={FileSearch} title="Inside books" count={textHits.length}>
                {text.isFetching && !textHits.length ? (
                  <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden />
                ) : (
                  <div className="flex flex-col divide-y">
                    {textHits.slice(0, scope === "all" ? 20 : 100).map((m) => (
                      <TextMatch key={m.book.id} match={m} query={q} />
                    ))}
                  </div>
                )}
              </Section>
            )}

          {noteHits.length > 0 && (
            <Section icon={Highlighter} title="Notes and highlights" count={noteHits.length}>
              <div className="flex flex-col">
                {noteHits.slice(0, limit).map((n) => (
                  <NoteRow key={n.annotation.id} note={n} query={q} />
                ))}
              </div>
            </Section>
          )}
        </div>
      </div>
    </div>
  );
}
