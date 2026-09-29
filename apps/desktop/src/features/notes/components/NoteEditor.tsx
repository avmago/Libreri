import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { BookOpen, Eye, Pencil } from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  DictateButton,
  VoiceNoteBar,
  VoiceNoteButton,
  attachVoicePlayers,
  useVoiceNote,
  voiceMarkdown,
  type RecordedVoice,
} from "@/features/speech";
import { useSpell } from "@/features/spell";
import { renderMarkdown } from "@/lib/markdown";
import { cn } from "@/lib/utils";
import { useNoteFile, useWriteNote } from "../api";
import { registerNoteFlush } from "../flush";

/** A Markdown note from the profile's Notes folder, edited in place. */
export function NoteEditor({
  relPath,
  title,
  bookTitle,
  onOpenBook,
  onLink,
}: {
  relPath: string;
  title: string;
  bookTitle?: string | null;
  onOpenBook?: () => void;
  onLink: (href: string) => void;
}) {
  const { data: content, isPending, error } = useNoteFile(relPath);
  const write = useWriteNote();
  const [text, setText] = useState<string | null>(null);
  const [mode, setMode] = useState<"preview" | "edit">("preview");
  const [status, setStatus] = useState<"saved" | "saving" | "failed">("saved");
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  /** Text typed and not written yet. */
  const unsaved = useRef<string | null>(null);
  /** Writes run one after another, so an older one never lands last. */
  const writing = useRef<Promise<void>>(Promise.resolve());
  const value = text ?? content ?? "";
  const { mutateAsync } = write;

  /** Writes what is waiting now (a save soon after typing, or before leaving). */
  const flush = useCallback(() => {
    clearTimeout(timer.current);
    const run = writing.current.then(async () => {
      const next = unsaved.current;
      if (next === null) return;
      unsaved.current = null;
      setStatus("saving");
      try {
        await mutateAsync({ relPath, content: next });
        if (unsaved.current === null) setStatus("saved");
      } catch {
        // Kept to try again (newer typing wins).
        unsaved.current ??= next;
        setStatus("failed");
      }
    });
    writing.current = run;
    return run;
  }, [mutateAsync, relPath]);

  const change = (next: string) => {
    setText(next);
    unsaved.current = next;
    setStatus("saving");
    clearTimeout(timer.current);
    timer.current = setTimeout(() => void flush(), 700);
  };

  // Written when the note closes, the window closes and before the profile
  // locks or switches (see `flushNotes`).
  useEffect(() => {
    const off = registerNoteFlush(flush);
    const onLeave = () => void flush();
    window.addEventListener("beforeunload", onLeave);
    window.addEventListener("pagehide", onLeave);
    return () => {
      off();
      window.removeEventListener("beforeunload", onLeave);
      window.removeEventListener("pagehide", onLeave);
      void flush();
    };
  }, [flush]);

  const html = useMemo(() => (mode === "preview" ? renderMarkdown(value) : ""), [mode, value]);
  const preview = useRef<HTMLDivElement>(null);
  const editor = useRef<HTMLTextAreaElement>(null);
  // Links to recordings play in place.
  useEffect(() => attachVoicePlayers(preview.current, relPath), [html, relPath]);
  useSpell(editor, {}, mode === "edit" && content !== undefined);
  const voice = useVoiceNote();
  const addVoice = (r: RecordedVoice) =>
    change(`${value.replace(/\s*$/, "")}\n\n${voiceMarkdown(relPath, r)}\n`);
  const onClick = (e: MouseEvent) => {
    const a = (e.target as HTMLElement).closest("a[href]");
    if (!a) return;
    e.preventDefault();
    onLink(a.getAttribute("href") ?? "");
  };

  return (
    <section aria-label={title} className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-11 shrink-0 items-center gap-2 border-b px-4">
        <div className="flex min-w-0 flex-1 flex-col leading-tight">
          <span className="truncate font-medium">{title}</span>
          <span className="truncate font-mono text-[10.5px] text-muted-foreground">{relPath}</span>
        </div>
        {status === "failed" ? (
          <span className="flex items-center gap-1.5 text-[11px] text-destructive" role="alert">
            Not saved
            <Button variant="outline" size="sm" className="h-6" onClick={() => void flush()}>
              Try again
            </Button>
          </span>
        ) : (
          <span className="text-[11px] text-muted-foreground" aria-live="polite">
            {status === "saving" ? "Saving…" : "Saved"}
          </span>
        )}
        <VoiceNoteButton v={voice} label="Record a voice note in this note" />
        {mode === "edit" && <DictateButton target={editor} />}
        {bookTitle && onOpenBook && (
          <Button variant="outline" size="sm" onClick={onOpenBook} title={`Open ${bookTitle}`}>
            <BookOpen /> Open book
          </Button>
        )}
        <div className="flex rounded-md border p-0.5">
          <Button
            variant="ghost"
            size="icon"
            className={cn("size-7", mode === "preview" && "bg-muted")}
            aria-label="Preview"
            aria-pressed={mode === "preview"}
            onClick={() => setMode("preview")}
          >
            <Eye />
          </Button>
          <Button
            variant="ghost"
            size="icon"
            className={cn("size-7", mode === "edit" && "bg-muted")}
            aria-label="Edit Markdown"
            aria-pressed={mode === "edit"}
            onClick={() => setMode("edit")}
          >
            <Pencil />
          </Button>
        </div>
      </div>
      <VoiceNoteBar v={voice} onDone={addVoice} className="border-b px-4 py-1.5" />
      <div className="min-h-0 flex-1">
        {isPending ? (
          <p className="p-6 text-muted-foreground">Opening…</p>
        ) : error ? (
          <p className="p-6 text-destructive">{String(error)}</p>
        ) : mode === "edit" ? (
          <textarea
            ref={editor}
            aria-label="Note (Markdown)"
            value={value}
            onChange={(e) => change(e.target.value)}
            autoFocus
            className="size-full resize-none bg-transparent px-6 py-5 font-mono text-[13px] leading-relaxed outline-none"
          />
        ) : (
          <div
            ref={preview}
            className="lb-doc lb-notebook size-full overflow-y-auto !max-w-3xl !px-6 !pt-5 !pb-16"
            onClick={onClick}
            onDoubleClick={() => setMode("edit")}
            // The user's own Markdown, rendered with raw HTML disabled.
            dangerouslySetInnerHTML={{ __html: html }}
          />
        )}
      </div>
    </section>
  );
}
