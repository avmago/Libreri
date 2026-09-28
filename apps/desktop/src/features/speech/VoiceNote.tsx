import { Check, Loader2, Mic, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { bookUrl } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { clock, type RecordedVoice } from "./voice";
import type { VoiceNoteRecording } from "./useVoiceNote";

/** The controls while a voice note is being recorded and saved. */
export function VoiceNoteBar({
  v,
  onDone,
  className,
}: {
  v: VoiceNoteRecording;
  onDone: (r: RecordedVoice) => void;
  className?: string;
}) {
  if (v.phase === "idle") return null;
  const busy = v.phase !== "recording";
  return (
    <div
      role="group"
      aria-label="Voice note"
      className={cn("flex items-center gap-2 text-[12.5px]", className)}
    >
      {busy ? (
        <>
          <Loader2 className="size-4 animate-spin text-muted-foreground" aria-hidden />
          <span className="flex-1 text-muted-foreground" aria-live="polite">
            {v.phase === "saving" ? "Saving…" : "Writing down what you said…"}
          </span>
        </>
      ) : (
        <>
          <span className="size-2.5 animate-pulse rounded-full bg-red-500" aria-hidden />
          <span className="tabular-nums" aria-live="off">
            {clock(v.seconds)}
          </span>
          <span className="h-1.5 w-16 overflow-hidden rounded-full bg-muted" aria-hidden>
            <span
              className="block h-full bg-red-500 transition-[width] duration-75"
              style={{ width: `${Math.round(v.level * 100)}%` }}
            />
          </span>
          <span className="flex-1 text-muted-foreground">
            {v.full ? "That's the longest a note can be" : "Recording"}
          </span>
          <Button
            size="sm"
            onClick={() => void v.finish().then((r) => r && onDone(r))}
            aria-label="Stop and save the voice note"
          >
            <Check /> Done
          </Button>
          <Button variant="ghost" size="icon" aria-label="Cancel the voice note" onClick={v.cancel}>
            <X />
          </Button>
        </>
      )}
    </div>
  );
}

/** A button that starts a voice note (pass the same `v` to a VoiceNoteBar). */
export function VoiceNoteButton({
  v,
  label = "Record a voice note",
  className,
}: {
  v: VoiceNoteRecording;
  label?: string;
  className?: string;
}) {
  return (
    <Button
      variant="ghost"
      size="icon"
      aria-label={label}
      title={label}
      disabled={v.phase !== "idle"}
      onClick={() => void v.start()}
      className={className}
    >
      <Mic />
    </Button>
  );
}

/** Plays a voice note from the library. */
export function VoicePlayer({ path, className }: { path: string; className?: string }) {
  return (
    <audio
      controls
      preload="metadata"
      src={bookUrl(path)}
      className={cn("h-8 w-full min-w-48", className)}
      aria-label="Voice note"
    />
  );
}
