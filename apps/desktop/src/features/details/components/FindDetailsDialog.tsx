import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { AlertTriangle, ExternalLink, Loader2, ScanBarcode, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { commands, type Candidate, type Lookup, type Query } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { BookCover, useBook, type BookView } from "@/features/library";
import { useApplyDetails, useCoverPreview, useDetailsQuery, useFindDetails } from "../api";
import {
  cleanQuery,
  defaultPicks,
  differs,
  FIELDS,
  full,
  isEmpty,
  matchLabel,
  merge,
  newItems,
  show,
  SOURCE_LABEL,
  type FieldDef,
  type Meta,
  type Picks,
} from "../model";
import { useDetailsDialog } from "../store";
import { ScanDialog } from "./ScanDialog";

/** Board 5: look a book up online and pick, field by field, what to keep. */
export function FindDetailsDialog() {
  const bookId = useDetailsDialog((s) => s.bookId);
  const close = useDetailsDialog((s) => s.close);
  return (
    <Dialog
      open={bookId !== null}
      onOpenChange={(o) => !o && close()}
      title="Find details online"
      description="Libreri asks free book and paper sources, then you choose what to keep. Only the ISBN, DOI, arXiv id, title and author are sent."
      className="w-[1000px]"
    >
      {bookId && <Finder key={bookId} bookId={bookId} onDone={close} />}
    </Dialog>
  );
}

function Finder({ bookId, onDone }: { bookId: string; onDone: () => void }) {
  const book = useBook(bookId).data;
  const initial = useDetailsQuery(bookId);
  const find = useFindDetails();
  const [form, setForm] = useState<Query | null>(null);
  const [result, setResult] = useState<Lookup | null>(null);
  const [selected, setSelected] = useState(0);
  const [picks, setPicks] = useState<Picks>({});
  const [coverFrom, setCoverFrom] = useState<number | null>(null);
  const [scanOpen, setScanOpen] = useState(false);

  // The form starts with what the book would be looked up by.
  if (form === null && initial.data) setForm(initial.data);

  const run = (q: Query) => {
    if (!book) return;
    find.mutate(
      { id: bookId, query: cleanQuery(q) },
      {
        onSuccess: (r) => {
          setResult(r);
          setSelected(0);
          const first = r.candidates[0];
          setPicks(first ? defaultPicks(book.metadata, full(first.metadata), 0) : {});
          setCoverFrom(first?.coverUrl && !book.hasCover ? 0 : null);
        },
      },
    );
  };

  // Search once, as soon as the book and its identifiers are known.
  const started = useRef(false);
  useEffect(() => {
    if (!started.current && book && initial.data) {
      started.current = true;
      run(initial.data);
    }
  });

  if (!book || form === null) {
    return (
      <p className="flex items-center gap-2 py-10 text-muted-foreground">
        <Loader2 className="size-4 animate-spin" /> Getting ready…
      </p>
    );
  }

  const set = (k: keyof Query) => (e: React.ChangeEvent<HTMLInputElement>) =>
    setForm({ ...form, [k]: e.target.value });

  return (
    <div className="grid min-h-0 grid-cols-[300px_1fr] gap-5">
      <div className="flex min-h-0 flex-col gap-4">
        <form
          className="flex flex-col gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            run(form);
          }}
        >
          <Field label="Title">
            <Input value={form.title ?? ""} onChange={set("title")} />
          </Field>
          <Field label="Author">
            <Input value={form.author ?? ""} onChange={set("author")} />
          </Field>
          <Field label="ISBN">
            <div className="flex gap-1.5">
              <Input value={form.isbn ?? ""} onChange={set("isbn")} className="font-mono" />
              <Button
                type="button"
                size="icon"
                variant="outline"
                aria-label="Scan a barcode"
                title="Scan a barcode"
                onClick={() => setScanOpen(true)}
              >
                <ScanBarcode />
              </Button>
            </div>
          </Field>
          <div className="grid grid-cols-2 gap-2">
            <Field label="DOI">
              <Input value={form.doi ?? ""} onChange={set("doi")} className="font-mono" />
            </Field>
            <Field label="arXiv">
              <Input value={form.arxivId ?? ""} onChange={set("arxivId")} className="font-mono" />
            </Field>
          </div>
          <Button type="submit" disabled={find.isPending}>
            {find.isPending ? <Loader2 className="animate-spin" /> : <Search />} Search
          </Button>
          {find.error && <p className="text-[12.5px] text-destructive">{find.error.message}</p>}
        </form>
        <Candidates
          result={result}
          pending={find.isPending}
          selected={selected}
          onSelect={setSelected}
        />
      </div>
      <div className="flex min-h-0 min-w-0 flex-col gap-3">
        {result && result.candidates[selected] ? (
          <Merge
            book={book}
            candidates={result.candidates}
            selected={selected}
            picks={picks}
            setPicks={setPicks}
            coverFrom={coverFrom}
            setCoverFrom={setCoverFrom}
            onDone={onDone}
          />
        ) : (
          <div className="flex flex-1 flex-col items-center justify-center gap-2 rounded-lg border border-dashed p-8 text-center text-muted-foreground">
            {find.isPending ? (
              <>
                <Loader2 className="size-5 animate-spin" />
                Asking the sources…
              </>
            ) : result ? (
              <>
                Nothing found. Try a shorter title, only the author's last name, or the ISBN from
                the back of the book.
              </>
            ) : (
              "Search to see what the sources know about this book."
            )}
          </div>
        )}
        {result && result.errors.length > 0 && (
          <p className="flex gap-2 text-[12.5px] text-muted-foreground">
            <AlertTriangle className="mt-0.5 size-3.5 shrink-0" />
            <span>
              {result.errors.map((e) => `${SOURCE_LABEL[e.source]}: ${e.message}`).join(" · ")}
            </span>
          </p>
        )}
      </div>
      <ScanDialog
        open={scanOpen}
        onOpenChange={setScanOpen}
        onScanned={(isbn) => {
          setScanOpen(false);
          const next = { ...form, isbn };
          setForm(next);
          run(next);
        }}
      />
    </div>
  );
}

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-[11.5px] font-medium text-muted-foreground">{label}</span>
      {children}
    </label>
  );
}

