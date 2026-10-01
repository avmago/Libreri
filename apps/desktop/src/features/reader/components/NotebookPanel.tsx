import { ResizablePanel } from "@/components/ResizablePanel";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { Eye, NotebookPen, Pencil, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DictateButton,
  VoiceNoteBar,
  VoiceNoteButton,
  attachVoicePlayers,
  useVoiceNote,
  voiceMarkdown,
} from "@/features/speech";
import { useSpell } from "@/features/spell";
import { renderMarkdown } from "@/lib/markdown";
import { cn } from "@/lib/utils";
import { useNotebook, useSaveNotebook } from "../api";

/**
 * The profile's Markdown notebook for this book (a real .md file in
 * Notes/<profile>/). Quotes added from highlights link back to the page.
 */
export function NotebookPanel({
  bookId,
  insert,
  onInserted,
  onLink,
  onClose,
  lang,
}: {
  bookId: string;
  /** The book's language, for writing down speech. */
  lang?: string | null;
  /** Text to append (from "Add to notebook"). */
  insert: string | null;
  onInserted: () => void;
  onLink: (href: string) => void;
  onClose: () => void;
}) {
  const { data: notebook, isPending, error } = useNotebook(bookId, true);
  const save = useSaveNotebook(bookId);
  const [text, setText] = useState<string | null>(null);
  const [mode, setMode] = useState<"edit" | "preview">("preview");
  const [dirty, setDirty] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const value = text ?? notebook?.content ?? "";

  const { mutate } = save;
  const scheduleSave = (next: string) => {
    setText(next);
    setDirty(true);
    clearTimeout(timer.current);
    timer.current = setTimeout(() => {
      mutate(next, { onSuccess: () => setDirty(false) });
    }, 700);
  };

  // Save straight away when the panel closes or the tab goes away.
  const latest = useRef({ value, dirty });
  useEffect(() => {
    latest.current = { value, dirty };
  });
  useEffect(
    () => () => {
      clearTimeout(timer.current);
      if (latest.current.dirty) mutate(latest.current.value);
    },
    [mutate],
  );

  // "Add to notebook" arrives as a prop from the reader; appending it is a
  // response to that event, so updating state here is intended.
  useEffect(() => {
    if (insert === null || !notebook) return;
    const base = (text ?? notebook.content).replace(/\s*$/, "");
    // eslint-disable-next-line react-hooks/set-state-in-effect
    scheduleSave(`${base}\n\n${insert}\n`);
    onInserted();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [insert, notebook]);

  const html = useMemo(() => (mode === "preview" ? renderMarkdown(value) : ""), [mode, value]);
  const preview = useRef<HTMLDivElement>(null);
  const editor = useRef<HTMLTextAreaElement>(null);
  const relPath = notebook?.relPath;
  // Links to recordings play in place.
  useEffect(() => {
    if (relPath) attachVoicePlayers(preview.current, relPath);
  }, [html, relPath]);

  useSpell(editor, { bookId }, mode === "edit" && !!notebook);
  const voice = useVoiceNote(lang);
  const addVoice = (r: Parameters<typeof voiceMarkdown>[1]) => {
    if (!relPath) return;
    const base = value.replace(/\s*$/, "");
    scheduleSave(`${base}\n\n${voiceMarkdown(relPath, r)}\n`);
  };

  const onPreviewClick = (e: MouseEvent) => {
    const a = (e.target as HTMLElement).closest("a[href]");
    if (!a) return;
    e.preventDefault();
    onLink(a.getAttribute("href") ?? "");
  };

  return (
    <ResizablePanel
      id="reader.notebook"
      initial={320}
      min={260}
      max={720}
      side="right"
      label="notebook"
    >
      <aside aria-label="Notebook" className="flex min-w-0 flex-1 flex-col border-l bg-sidebar">
        <div className="flex h-10 shrink-0 items-center gap-1 border-b pr-2 pl-3">
          <NotebookPen className="size-4 text-muted-foreground" aria-hidden />
          <span className="flex-1 truncate text-[12.5px] font-medium">Notebook</span>
          <span className="text-[11px] text-muted-foreground" aria-live="polite">
            {save.isPending || dirty ? "Saving…" : notebook ? "Saved" : ""}
          </span>
          {notebook && (
            <VoiceNoteButton
              v={voice}
              className="size-7"
              label="Record a voice note in the notebook"
            />
          )}
          {mode === "edit" && notebook && (
            <DictateButton target={editor} lang={lang} className="size-7" />
          )}
          <div className="ml-1 flex rounded-md border p-0.5">
            <Button
              variant="ghost"
              size="icon"
              className={cn("size-6", mode === "preview" && "bg-muted")}
              aria-label="Preview"
              aria-pressed={mode === "preview"}
              onClick={() => setMode("preview")}
            >
              <Eye className="!size-3.5" />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              className={cn("size-6", mode === "edit" && "bg-muted")}
              aria-label="Edit Markdown"
              aria-pressed={mode === "edit"}
              onClick={() => setMode("edit")}
            >
              <Pencil className="!size-3.5" />
            </Button>
          </div>
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label="Close notebook"
            onClick={onClose}
          >
            <X />
          </Button>
        </div>
        <VoiceNoteBar v={voice} onDone={addVoice} className="border-b px-3 py-1.5" />
        {notebook && (
          <p
            className="truncate border-b px-3 py-1.5 font-mono text-[10.5px] text-muted-foreground"
            title={notebook.relPath}
          >
            {notebook.relPath}
          </p>
        )}
        <div className="min-h-0 flex-1">
          {isPending ? (
            <p className="p-4 text-muted-foreground">Opening notebook…</p>
          ) : error ? (
            <p className="p-4 text-destructive">{String(error)}</p>
          ) : mode === "edit" ? (
            <textarea
              ref={editor}
              aria-label="Notebook (Markdown)"
              value={value}
              onChange={(e) => scheduleSave(e.target.value)}
              className="size-full resize-none bg-transparent px-3 py-3 font-mono text-[12.5px] leading-relaxed outline-none"
            />
          ) : (
            <div
              ref={preview}
              className="lb-doc lb-notebook size-full overflow-y-auto !px-4 !pt-3 !pb-10"
              onClick={onPreviewClick}
              onDoubleClick={() => setMode("edit")}
              // Rendered from the user's own Markdown with raw HTML disabled.
              dangerouslySetInnerHTML={{ __html: html }}
            />
          )}
        </div>
      </aside>
    </ResizablePanel>
  );
}
