import { useMemo, useState } from "react";
import { toast } from "sonner";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input, NativeSelect } from "@/components/ui/input";
import type { BulkEdit, ContentType } from "@/lib/ipc";
import { useBulkEdit } from "../api";
import { useLibraryDialogs } from "../dialogs";
import { CONTENT_TYPE_LABEL, type BookView } from "../model";
import { ListInput } from "./MetadataForm";

type TextField = "publisher" | "year" | "language" | "series";

const TEXT_FIELDS: { key: TextField; label: string; placeholder: string }[] = [
  { key: "publisher", label: "Publisher", placeholder: "Leave empty to remove" },
  { key: "year", label: "Year", placeholder: "e.g. 2021, or empty to remove" },
  { key: "language", label: "Language", placeholder: "e.g. en, de" },
  { key: "series", label: "Series", placeholder: "Leave empty to remove" },
];

/** Values shared by every book, or null when they differ. */
function shared(books: BookView[], get: (b: BookView) => string): string | null {
  const first = get(books[0]!);
  return books.every((b) => get(b) === first) ? first : null;
}

function counts(lists: string[][]): [string, number][] {
  const m = new Map<string, number>();
  for (const l of lists) for (const x of l) m.set(x, (m.get(x) ?? 0) + 1);
  return [...m.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
}

/** Board 10: change details of many books at once. */
export function BulkEditDialog() {
  const books = useLibraryDialogs((s) => s.bulkEdit);
  const close = useLibraryDialogs((s) => s.closeBulkEdit);
  return (
    <Dialog
      open={books !== null}
      onOpenChange={(o) => !o && close()}
      title={
        books ? `Edit ${books.length === 1 ? "1 book" : `${books.length} books`} together` : ""
      }
      description="Only the fields you tick are changed. Everything else stays as it is on each book."
      className="w-[600px]"
    >
      {books && <BulkForm key={books.map((b) => b.id).join()} books={books} onDone={close} />}
    </Dialog>
  );
}

function BulkForm({ books, onDone }: { books: BookView[]; onDone: () => void }) {
  const save = useBulkEdit();
  const [on, setOn] = useState<Record<string, boolean>>({});
  const [text, setText] = useState<Record<TextField, string>>(() => ({
    publisher: shared(books, (b) => b.metadata.publisher ?? "") ?? "",
    year: shared(books, (b) => (b.metadata.year ?? "").toString()) ?? "",
    language: shared(books, (b) => b.metadata.language ?? "") ?? "",
    series: shared(books, (b) => b.metadata.series ?? "") ?? "",
  }));
  const [authors, setAuthors] = useState<string[]>(() =>
    (shared(books, (b) => b.metadata.authors.join("\u0000")) ?? "").split("\u0000").filter(Boolean),
  );
  const [contentType, setContentType] = useState<ContentType>(
    () => (shared(books, (b) => b.metadata.contentType) as ContentType | null) ?? "book",
  );
  const [numberSeries, setNumberSeries] = useState(false);
  const [addTags, setAddTags] = useState<string[]>([]);
  const [removeTags, setRemoveTags] = useState<string[]>([]);
  const [addCats, setAddCats] = useState<string[]>([]);
  const [removeCats, setRemoveCats] = useState<string[]>([]);
  const tagCounts = useMemo(() => counts(books.map((b) => b.metadata.tags)), [books]);
  const catCounts = useMemo(() => counts(books.map((b) => b.metadata.categories)), [books]);

  const edit: BulkEdit = {
    authors: on.authors ? authors : null,
    publisher: on.publisher ? text.publisher : null,
    year: on.year ? text.year : null,
    language: on.language ? text.language : null,
    contentType: on.contentType ? contentType : null,
    series: on.series ? text.series : null,
    numberSeries: on.series && numberSeries,
    addTags,
    removeTags,
    addCategories: addCats,
    removeCategories: removeCats,
  };
  const changes =
    Object.values(on).filter(Boolean).length +
    addTags.length +
    removeTags.length +
    addCats.length +
    removeCats.length;

  const submit = () =>
    save.mutate(
      { ids: books.map((b) => b.id), edit },
      {
        onSuccess: (n) => {
          toast.success(n === 1 ? "Updated 1 book" : `Updated ${n} books`);
          onDone();
        },
      },
    );

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        submit();
      }}
    >
      <p className="line-clamp-2 text-[12px] text-muted-foreground">
        {books
          .slice(0, 6)
          .map((b) => b.metadata.title)
          .join(" · ")}
        {books.length > 6 && ` · and ${books.length - 6} more`}
      </p>
      <div className="flex items-start gap-3">
        <Toggle id="authors" label="Authors" on={on} setOn={setOn} />
        <div className="flex-1" aria-disabled={!on.authors}>
          <ListInput
            value={authors}
            onChange={(v) => {
              setAuthors(v);
              setOn({ ...on, authors: true });
            }}
            placeholder="Replace the authors…"
          />
        </div>
      </div>
      {TEXT_FIELDS.map((f) => (
        <div key={f.key} className="flex items-start gap-3">
          <Toggle id={f.key} label={f.label} on={on} setOn={setOn} />
          <div className="flex flex-1 flex-col gap-1.5">
            <Input
              value={text[f.key]}
              placeholder={f.placeholder}
              onChange={(e) => {
                setText({ ...text, [f.key]: e.target.value });
                setOn({ ...on, [f.key]: true });
              }}
            />
            {f.key === "series" && on.series && (
              <label className="flex items-center gap-2 text-[12.5px] text-muted-foreground">
                <input
                  type="checkbox"
                  checked={numberSeries}
                  onChange={(e) => setNumberSeries(e.target.checked)}
                  className="size-4 accent-primary"
                />
                Number the books 1, 2, 3… in the order shown
              </label>
            )}
          </div>
        </div>
      ))}
      <div className="flex items-start gap-3">
        <Toggle id="contentType" label="Content type" on={on} setOn={setOn} />
        <NativeSelect
          value={contentType}
          onChange={(e) => {
            setContentType(e.target.value as ContentType);
            setOn({ ...on, contentType: true });
          }}
          className="flex-1"
        >
          {(Object.keys(CONTENT_TYPE_LABEL) as ContentType[]).map((t) => (
            <option key={t} value={t}>
              {CONTENT_TYPE_LABEL[t]}
            </option>
          ))}
        </NativeSelect>
      </div>

      <ChipSection
        title="Tags"
        existing={tagCounts}
        total={books.length}
        add={addTags}
        setAdd={setAddTags}
        remove={removeTags}
        setRemove={setRemoveTags}
      />
      <ChipSection
        title="Categories"
        existing={catCounts}
        total={books.length}
        add={addCats}
        setAdd={setAddCats}
        remove={removeCats}
        setRemove={setRemoveCats}
        placeholder="Add a category, e.g. Science/Physics"
      />

      {save.error && <p className="text-destructive">{save.error.message}</p>}
      <div className="flex items-center justify-end gap-2 border-t pt-3">
        <span className="mr-auto text-[12px] text-muted-foreground">
          {changes === 0 ? "Nothing to change yet" : `${changes} change${changes === 1 ? "" : "s"}`}
        </span>
        <Button type="button" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" disabled={changes === 0 || save.isPending}>
          Apply to {books.length === 1 ? "1 book" : `${books.length} books`}
        </Button>
      </div>
    </form>
  );
}

