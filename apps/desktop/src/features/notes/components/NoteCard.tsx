import { useState } from "react";
import { ArrowUpRight, Bookmark, Copy, MessageSquareText, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import type { HighlightColor, NoteDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { HIGHLIGHT_COLORS } from "@/readers";
import { relativeDate } from "../model";

const SWATCH: Record<HighlightColor, string> = {
  yellow: "#facc15",
  green: "#4ade80",
  blue: "#60a5fa",
  pink: "#f472b6",
};

/** One highlight, comment or bookmark in the Notes hub. */
export function NoteCard({
  note,
  showBook,
  onOpen,
  onChange,
  onDelete,
}: {
  note: NoteDto;
  showBook: boolean;
  onOpen: () => void;
  onChange: (a: NoteDto["annotation"]) => void;
  onDelete: () => void;
}) {
  const a = note.annotation;
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState(a.note ?? "");
  const bookmark = a.kind === "bookmark";

  return (
    <article
      className="group relative flex gap-3 rounded-lg border bg-background p-3.5 pl-4"
      aria-label={bookmark ? `Bookmark, ${a.label ?? ""}` : `Highlight: ${a.quote?.exact ?? ""}`}
    >
      <span
        aria-hidden
        className="absolute inset-y-3 left-0 w-1 rounded-r"
        style={{ background: bookmark ? "var(--muted-foreground)" : SWATCH[a.color ?? "yellow"] }}
      />
      <div className="flex min-w-0 flex-1 flex-col gap-2">
        {bookmark ? (
          <p className="flex items-center gap-2 font-medium">
            <Bookmark className="size-4 text-muted-foreground" aria-hidden /> Bookmark
          </p>
        ) : (
          <blockquote className="line-clamp-6 text-[13.5px] leading-relaxed whitespace-pre-line">
            {a.quote?.exact}
          </blockquote>
        )}
        {editing ? (
          <form
            className="flex flex-col gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              onChange({ ...a, note: draft.trim() || null });
              setEditing(false);
            }}
          >
            <Textarea
              autoFocus
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Escape") setEditing(false);
                if (e.key === "Enter" && (e.metaKey || e.ctrlKey))
                  e.currentTarget.form?.requestSubmit();
              }}
              aria-label="Comment"
              placeholder="Your comment…"
            />
            <div className="flex justify-end gap-2">
              <Button type="button" size="sm" variant="ghost" onClick={() => setEditing(false)}>
                Cancel
              </Button>
              <Button type="submit" size="sm">
                Save
              </Button>
            </div>
          </form>
        ) : (
          a.note && (
            <p className="flex gap-2 rounded-md bg-muted px-2.5 py-2 text-[13px] leading-relaxed whitespace-pre-line">
              <MessageSquareText
                className="mt-0.5 size-3.5 shrink-0 text-muted-foreground"
                aria-hidden
              />
              {a.note}
            </p>
          )
        )}
        <footer className="flex flex-wrap items-center gap-x-2 text-[11.5px] text-muted-foreground">
          {showBook && (
            <span className="max-w-72 truncate font-medium text-foreground/80">
              {note.bookTitle}
            </span>
          )}
          {showBook && a.label && <span aria-hidden>·</span>}
          {a.label && <span>{a.label}</span>}
          <span aria-hidden>·</span>
          <span>{relativeDate(a.modifiedAt || a.createdAt)}</span>
        </footer>
      </div>
      <div className="flex shrink-0 flex-col items-end gap-1.5 opacity-60 group-focus-within:opacity-100 group-hover:opacity-100">
        <div className="flex">
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label="Open at the page"
            title="Open at the page"
            onClick={onOpen}
          >
            <ArrowUpRight />
          </Button>
          {!bookmark && (
            <>
              <Button
                variant="ghost"
                size="icon"
                className="size-7"
                aria-label={a.note ? "Edit comment" : "Add a comment"}
                title={a.note ? "Edit comment" : "Add a comment"}
                onClick={() => {
                  setDraft(a.note ?? "");
                  setEditing(true);
                }}
              >
                <MessageSquareText />
              </Button>
              <Button
                variant="ghost"
                size="icon"
                className="size-7"
                aria-label="Copy the quote"
                title="Copy the quote"
                onClick={() => void navigator.clipboard.writeText(a.quote?.exact ?? "")}
              >
                <Copy />
              </Button>
            </>
          )}
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label="Delete"
            title="Delete"
            onClick={onDelete}
          >
            <Trash2 />
          </Button>
        </div>
        {!bookmark && (
          <div className="flex gap-1 pr-1.5" role="radiogroup" aria-label="Colour">
            {HIGHLIGHT_COLORS.map((c) => (
              <button
                key={c}
                type="button"
                role="radio"
                aria-checked={(a.color ?? "yellow") === c}
                aria-label={c}
                onClick={() => onChange({ ...a, color: c })}
                className={cn(
                  "size-3.5 rounded-full border border-black/10",
                  (a.color ?? "yellow") === c &&
                    "ring-2 ring-ring ring-offset-1 ring-offset-background",
                )}
                style={{ background: SWATCH[c] }}
              />
            ))}
          </div>
        )}
      </div>
    </article>
  );
}
