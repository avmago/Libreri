import { useState, type KeyboardEvent, type ReactNode } from "react";
import { X } from "lucide-react";
import { ask } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Input, NativeSelect, Textarea } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import type { ContentType } from "@/lib/ipc";
import { useShortcut } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";
import {
  addListItems,
  CONTENT_TYPE_LABEL,
  parseNumberField,
  type BookView,
  type EMPTY_METADATA,
} from "../model";

type Metadata = typeof EMPTY_METADATA;

export function Field({
  label,
  children,
  className,
}: {
  label: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <label className={cn("flex flex-col gap-1", className)}>
      <span className="text-[11.5px] font-medium text-muted-foreground">{label}</span>
      {children}
    </label>
  );
}

/**
 * A list typed as chips: Enter or comma adds, Backspace on an empty field
 * removes the last one. Used for authors, tags and categories.
 */
export function ListInput({
  value,
  onChange,
  placeholder,
  splitOnComma = true,
  onDraft,
}: {
  value: string[];
  onChange: (v: string[]) => void;
  placeholder: string;
  splitOnComma?: boolean;
  /** Told what is typed but not added yet, so a save can include it. */
  onDraft?: (text: string) => void;
}) {
  const [draft, setDraftState] = useState("");
  const setDraft = (text: string) => {
    setDraftState(text);
    onDraft?.(text);
  };
  const add = (text: string) => {
    const next = addListItems(value, text, splitOnComma);
    if (next !== value) onChange(next);
    setDraft("");
  };
  const onKeyDown = (e: KeyboardEvent<HTMLInputElement>) => {
    if (e.key === "Enter" || (splitOnComma && e.key === ",")) {
      e.preventDefault();
      if (draft.trim()) add(draft);
    } else if (e.key === "Backspace" && draft === "" && value.length) {
      onChange(value.slice(0, -1));
    }
  };
  return (
    <div className="flex min-h-8 flex-wrap items-center gap-1 rounded-md border border-input bg-background px-1.5 py-1 focus-within:border-ring focus-within:ring-2 focus-within:ring-ring/20">
      {value.map((v, i) => (
        <span
          key={`${v}-${i}`}
          className="flex items-center gap-1 rounded bg-muted py-0.5 pr-1 pl-1.5 text-[12px]"
        >
          {v}
          <button
            type="button"
            aria-label={`Remove ${v}`}
            onClick={() => onChange(value.filter((_, j) => j !== i))}
            className="rounded text-muted-foreground hover:text-foreground"
          >
            <X className="size-3" />
          </button>
        </span>
      ))}
      <input
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onKeyDown={onKeyDown}
        onBlur={() => draft.trim() && add(draft)}
        placeholder={value.length ? "" : placeholder}
        className="h-6 min-w-24 flex-1 bg-transparent px-1 text-[13px] outline-none placeholder:text-muted-foreground"
      />
    </div>
  );
}

const text = (v: string | null) => v ?? "";
const orNull = (v: string) => (v.trim() === "" ? null : v);

/** Fields typed as numbers, kept as typed ("1." on the way to "1.5"). */
const NUMBER_FIELDS = [
  { key: "seriesNumber", label: "Series number", whole: false },
  { key: "year", label: "Year", whole: true },
  { key: "pages", label: "Pages", whole: true },
] as const;
type NumberKey = (typeof NUMBER_FIELDS)[number]["key"];
/** Chip fields, whose typed text is added when saving. */
const LIST_FIELDS = {
  authors: false,
  contributors: false,
  tags: true,
  categories: true,
} as const;
type ListKey = keyof typeof LIST_FIELDS;