function Toggle({
  id,
  label,
  on,
  setOn,
}: {
  id: string;
  label: string;
  on: Record<string, boolean>;
  setOn: (v: Record<string, boolean>) => void;
}) {
  return (
    <label className="flex w-36 shrink-0 items-center gap-2 pt-1.5 text-[13px] font-medium">
      <input
        type="checkbox"
        checked={!!on[id]}
        onChange={(e) => setOn({ ...on, [id]: e.target.checked })}
        className="size-4 accent-primary"
      />
      {label}
    </label>
  );
}

function ChipSection({
  title,
  existing,
  total,
  add,
  setAdd,
  remove,
  setRemove,
  placeholder = "Add…",
}: {
  title: string;
  existing: [string, number][];
  total: number;
  add: string[];
  setAdd: (v: string[]) => void;
  remove: string[];
  setRemove: (v: string[]) => void;
  placeholder?: string;
}) {
  return (
    <div className="flex items-start gap-3">
      <span className="w-36 shrink-0 pt-1.5 text-[13px] font-medium">{title}</span>
      <div className="flex flex-1 flex-col gap-2">
        {existing.length > 0 && (
          <div className="flex flex-wrap gap-1.5" aria-label={`${title} on these books`}>
            {existing.map(([name, n]) => {
              const removing = remove.includes(name);
              return (
                <button
                  key={name}
                  type="button"
                  aria-pressed={removing}
                  title={
                    removing ? "Keep" : `Remove from ${n === total ? "all" : `${n} of ${total}`}`
                  }
                  onClick={() =>
                    setRemove(removing ? remove.filter((r) => r !== name) : [...remove, name])
                  }
                  className={
                    "flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-[12px] " +
                    (removing
                      ? "border-destructive/50 text-destructive line-through"
                      : "bg-sidebar")
                  }
                >
                  {name}
                  <span className="text-muted-foreground no-underline">
                    {n === total ? "" : `${n}/${total}`}
                  </span>
                  <X className="size-3" aria-hidden />
                </button>
              );
            })}
          </div>
        )}
        <ListInput
          value={add}
          onChange={setAdd}
          placeholder={placeholder}
          splitOnComma={title === "Tags"}
        />
      </div>
    </div>
  );
}
