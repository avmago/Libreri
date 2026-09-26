import { useState, type KeyboardEvent, type ReactNode } from "react";
import { X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input, NativeSelect, Textarea } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import type { ContentType } from "@/lib/ipc";
import { DEFAULT_SHORTCUTS, useShortcut } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";
import { CONTENT_TYPE_LABEL, type BookView, type EMPTY_METADATA } from "../model";

type Metadata = typeof EMPTY_METADATA;

function Field({
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
function ListInput({
  value,
  onChange,
  placeholder,
  splitOnComma = true,
}: {
  value: string[];
  onChange: (v: string[]) => void;
  placeholder: string;
  splitOnComma?: boolean;
}) {
  const [draft, setDraft] = useState("");
  const add = (text: string) => {
    const items = text
      .split(splitOnComma ? /[,;\n]/ : /[;\n]/)
      .map((s) => s.trim())
      .filter(Boolean);
    if (items.length) onChange([...value, ...items.filter((i) => !value.includes(i))]);
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
const numOrNull = (v: string) => {
  const n = Number(v);
  return v.trim() === "" || Number.isNaN(n) ? null : n;
};

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
  const set = <K extends keyof Metadata>(key: K, value: Metadata[K]) =>
    setM((p) => ({ ...p, [key]: value }));
  useShortcut("details.save", () => onSave(m));

  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        onSave(m);
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          onCancel();
        }
      }}
    >
      {error && (
        <p
          role="alert"
          className="rounded-md border border-destructive/40 bg-destructive/10 px-2.5 py-2 text-destructive"
        >
          {error}
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
          placeholder="Type a name, press Enter"
          splitOnComma={false}
        />
      </Field>
      <Field label="Editors, translators…">
        <ListInput
          value={m.contributors}
          onChange={(v) => set("contributors", v)}
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
          placeholder="e.g. physics, quantum"
        />
      </Field>
      <Field label="Categories">
        <ListInput
          value={m.categories}
          onChange={(v) => set("categories", v)}
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
            value={m.seriesNumber ?? ""}
            onChange={(e) => set("seriesNumber", numOrNull(e.target.value))}
          />
        </Field>
      </div>
      <div className="grid grid-cols-2 gap-2">
        <Field label="Year">
          <Input
            inputMode="numeric"
            value={m.year ?? ""}
            onChange={(e) => set("year", numOrNull(e.target.value))}
          />
        </Field>
        <Field label="Pages">
          <Input
            inputMode="numeric"
            value={m.pages ?? ""}
            onChange={(e) => set("pages", numOrNull(e.target.value))}
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
        <Kbd className="ml-auto" shortcut={DEFAULT_SHORTCUTS["details.save"]} />
      </div>
    </form>
  );
}
