import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { Copy, MessageSquarePlus, NotebookPen, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import type { Annotation, HighlightColor } from "@/lib/ipc";
import { HIGHLIGHT_COLORS, highlightFill } from "@/readers";
import { cn } from "@/lib/utils";

const COLOR_NAME: Record<HighlightColor, string> = {
  yellow: "Yellow",
  green: "Green",
  blue: "Blue",
  pink: "Pink",
};

/** A small card next to a rectangle on screen, kept inside the window. */
function Floating({
  rect,
  children,
  onClose,
}: {
  rect: DOMRect;
  children: ReactNode;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ left: rect.left, top: rect.bottom + 8 });
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const w = el.offsetWidth;
    const h = el.offsetHeight;
    let left = rect.left + rect.width / 2 - w / 2;
    left = Math.max(8, Math.min(left, window.innerWidth - w - 8));
    let top = rect.bottom + 8;
    if (top + h > window.innerHeight - 8) top = Math.max(8, rect.top - h - 8);
    setPos({ left, top });
  }, [rect]);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    const onDown = (e: PointerEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    window.addEventListener("keydown", onKey);
    // Wait a tick so the click that opened us does not close us.
    const t = setTimeout(() => window.addEventListener("pointerdown", onDown), 0);
    return () => {
      clearTimeout(t);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("pointerdown", onDown);
    };
  }, [onClose]);
  return (
    <div
      ref={ref}
      role="dialog"
      aria-label="Highlight"
      className="fixed z-50 flex flex-col gap-2 rounded-lg border bg-popover p-1.5 text-popover-foreground shadow-lg"
      style={pos}
    >
      {children}
    </div>
  );
}

function Swatches({
  value,
  onPick,
}: {
  value?: HighlightColor | null;
  onPick: (c: HighlightColor) => void;
}) {
  return (
    <div className="flex items-center gap-1" role="radiogroup" aria-label="Highlight colour">
      {HIGHLIGHT_COLORS.map((c, i) => (
        <button
          key={c}
          type="button"
          role="radio"
          aria-checked={value === c}
          aria-label={`${COLOR_NAME[c]} (${i + 1})`}
          title={`${COLOR_NAME[c]} (${i + 1})`}
          onClick={() => onPick(c)}
          className={cn(
            "size-7 rounded-md border border-black/10 hover:scale-105",
            value === c && "ring-2 ring-ring ring-offset-1 ring-offset-popover",
          )}
          style={{ background: highlightFill(c, false).replace(/[\d.]+\)$/, "0.75)") }}
        />
      ))}
    </div>
  );
}

/** Shown after selecting text: highlight in a colour, comment, notebook, copy. */
export function SelectionMenu({
  rect,
  onHighlight,
  onComment,
  onNotebook,
  onCopy,
  onClose,
}: {
  rect: DOMRect;
  onHighlight: (c: HighlightColor) => void;
  onComment: () => void;
  onNotebook: () => void;
  onCopy: () => void;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      const n = Number(e.key);
      if (n >= 1 && n <= 4 && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        onHighlight(HIGHLIGHT_COLORS[n - 1]!);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onHighlight]);
  return (
    <Floating rect={rect} onClose={onClose}>
      <div className="flex items-center gap-1">
        <Swatches onPick={onHighlight} />
        <div className="mx-0.5 h-6 w-px bg-border" />
        <Button
          variant="ghost"
          size="icon"
          aria-label="Highlight and comment"
          title="Comment"
          onClick={onComment}
        >
          <MessageSquarePlus />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Add to notebook"
          title="Add to notebook"
          onClick={onNotebook}
        >
          <NotebookPen />
        </Button>
        <Button variant="ghost" size="icon" aria-label="Copy" title="Copy" onClick={onCopy}>
          <Copy />
        </Button>
      </div>
    </Floating>
  );
}

/** Shown when clicking a highlight: colour, comment, notebook, delete. */
export function AnnotationMenu({
  annotation,
  rect,
  startEditing,
  onChange,
  onNotebook,
  onDelete,
  onClose,
}: {
  annotation: Annotation;
  rect: DOMRect;
  startEditing: boolean;
  onChange: (a: Annotation) => void;
  onNotebook: (a: Annotation) => void;
  onDelete: (a: Annotation) => void;
  onClose: () => void;
}) {
  const [note, setNote] = useState(annotation.note ?? "");
  const [editing, setEditing] = useState(startEditing || Boolean(annotation.note));
  const commit = () => {
    if ((annotation.note ?? "") !== note.trim())
      onChange({ ...annotation, note: note.trim() || null });
  };
  return (
    <Floating
      rect={rect}
      onClose={() => {
        commit();
        onClose();
      }}
    >
      <div className="flex items-center gap-1">
        <Swatches value={annotation.color} onPick={(color) => onChange({ ...annotation, color })} />
        <div className="mx-0.5 h-6 w-px bg-border" />
        <Button
          variant="ghost"
          size="icon"
          aria-label="Comment"
          aria-pressed={editing}
          onClick={() => setEditing(true)}
        >
          <MessageSquarePlus />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Add to notebook"
          onClick={() => onNotebook(annotation)}
        >
          <NotebookPen />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Delete highlight"
          className="text-destructive"
          onClick={() => onDelete(annotation)}
        >
          <Trash2 />
        </Button>
      </div>
      {editing && (
        <div className="flex w-64 flex-col gap-1.5 px-0.5 pb-0.5">
          <Textarea
            autoFocus={startEditing || !annotation.note}
            rows={3}
            placeholder="Write a comment…"
            value={note}
            onChange={(e) => setNote(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
                commit();
                onClose();
              }
            }}
            className="min-h-16 text-[13px]"
          />
          <div className="flex justify-end">
            <Button
              size="sm"
              onClick={() => {
                commit();
                onClose();
              }}
            >
              Save
            </Button>
          </div>
        </div>
      )}
    </Floating>
  );
}
