import { useState, type ReactNode } from "react";
import {
  AlertTriangle,
  BookOpen,
  Download,
  FileSearch,
  FolderSearch,
  Globe,
  Heart,
  Pencil,
  Quote,
  Star,
  Trash2,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import { NativeSelect } from "@/components/ui/input";
import type { ReadingStatus } from "@/lib/ipc";
import { useShortcut } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";
import { useDetailsDialog, useFillDetails } from "@/features/details";
import { useLocateFile, usePortability } from "@/features/portability";
import { usePermissions } from "@/features/profiles";
import { TextStatusRow } from "@/features/search";
import { useUpdateBook } from "../api";
import { useLibraryDialogs } from "../dialogs";
import { useBookActions } from "../hooks/useBookActions";
import {
  CONTENT_TYPE_LABEL,
  FILE_TYPE_LABEL,
  fileName,
  formatDate,
  formatSize,
  STATUS_LABEL,
  type BookView,
} from "../model";
import { useLibraryView } from "../store";
import { BookCover } from "./BookCover";
import { MetadataForm } from "./MetadataForm";

function Row({ label, children, mono }: { label: string; children: ReactNode; mono?: boolean }) {
  if (children === null || children === undefined || children === "") return null;
  return (
    <div className="grid grid-cols-[92px_1fr] gap-2 py-1">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className={cn("min-w-0 break-words", mono && "font-mono text-[12px]")}>{children}</dd>
    </div>
  );
}

function Rating({ book }: { book: BookView }) {
  const { setRating } = useBookActions();
  return (
    <div className="flex items-center" role="radiogroup" aria-label="Rating">
      {[1, 2, 3, 4, 5].map((n) => (
        <button
          key={n}
          type="button"
          role="radio"
          aria-checked={book.user.rating === n}
          aria-label={`${n} star${n > 1 ? "s" : ""}`}
          onClick={() => setRating(book, book.user.rating === n ? 0 : n)}
          className="p-0.5 text-muted-foreground hover:text-foreground"
        >
          <Star className={cn("size-4", n <= book.user.rating && "fill-current text-foreground")} />
        </button>
      ))}
    </div>
  );
}

function Chips({ items }: { items: string[] }) {
  const toggleTag = useLibraryView((s) => s.toggleTag);
  if (!items.length) return null;
  return (
    <div className="flex flex-wrap gap-1">
      {items.map((t) => (
        <button
          key={t}
          type="button"
          onClick={() => toggleTag(t)}
          title="Show books with this tag"
          className="rounded bg-muted px-1.5 py-0.5 text-[12px] hover:bg-border"
        >
          {t}
        </button>
      ))}
    </div>
  );
}

function SingleBook({ book }: { book: BookView }) {
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const update = useUpdateBook();
  const actions = useBookActions();
  const { editLibrary } = usePermissions();
  const openFinder = useDetailsDialog((s) => s.open);
  const openCitation = usePortability((s) => s.openCitation);
  const m = book.metadata;
  useShortcut("details.edit", () => editLibrary && setEditing(true));

  if (editing) {
    return (
      <div className="flex flex-col gap-3 px-4 pt-4">
        <h2 className="text-[15px] font-semibold">Edit details</h2>
        <MetadataForm
          key={book.id}
          book={book}
          saving={update.isPending}
          error={error}
          onCancel={() => {
            setError(null);
            setEditing(false);
          }}
          onSave={(metadata) =>
            update.mutate(
              { id: book.id, metadata },
              {
                onSuccess: () => {
                  setError(null);
                  setEditing(false);
                },
                onError: (e) => setError(e instanceof Error ? e.message : String(e)),
              },
            )
          }
        />
      </div>
    );
  }

  return (
    <div className="flex flex-col gap-4 px-4 py-4">
      <div className="mx-auto w-40">
        <BookCover book={book} large />
      </div>
      <div className="flex flex-col gap-1 text-center">
        <h2 className="text-[15px] leading-snug font-semibold">{m.title}</h2>
        {m.subtitle && <p className="text-muted-foreground">{m.subtitle}</p>}
        <p className="text-muted-foreground">{m.authors.join(", ") || "Unknown author"}</p>
      </div>
      {book.missing && <MissingFile book={book} />}
      <div className="flex items-center justify-center gap-1.5">
        <Button size="sm" onClick={() => void actions.open(book)} disabled={book.missing}>
          <BookOpen /> Open
        </Button>
        <Button
          size="icon"
          variant="outline"
          aria-label="Show in file manager"
          onClick={() => void actions.reveal(book)}
          disabled={book.missing}
        >
          <FolderSearch />
        </Button>
        <Button
          size="icon"
          variant="outline"
          aria-label={book.user.favorite ? "Remove from Favourites" : "Add to Favourites"}
          aria-pressed={book.user.favorite}
          onClick={() => actions.toggleFavorite([book])}
        >
          <Heart className={cn(book.user.favorite && "fill-current")} />
        </Button>
        {editLibrary && (
          <Button
            size="icon"
            variant="outline"
            aria-label="Edit details"
            onClick={() => setEditing(true)}
          >
            <Pencil />
          </Button>
        )}
        <Button
          size="icon"
          variant="outline"
          aria-label="Cite"
          title="Cite"
          onClick={() => openCitation([book.id])}
        >
          <Quote />
        </Button>
        {editLibrary && (
          <Button
            size="icon"
            variant="outline"
            aria-label="Find details online"
            title="Find details online"
            onClick={() => openFinder(book.id)}
          >
            <Globe />
          </Button>
        )}
      </div>
      <div className="flex items-center gap-2">
        <NativeSelect
          aria-label="Reading status"
          value={book.user.status}
          onChange={(e) => actions.setStatus([book], e.target.value as ReadingStatus)}
          className="flex-1"
        >
          {(Object.keys(STATUS_LABEL) as ReadingStatus[]).map((s) => (
            <option key={s} value={s}>
              {STATUS_LABEL[s]}
            </option>
          ))}
        </NativeSelect>
        <Rating book={book} />
      </div>

      <dl className="flex flex-col border-t pt-2">
        <Row label="Type">{CONTENT_TYPE_LABEL[m.contentType]}</Row>
        <Row label="Series">
          {m.series && `${m.series}${m.seriesNumber !== null ? ` #${m.seriesNumber}` : ""}`}
        </Row>
        <Row label="Year">{m.year}</Row>
        <Row label="Publisher">{m.publisher}</Row>
        <Row label="Edition">{m.edition}</Row>
        <Row label="Pages">{m.pages}</Row>
        <Row label="Language">{m.language}</Row>
        <Row label="ISBN-13" mono>
          {m.isbn13}
        </Row>
        <Row label="ISBN-10" mono>
          {m.isbn10}
        </Row>
        <Row label="DOI" mono>
          {m.doi}
        </Row>
        <Row label="arXiv" mono>
          {m.arxivId}
        </Row>
        <Row label="Journal">
          {[m.journal, m.volume && `vol. ${m.volume}`, m.issue && `no. ${m.issue}`]
            .filter(Boolean)
            .join(", ")}
        </Row>
        <Row label="Contributors">{m.contributors.join(", ")}</Row>
        <Row label="Tags">{m.tags.length ? <Chips items={m.tags} /> : null}</Row>
        <Row label="Categories">{m.categories.join(" · ")}</Row>
        <Row label="Web page">{m.url}</Row>
      </dl>
      {m.about && (
        <div className="flex flex-col gap-1 border-t pt-3">
          <h3 className="text-[11.5px] font-medium text-muted-foreground">About</h3>
          <p className="leading-relaxed whitespace-pre-line">{m.about}</p>
        </div>
      )}
      <dl className="flex flex-col border-t pt-2 text-[12px]">
        <Row label="File">{`${FILE_TYPE_LABEL[book.fileType]} · ${formatSize(book.fileSize)}`}</Row>
        <Row label="Name" mono>
          {fileName(book.relPath)}
        </Row>
        <Row label="Folder" mono>
          {book.folder || "Books"}
        </Row>
        <Row label="Added">{formatDate(book.addedAt)}</Row>
        <TextStatusRow bookId={book.id} />
      </dl>
    </div>
  );
}

function ManyBooks({ books }: { books: BookView[] }) {
  const actions = useBookActions();
  const { editLibrary } = usePermissions();
  const openBulk = useLibraryDialogs((s) => s.openBulkEdit);
  const fillDetails = useFillDetails();
  const openCitation = usePortability((s) => s.openCitation);
  const openExport = usePortability((s) => s.openExport);
  const size = books.reduce((n, b) => n + b.fileSize, 0);
  return (
    <div className="flex flex-col gap-4 px-4 py-6">
      <div className="relative mx-auto h-44 w-40">
        {books.slice(0, 3).map((b, i) => (
          <div
            key={b.id}
            className="absolute w-28"
            style={{
              left: 6 + i * 18,
              top: i * 8,
              zIndex: 3 - i,
              transform: `rotate(${(i - 1) * 4}deg)`,
            }}
          >
            <BookCover book={b} />
          </div>
        ))}
      </div>
      <div className="text-center">
        <h2 className="text-[15px] font-semibold">{books.length} books selected</h2>
        <p className="text-muted-foreground">{formatSize(size)} in total</p>
      </div>
      <NativeSelect
        aria-label="Reading status"
        value=""
        onChange={(e) =>
          e.target.value && actions.setStatus(books, e.target.value as ReadingStatus)
        }
      >
        <option value="">Set reading status…</option>
        {(Object.keys(STATUS_LABEL) as ReadingStatus[]).map((s) => (
          <option key={s} value={s}>
            {STATUS_LABEL[s]}
          </option>
        ))}
      </NativeSelect>
      <Button variant="outline" onClick={() => actions.toggleFavorite(books)}>
        <Heart />{" "}
        {books.every((b) => b.user.favorite) ? "Remove from Favourites" : "Add to Favourites"}
      </Button>
      <Button variant="outline" onClick={() => openCitation(books.map((b) => b.id))}>
        <Quote /> Cite {books.length} books…
      </Button>
      {editLibrary && (
        <>
          <Button variant="outline" onClick={() => openExport(books.map((b) => b.id))}>
            <Download /> Export {books.length} books…
          </Button>
          <Button onClick={() => openBulk(books)}>
            <Pencil /> Edit {books.length} books together…
          </Button>
          <Button variant="outline" onClick={() => fillDetails(books.map((b) => b.id))}>
            <Globe /> Fill in missing details online
          </Button>
          <Button
            variant="outline"
            className="text-destructive"
            onClick={() => void actions.moveToTrash(books)}
          >
            <Trash2 /> Move to Trash
          </Button>
          <p className="text-center text-[12px] text-muted-foreground">
            Drag the selection onto a folder to move it.
          </p>
        </>
      )}
    </div>
  );
}

/** A book whose file is gone: its notes are kept; find the file again. */
function MissingFile({ book }: { book: BookView }) {
  const { editLibrary } = usePermissions();
  const locate = useLocateFile();
  const actions = useBookActions();
  return (
    <div className="flex flex-col gap-2 rounded-md border border-destructive/40 bg-destructive/10 px-2.5 py-2">
      <p className="flex gap-2 text-destructive">
        <AlertTriangle className="mt-0.5 size-4 shrink-0" />
        The file is missing. Notes and details are kept; put the file back (or import it again) and
        everything reconnects.
      </p>
      {editLibrary && (
        <div className="flex flex-wrap gap-1.5">
          <Button
            size="sm"
            variant="outline"
            onClick={() => void locate({ id: book.id, title: book.metadata.title })}
          >
            <FileSearch /> Locate file…
          </Button>
          <Button size="sm" variant="ghost" onClick={() => void actions.moveToTrash([book])}>
            Remove from library
          </Button>
        </div>
      )}
    </div>
  );
}

/** The right-hand panel: details of the selected book, or of the selection. */
export function DetailsPanel({ books }: { books: BookView[] }) {
  const { selection, setDetailsOpen } = useLibraryView();
  const selected = books.filter((b) => selection.includes(b.id));

  return (
    <aside aria-label="Details" className="flex w-80 shrink-0 flex-col border-l bg-sidebar">
      <div className="flex h-10 shrink-0 items-center justify-between border-b pr-2 pl-4">
        <span className="text-[11px] font-semibold tracking-wide text-muted-foreground">
          DETAILS
        </span>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Hide details"
          onClick={() => setDetailsOpen(false)}
        >
          <X />
        </Button>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto">
        {selected.length === 1 && selected[0] ? (
          <SingleBook key={selected[0].id} book={selected[0]} />
        ) : selected.length > 1 ? (
          <ManyBooks books={selected} />
        ) : (
          <p className="px-6 py-10 text-center text-muted-foreground">
            Select a book to see and edit its details.
          </p>
        )}
      </div>
    </aside>
  );
}