/** Every editable field of a book. Saving is validated in Rust. */
export function MetadataForm({
  book,
  saving,
  error,
  onSave,
  onCancel,
}: {
  book: BookView;
  saving: boolean;
  error: string | null;
  onSave: (m: Metadata) => void;
  onCancel: () => void;
}) {
  const [m, setM] = useState<Metadata>(book.metadata);
  const [numbers, setNumbers] = useState<Record<NumberKey, string>>(() => ({
    seriesNumber: String(book.metadata.seriesNumber ?? ""),
    year: String(book.metadata.year ?? ""),
    pages: String(book.metadata.pages ?? ""),
  }));
  const [problem, setProblem] = useState<string | null>(null);
  /** Text typed in chip fields and not added yet. */
  const [drafts, setDrafts] = useState<Partial<Record<ListKey, string>>>({});
  const set = <K extends keyof Metadata>(key: K, value: Metadata[K]) =>
    setM((p) => ({ ...p, [key]: value }));
  const draftFor = (key: ListKey) => (t: string) => setDrafts((d) => ({ ...d, [key]: t }));

  /** What would be saved now (with text typed in chip fields), or what is wrong. */
  const collect = (): { next: Metadata } | { problem: string } => {
    const next = { ...m };
    for (const { key, label, whole } of NUMBER_FIELDS) {
      const n = parseNumberField(numbers[key]);
      if (n === undefined || (whole && n !== null && !Number.isInteger(n))) {
        return { problem: `${label} must be a ${whole ? "whole " : ""}number.` };
      }
      next[key] = n;
    }
    for (const key of Object.keys(LIST_FIELDS) as ListKey[]) {
      const typed = drafts[key];
      if (typed?.trim()) next[key] = addListItems(next[key], typed, LIST_FIELDS[key]);
    }
    return { next };
  };
  const save = () => {
    const r = collect();
    setProblem("problem" in r ? r.problem : null);
    if ("next" in r) onSave(r.next);
  };
  const cancel = async () => {
    const r = collect();
    const dirty = !("next" in r) || JSON.stringify(r.next) !== JSON.stringify(book.metadata);
    if (
      dirty &&
      !(await ask("The changes you made to these details are lost.", {
        title: "Discard your changes?",
        kind: "warning",
        okLabel: "Discard",
        cancelLabel: "Keep editing",
      }))
    ) {
      return;
    }
    onCancel();
  };
  useShortcut("details.save", save);

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        save();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          void cancel();
        }
      }}
    >
      {(problem ?? error) && (
        <p
          role="alert"
          className="rounded-md border border-destructive/40 bg-destructive/10 px-2.5 py-2 text-destructive"
        >
          {problem ?? error}
        </p>
      )}
      <Field label="Title">
        <Input autoFocus value={m.title} onChange={(e) => set("title", e.target.value)} />
      </Field>
      <Field label="Subtitle">
        <Input value={text(m.subtitle)} onChange={(e) => set("subtitle", orNull(e.target.value))} />
      </Field>
      <Field label="Authors">
        <ListInput
          value={m.authors}
          onChange={(v) => set("authors", v)}
          onDraft={draftFor("authors")}
          placeholder="Type a name, press Enter"
          splitOnComma={false}
        />
      </Field>
      <Field label="Editors, translators…">
        <ListInput
          value={m.contributors}
          onChange={(v) => set("contributors", v)}
          onDraft={draftFor("contributors")}
          placeholder="e.g. Jane Smith (translator)"
          splitOnComma={false}
        />
      </Field>
      <Field label="Content type">
        <NativeSelect
          value={m.contentType}
          onChange={(e) => set("contentType", e.target.value as ContentType)}
        >
          {Object.entries(CONTENT_TYPE_LABEL).map(([value, label]) => (
            <option key={value} value={value}>
              {label}
            </option>
          ))}
        </NativeSelect>
      </Field>
      <Field label="Tags">
        <ListInput
          value={m.tags}
          onChange={(v) => set("tags", v)}
          onDraft={draftFor("tags")}
          placeholder="e.g. physics, quantum"
        />
      </Field>
      <Field label="Categories">
        <ListInput
          value={m.categories}
          onChange={(v) => set("categories", v)}
          onDraft={draftFor("categories")}
          placeholder="e.g. Science/Physics"
        />
      </Field>
      <div className="grid grid-cols-[1fr_72px] gap-2">
        <Field label="Series">
          <Input value={text(m.series)} onChange={(e) => set("series", orNull(e.target.value))} />
        </Field>
        <Field label="No.">
          <Input
            inputMode="decimal"
            value={numbers.seriesNumber}
            onChange={(e) => setNumbers((p) => ({ ...p, seriesNumber: e.target.value }))}
          />
        </Field>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <Field label="Year">
          <Input
            inputMode="numeric"
            value={numbers.year}
            onChange={(e) => setNumbers((p) => ({ ...p, year: e.target.value }))}
          />
        </Field>
        <Field label="Pages">
          <Input
            inputMode="numeric"
            value={numbers.pages}
            onChange={(e) => setNumbers((p) => ({ ...p, pages: e.target.value }))}
          />
        </Field>
        <Field label="Publisher" className="col-span-2">
          <Input
            value={text(m.publisher)}
            onChange={(e) => set("publisher", orNull(e.target.value))}
          />
        </Field>
        <Field label="Edition">
          <Input value={text(m.edition)} onChange={(e) => set("edition", orNull(e.target.value))} />
        </Field>
        <Field label="Language">
          <Input
            value={text(m.language)}
            placeholder="e.g. en"
            onChange={(e) => set("language", orNull(e.target.value))}
          />
        </Field>
        <Field label="ISBN-13">
          <Input
            className="font-mono text-[12px]"
            value={text(m.isbn13)}
            onChange={(e) => set("isbn13", orNull(e.target.value))}
          />
        </Field>
        <Field label="ISBN-10">
          <Input
            className="font-mono text-[12px]"
            value={text(m.isbn10)}
            onChange={(e) => set("isbn10", orNull(e.target.value))}
          />
        </Field>
        <Field label="DOI">
          <Input
            className="font-mono text-[12px]"
            value={text(m.doi)}
            onChange={(e) => set("doi", orNull(e.target.value))}
          />
        </Field>
        <Field label="arXiv ID">
          <Input
            className="font-mono text-[12px]"
            value={text(m.arxivId)}
            onChange={(e) => set("arxivId", orNull(e.target.value))}
          />
        </Field>
        <Field label="Journal / conference" className="col-span-2">
          <Input value={text(m.journal)} onChange={(e) => set("journal", orNull(e.target.value))} />
        </Field>
        <Field label="Volume">
          <Input value={text(m.volume)} onChange={(e) => set("volume", orNull(e.target.value))} />
        </Field>
        <Field label="Issue">
          <Input value={text(m.issue)} onChange={(e) => set("issue", orNull(e.target.value))} />
        </Field>
        <Field label="Web page" className="col-span-2">
          <Input value={text(m.url)} onChange={(e) => set("url", orNull(e.target.value))} />
        </Field>
      </div>
      <Field label="About">
        <Textarea
          rows={6}
          value={text(m.about)}
          onChange={(e) => set("about", orNull(e.target.value))}
        />
      </Field>
      <div className="sticky bottom-0 -mx-4 flex items-center gap-2 border-t bg-sidebar px-4 py-3">
        <Button type="submit" disabled={saving}>
          {saving ? "Saving…" : "Save"}
        </Button>
        <Button type="button" variant="ghost" onClick={onCancel}>
          Cancel
        </Button>
        <Kbd className="ml-auto" action="details.save" />
      </div>
    </form>
  );
}