function Candidates({
  result,
  pending,
  selected,
  onSelect,
}: {
  result: Lookup | null;
  pending: boolean;
  selected: number;
  onSelect: (i: number) => void;
}) {
  if (!result || result.candidates.length === 0) return null;
  return (
    <div className={cn("flex min-h-0 flex-col gap-1", pending && "opacity-60")}>
      <h3 className="text-[11.5px] font-medium text-muted-foreground">
        {result.candidates.length === 1 ? "1 match" : `${result.candidates.length} matches`}
      </h3>
      <ul
        role="listbox"
        aria-label="Matches"
        className="flex max-h-72 flex-col gap-1 overflow-y-auto"
      >
        {result.candidates.map((c, i) => (
          <li key={`${c.source}-${c.sourceId}-${i}`}>
            <button
              type="button"
              role="option"
              aria-selected={i === selected}
              onClick={() => onSelect(i)}
              className={cn(
                "flex w-full flex-col gap-0.5 rounded-md border px-2.5 py-2 text-left hover:bg-muted",
                i === selected && "border-foreground/60 bg-muted",
              )}
            >
              <span className="line-clamp-2 font-medium">{c.metadata.title}</span>
              <span className="line-clamp-1 text-[12px] text-muted-foreground">
                {[c.metadata.authors?.join(", "), c.metadata.year].filter(Boolean).join(" · ") ||
                  "No author given"}
              </span>
              <span className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
                <span className="rounded bg-background px-1.5 py-px font-medium text-foreground">
                  {SOURCE_LABEL[c.source]}
                </span>
                {matchLabel(c)}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}

function Merge({
  book,
  candidates,
  selected,
  picks,
  setPicks,
  coverFrom,
  setCoverFrom,
  onDone,
}: {
  book: BookView;
  candidates: Candidate[];
  selected: number;
  picks: Picks;
  setPicks: (p: Picks) => void;
  coverFrom: number | null;
  setCoverFrom: (i: number | null) => void;
  onDone: () => void;
}) {
  const apply = useApplyDetails();
  const [onlyChanges, setOnlyChanges] = useState(true);
  const current = book.metadata;
  const found = candidates.map((c) => full(c.metadata));
  const here = found[selected]!;
  const candidate = candidates[selected]!;
  const rows = FIELDS.filter(
    (f) =>
      picks[f.key] !== undefined ||
      differs(f, current, here) ||
      (!onlyChanges && !(isEmpty(current[f.key]) && isEmpty(here[f.key]))),
  );
  const count = Object.keys(picks).length + (coverFrom !== null ? 1 : 0);

  const toggle = (f: FieldDef, on: boolean) => {
    const next = { ...picks };
    if (on) next[f.key] = selected;
    else delete next[f.key];
    setPicks(next);
  };

  const save = () => {
    const metadata = merge(current, found, picks);
    const coverUrl = coverFrom !== null ? (candidates[coverFrom]?.coverUrl ?? null) : null;
    apply.mutate(
      { id: book.id, metadata, coverUrl },
      {
        onSuccess: (r) => {
          if (r.coverError)
            toast.warning("Details saved, but not the cover", { description: r.coverError });
          else toast.success("Details saved");
          onDone();
        },
      },
    );
  };

  return (
    <>
      <div className="flex items-center gap-2">
        <div className="flex min-w-0 flex-1 flex-col">
          <span className="truncate font-medium">
            {SOURCE_LABEL[candidate.source]} · {matchLabel(candidate)}
          </span>
          {candidate.link && (
            <button
              type="button"
              className="flex w-fit items-center gap-1 text-[12px] text-muted-foreground hover:text-foreground"
              onClick={() => void commands.openExternalUrl(candidate.link!)}
            >
              See it on {SOURCE_LABEL[candidate.source]} <ExternalLink className="size-3" />
            </button>
          )}
        </div>
        <Button
          size="sm"
          variant="outline"
          onClick={() => {
            const next = { ...picks };
            for (const f of FIELDS) if (differs(f, current, here)) next[f.key] = selected;
            setPicks(next);
            if (candidate.coverUrl) setCoverFrom(selected);
          }}
        >
          Take all from this match
        </Button>
        <Button
          size="sm"
          variant="ghost"
          onClick={() => {
            setPicks({});
            setCoverFrom(null);
          }}
        >
          Keep mine
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto rounded-lg border">
        <table className="w-full table-fixed border-collapse text-[13px]">
          <thead className="sticky top-0 z-10 bg-popover text-left text-[11.5px] text-muted-foreground">
            <tr className="border-b">
              <th className="w-28 px-3 py-2 font-medium">Field</th>
              <th className="px-3 py-2 font-medium">This book</th>
              <th className="px-3 py-2 font-medium">Found</th>
              <th className="w-16 px-3 py-2 text-center font-medium">Use</th>
            </tr>
          </thead>
          <tbody>
            <CoverRow
              book={book}
              candidates={candidates}
              selected={selected}
              coverFrom={coverFrom}
              setCoverFrom={setCoverFrom}
            />
            {rows.map((f) => (
              <FieldRow
                key={f.key}
                field={f}
                current={current}
                here={here}
                pickedFrom={picks[f.key]}
                selected={selected}
                sourceOf={(i) => SOURCE_LABEL[candidates[i]!.source]}
                onToggle={(on) => toggle(f, on)}
              />
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={4} className="px-3 py-6 text-center text-muted-foreground">
                  This match has nothing this book does not already have.
                </td>
              </tr>
            )}
          </tbody>
        </table>
      </div>

      <div className="flex items-center gap-3">
        <label className="flex items-center gap-2 text-[12.5px] text-muted-foreground">
          <input
            type="checkbox"
            className="size-4 accent-primary"
            checked={onlyChanges}
            onChange={(e) => setOnlyChanges(e.target.checked)}
          />
          Only show what would change
        </label>
        <div className="flex-1" />
        {apply.error && (
          <span className="text-[12.5px] text-destructive">{apply.error.message}</span>
        )}
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button onClick={save} disabled={count === 0 || apply.isPending}>
          {apply.isPending && <Loader2 className="animate-spin" />}
          {count === 0
            ? "Nothing to save"
            : count === 1
              ? "Save 1 change"
              : `Save ${count} changes`}
        </Button>
      </div>
    </>
  );
}

function FieldRow({
  field,
  current,
  here,
  pickedFrom,
  selected,
  sourceOf,
  onToggle,
}: {
  field: FieldDef;
  current: Meta;
  here: Meta;
  pickedFrom: number | undefined;
  selected: number;
  sourceOf: (i: number) => string;
  onToggle: (on: boolean) => void;
}) {
  const mine = show(field, current[field.key]);
  const theirs =
    field.kind === "addList"
      ? newItems(current[field.key] as string[], here[field.key] as string[])
      : here[field.key];
  const text = show(field, theirs);
  const can = differs(field, current, here);
  const on = pickedFrom === selected;
  const long = field.kind === "long";
  return (
    <tr className={cn("border-b align-top last:border-b-0", on && "bg-muted/60")}>
      <td className="px-3 py-2 text-muted-foreground">{field.label}</td>
      <td
        className={cn(
          "px-3 py-2 break-words",
          on && field.kind !== "addList" && "text-muted-foreground line-through",
        )}
      >
        <Value text={mine} long={long} />
      </td>
      <td className="px-3 py-2 break-words">
        {field.kind === "addList" && text ? (
          <span>
            <span className="text-muted-foreground">Add </span>
            {text}
          </span>
        ) : (
          <Value text={text} long={long} />
        )}
        {pickedFrom !== undefined && pickedFrom !== selected && (
          <span className="mt-0.5 block text-[11.5px] text-muted-foreground">
            Using {sourceOf(pickedFrom)}’s
          </span>
        )}
      </td>
      <td className="px-3 py-2 text-center">
        <input
          type="checkbox"
          className="size-4 accent-primary"
          aria-label={`Use the found ${field.label.toLowerCase()}`}
          checked={on}
          disabled={!can && !on}
          onChange={(e) => onToggle(e.target.checked)}
        />
      </td>
    </tr>
  );
}

function Value({ text, long }: { text: string; long: boolean }) {
  if (!text) return <span className="text-muted-foreground/60">—</span>;
  return long ? <p className="line-clamp-6 whitespace-pre-line">{text}</p> : <>{text}</>;
}

function CoverRow({
  book,
  candidates,
  selected,
  coverFrom,
  setCoverFrom,
}: {
  book: BookView;
  candidates: Candidate[];
  selected: number;
  coverFrom: number | null;
  setCoverFrom: (i: number | null) => void;
}) {
  const url = candidates[selected]?.coverUrl ?? null;
  const preview = useCoverPreview(url);
  const on = coverFrom === selected;
  return (
    <tr className={cn("border-b align-top", on && "bg-muted/60")}>
      <td className="px-3 py-2 text-muted-foreground">Cover</td>
      <td className="px-3 py-2">
        <div className="w-20">
          <BookCover book={book} />
        </div>
      </td>
      <td className="px-3 py-2">
        {!url ? (
          <span className="text-muted-foreground/60">—</span>
        ) : preview.isPending ? (
          <div className="flex aspect-[2/3] w-20 items-center justify-center rounded-[3px] bg-muted">
            <Loader2 className="size-4 animate-spin text-muted-foreground" />
          </div>
        ) : preview.data ? (
          <img src={preview.data} alt="Found cover" className="w-20 rounded-[3px] shadow" />
        ) : (
          <span className="text-[12px] text-muted-foreground">
            {preview.error?.message ?? "No cover"}
          </span>
        )}
        {coverFrom !== null && coverFrom !== selected && (
          <span className="mt-0.5 block text-[11.5px] text-muted-foreground">
            Using {SOURCE_LABEL[candidates[coverFrom]!.source]}’s
          </span>
        )}
      </td>
      <td className="px-3 py-2 text-center">
        <input
          type="checkbox"
          className="size-4 accent-primary"
          aria-label="Use the found cover"
          checked={on}
          disabled={!preview.data}
          onChange={(e) => setCoverFrom(e.target.checked ? selected : null)}
        />
      </td>
    </tr>
  );
}
