import { useRef, useState, type RefObject } from "react";
import { Loader2, Speech, Square } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { transcribe, useSpeechSettings } from "./api";
import { SYSTEM_DICTATION, openSpeechSettings } from "./opener";
import { insertAtCursor, type Field } from "./insert";
import { useRecorder } from "./useRecorder";

/**
 * Dictation with the speech model built into Libreri: while on, what is
 * said is written at the cursor at each pause. The system's own dictation
 * works in every text box too; this button is for when it is not there or
 * not wanted.
 */
export function DictateButton({
  target,
  lang,
  className,
}: {
  target: RefObject<Field | null>;
  lang?: string | null;
  className?: string;
}) {
  const { data: settings } = useSpeechSettings();
  const [pending, setPending] = useState(0);
  const queue = useRef<Promise<void>>(Promise.resolve());

  // Parts are written down one after another, so they arrive in order.
  const send = (pcm: Int16Array) => {
    setPending((n) => n + 1);
    queue.current = queue.current
      .then(() => transcribe(pcm, lang))
      .then((text) => insertAtCursor(target.current, text))
      .catch((e: unknown) => {
        toast.error("Could not write down what was said", { description: String(e) });
      })
      .finally(() => setPending((n) => n - 1));
  };
  const rec = useRecorder({ onPart: send, maxSeconds: 1800, onFull: () => stop() });
  const on = rec.state !== "idle";

  const stop = () => {
    const rest = rec.stop();
    if (rest) send(rest);
  };
  const toggle = async () => {
    if (on) return stop();
    if (!settings?.model) {
      toast("Dictating needs a speech model", {
        description: SYSTEM_DICTATION
          ? `Download one in Settings › Speech, or use ${SYSTEM_DICTATION}.`
          : "Download one in Settings › Speech.",
        action: { label: "Settings", onClick: openSpeechSettings },
      });
      return;
    }
    target.current?.focus();
    const error = await rec.start();
    if (error) toast.error(error);
  };

  const title = on
    ? "Stop dictating"
    : `Dictate${SYSTEM_DICTATION ? ` (or use ${SYSTEM_DICTATION})` : ""}`;
  return (
    <Button
      type="button"
      variant="ghost"
      size="icon"
      aria-label={on ? "Stop dictating" : "Dictate"}
      aria-pressed={on}
      title={title}
      // Keep the cursor where it is in the text box.
      onMouseDown={(e) => e.preventDefault()}
      onClick={() => void toggle()}
      className={cn("relative", on && "text-red-600 dark:text-red-400", className)}
    >
      {on ? (
        <Square className="!size-3 fill-current" />
      ) : pending ? (
        <Loader2 className="animate-spin" />
      ) : (
        <Speech />
      )}
      {on && (
        <span
          aria-hidden
          className="pointer-events-none absolute inset-0.5 rounded-md border-2 border-red-500/60 transition-transform"
          style={{ transform: `scale(${1 + rec.level * 0.25})` }}
        />
      )}
      {on && pending > 0 && (
        <span className="absolute -top-0.5 -right-0.5 size-2 animate-pulse rounded-full bg-primary" />
      )}
    </Button>
  );
}
