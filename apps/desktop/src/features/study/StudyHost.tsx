import { useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Textarea } from "@/components/ui/input";
import { commands, events, unwrap } from "@/lib/ipc";
import { useStudy } from "./store";
import { timerReading, useTimer } from "./timer";
import { timerLabel } from "./label";

/**
 * Keeps the study timer going for the whole window: ticks it, shows it in
 * the menu bar / taskbar / tray (with Pause, Skip and Stop there), and the
 * card at each break.
 */
export function StudyHost({ keepsData }: { keepsData: boolean }) {
  const status = useTimer((s) => s.status);
  const card = useTimer((s) => s.card);

  // The calendar of whoever is signed in.
  useEffect(() => {
    if (keepsData) void useStudy.getState().load();
  }, [keepsData]);

  // Tick, and mirror into the system's menu bar or tray.
  const shown = useRef(false);
  useEffect(() => {
    const push = () => {
      const s = useTimer.getState();
      s.tick();
      const t = useTimer.getState();
      if (t.status === "idle") {
        if (shown.current) void commands.timerTray(null);
        shown.current = false;
        return;
      }
      const now = Date.now();
      const { progress } = timerReading(t, now);
      const label = timerLabel(t, now);
      const f = useStudy.getState().data.focus;
      const detail = [
        t.mode === "focus"
          ? `${label.name} · round ${((t.round - 1) % f.rounds) + 1} of ${f.rounds}`
          : label.name,
        t.context?.title,
        t.status === "paused" ? "paused" : null,
      ]
        .filter(Boolean)
        .join(" · ");
      shown.current = true;
      void commands.timerTray({
        text: label.time,
        detail,
        progress,
        paused: t.status === "paused",
        canSkip: t.mode === "focus",
      });
    };
    push();
    if (status === "idle") return;
    const id = setInterval(push, 1000);
    return () => clearInterval(id);
  }, [status]);

  // Leaving (or the window closing) takes the clock out of the menu bar.
  useEffect(
    () => () => {
      void commands.timerTray(null);
    },
    [],
  );

  // Choices from the menu bar or tray.
  useEffect(() => {
    const off = events.timerTrayAction.listen(({ payload }) => {
      const t = useTimer.getState();
      if (payload.action === "pause") (t.status === "running" ? t.pause : t.resume)();
      else if (payload.action === "skip") t.skip();
      else if (payload.action === "stop") t.stop();
    });
    return () => void off.then((f) => f());
  }, []);

  return card ? <BreakCardView /> : null;
}

function BreakCardView() {
  const card = useTimer((s) => s.card)!;
  const ask = useStudy((s) => s.data.options.askTakeaway);
  const lengths = useStudy((s) => s.data.focus);
  const [text, setText] = useState("");
  const pages =
    card.pageFrom != null && card.pageTo != null && card.pageTo > card.pageFrom
      ? `pages ${card.pageFrom}–${card.pageTo}`
      : null;
  const breakMin = card.next === "long" ? lengths.long : lengths.brk;

  const saveNote = async () => {
    const note = text.trim();
    if (!note || !card.bookId) return;
    try {
      const nb = await unwrap(commands.getNotebook(card.bookId));
      const day = new Date().toLocaleDateString(undefined, {
        day: "numeric",
        month: "short",
        year: "numeric",
      });
      const head = `## Takeaway, ${day}${pages ? `, ${pages}` : ""}`;
      const content = `${nb.content.trimEnd()}${nb.content.trim() ? "\n\n" : ""}${head}\n\n${note}\n`;
      await unwrap(commands.saveNotebook(card.bookId, content));
      toast("Saved to the book's notebook");
    } catch (e) {
      toast.error("The note could not be saved", {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const go = (start: boolean) => {
    void saveNote();
    const t = useTimer.getState();
    if (start) t.startBreak();
    else {
      // No break: straight back to reading.
      t.closeCard();
      t.skip();
    }
  };

  return (
    <div
      role="dialog"
      aria-label="Break time"
      className="fixed bottom-6 left-6 z-50 flex w-[340px] max-w-[calc(100vw-48px)] flex-col gap-2 rounded-xl border bg-popover p-4 text-popover-foreground shadow-2xl"
    >
      <div className="flex items-start gap-2">
        <div className="flex flex-1 flex-col">
          <span className="font-semibold">
            {card.next === "long" ? "Long break" : "Break time"} · {card.minutes} min read
          </span>
          {(card.title || pages) && (
            <span className="text-[12px] text-muted-foreground">
              {[card.title, pages].filter(Boolean).join(" · ")}
            </span>
          )}
        </div>
        <Button
          variant="ghost"
          size="icon"
          className="size-7"
          aria-label="Close"
          onClick={() => useTimer.getState().closeCard()}
        >
          <X />
        </Button>
      </div>
      {ask && card.bookId && (
        <Textarea
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="What did you take from this? (saved to the book's notebook)"
          className="min-h-16 text-[13px]"
        />
      )}
      <div className="flex justify-end gap-2">
        <Button variant="outline" size="sm" onClick={() => go(false)}>
          Skip break
        </Button>
        <Button size="sm" onClick={() => go(true)}>
          Start {breakMin}-minute break
        </Button>
      </div>
    </div>
  );
}
