import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { PHASE, timerLabel, useNow } from "./label";
import { Timer } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { clock } from "./model";
import { useStudy } from "./store";
import { timerReading, useTimer, type TimerMode } from "./timer";

/** The clock in the reader's toolbar, with the timer under it. */
export function TimerButton() {
  const t = useTimer();
  const now = useNow(t.status === "running");
  const ref = useRef<HTMLDivElement>(null);
  const panel = useRef<HTMLDivElement>(null);
  // Where the panel goes: under the button, kept on screen. It is drawn on
  // top of everything (a portal), so the page never covers it.
  const [pos, setPos] = useState<{ top: number; right: number } | null>(null);
  const toggle = (el: HTMLElement) => {
    if (open) return t.setOpen(false);
    const r = el.getBoundingClientRect();
    setPos({ top: r.bottom + 6, right: Math.max(8, window.innerWidth - r.right) });
    t.setOpen(true);
  };
  const { open, setOpen } = t;

  useEffect(() => {
    if (!open) return;
    const away = (e: PointerEvent) => {
      const n = e.target as Node;
      if (!ref.current?.contains(n) && !panel.current?.contains(n)) setOpen(false);
    };
    const esc = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    const resized = () => setOpen(false);
    document.addEventListener("pointerdown", away);
    document.addEventListener("keydown", esc);
    window.addEventListener("resize", resized);
    return () => {
      window.removeEventListener("resize", resized);
      document.removeEventListener("pointerdown", away);
      document.removeEventListener("keydown", esc);
    };
  }, [open, setOpen]);

  const { progress } = timerReading(t, now);
  const label = timerLabel(t, now);
  const brk = t.mode === "focus" && t.phase !== "work";

  return (
    <div ref={ref} className="relative">
      {t.status === "idle" ? (
        <Button
          variant="ghost"
          size="icon"
          aria-label="Study timer"
          title="Study timer: focus sessions, a timer and a stopwatch"
          aria-expanded={open}
          onClick={(e) => toggle(e.currentTarget)}
        >
          <Timer />
        </Button>
      ) : (
        <button
          type="button"
          aria-label={`Study timer: ${label.name} ${label.time}`}
          aria-expanded={open}
          onClick={(e) => toggle(e.currentTarget)}
          className={cn(
            "flex h-7 items-center gap-1.5 rounded-full border py-0.5 pr-2.5 pl-1 text-[12.5px] font-semibold tabular-nums",
            brk
              ? "border-sky-200 bg-sky-50 text-sky-800 dark:border-sky-900 dark:bg-sky-950 dark:text-sky-200"
              : "border-emerald-200 bg-emerald-50 text-emerald-800 dark:border-emerald-900 dark:bg-emerald-950 dark:text-emerald-200",
            t.status === "paused" && "opacity-70",
          )}
        >
          <Ring progress={progress} />
          <span className="@max-3xl:hidden">{label.name}</span>
          {label.time}
        </button>
      )}
      {open &&
        pos &&
        createPortal(
          <div ref={panel} className="fixed z-[60]" style={{ top: pos.top, right: pos.right }}>
            <TimerPanel now={now} />
          </div>,
          document.body,
        )}
    </div>
  );
}

function Ring({ progress }: { progress: number | null }) {
  if (progress === null) return <Timer className="size-4" aria-hidden />;
  return (
    <span
      aria-hidden
      className="size-[18px] rounded-full"
      style={{
        background: `conic-gradient(currentColor ${Math.round(progress * 100)}%, color-mix(in srgb, currentColor 20%, transparent) 0)`,
      }}
    />
  );
}

const MODES: [TimerMode, string][] = [
  ["focus", "Focus"],
  ["timer", "Timer"],
  ["stopwatch", "Stopwatch"],
];

function TimerPanel({ now }: { now: number }) {
  const t = useTimer();
  const study = useStudy((s) => s.data);
  const update = useStudy((s) => s.update);
  const [editing, setEditing] = useState(false);
  const label = timerLabel(t, now);
  const f = study.focus;
  const o = study.options;
  const idle = t.status === "idle";

  const big = idle
    ? t.mode === "focus"
      ? clock(f.work * 60_000)
      : t.mode === "timer"
        ? clock(o.timerMinutes * 60_000)
        : "0:00"
    : label.time;
  const line = idle
    ? t.mode === "focus"
      ? `${f.work} min of reading, then a ${f.brk}-minute break`
      : t.mode === "timer"
        ? "A plain countdown"
        : "Counts up until you stop it"
    : t.mode === "focus"
      ? t.phase === "work"
        ? `Round ${((t.round - 1) % f.rounds) + 1} of ${f.rounds} · then a ${t.round % f.rounds === 0 ? `${f.long}-minute long` : `${f.brk}-minute`} break`
        : `${PHASE[t.phase]} · round ${((t.round - 1) % f.rounds) + 1} of ${f.rounds} done`
      : t.status === "paused"
        ? "Paused"
        : t.context
          ? `Reading ${t.context.title}`
          : "Running";

  const setOption = (k: keyof typeof o, v: boolean) =>
    update((d) => ({ ...d, options: { ...d.options, [k]: v } }));

  return (
    <div
      role="dialog"
      aria-label="Study timer"
      className="flex w-80 max-w-[calc(100vw-16px)] flex-col gap-3 rounded-xl border bg-popover p-3.5 text-popover-foreground shadow-xl"
    >
      <div role="tablist" aria-label="Timer kind" className="flex rounded-lg bg-muted p-0.5">
        {MODES.map(([m, name]) => (
          <button
            key={m}
            type="button"
            role="tab"
            aria-selected={t.mode === m}
            onClick={() => t.setMode(m)}
            className={cn(
              "flex-1 rounded-md py-1 text-[12.5px] font-medium",
              t.mode === m
                ? "bg-background shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            {name}
          </button>
        ))}
      </div>
      <div className="text-center">
        <div className="text-[42px] leading-tight font-semibold tracking-tight tabular-nums">
          {big}
        </div>
        <p className="text-[12px] text-muted-foreground">{line}</p>
        {t.mode === "focus" && !idle && (
          <div className="mt-2 flex justify-center gap-1.5" aria-hidden>
            {Array.from({ length: f.rounds }, (_, i) => (
              <span
                key={i}
                className={cn(
                  "size-2 rounded-full",
                  i < ((t.round - 1) % f.rounds) + (t.phase === "work" ? 0 : 1)
                    ? "bg-emerald-600"
                    : "bg-muted",
                )}
              />
            ))}
          </div>
        )}
      </div>
      <div className="flex justify-center gap-2">
        {idle ? (
          <Button size="sm" onClick={t.start}>
            Start
          </Button>
        ) : (
          <>
            <Button size="sm" onClick={t.status === "running" ? t.pause : t.resume}>
              {t.status === "running"
                ? "Pause"
                : t.phase !== "work"
                  ? "Start break"
                  : t.began
                    ? "Resume"
                    : "Start reading"}
            </Button>
            {t.mode === "focus" && (
              <Button size="sm" variant="outline" onClick={t.skip}>
                Skip
              </Button>
            )}
            <Button size="sm" variant="outline" onClick={t.stop}>
              {t.mode === "stopwatch" ? "Stop and save" : "Stop"}
            </Button>
          </>
        )}
      </div>
      <div className="flex flex-col divide-y border-t text-[12.5px]">
        {t.mode === "focus" &&
          (editing ? (
            <div className="grid grid-cols-4 gap-2 py-2">
              {(
                [
                  ["work", "Focus", 5, 120],
                  ["brk", "Break", 1, 60],
                  ["long", "Long", 1, 90],
                  ["rounds", "Rounds", 1, 12],
                ] as const
              ).map(([k, name, min, max]) => (
                <label key={k} className="flex flex-col gap-0.5 text-[11px] text-muted-foreground">
                  {name}
                  <input
                    type="number"
                    min={min}
                    max={max}
                    value={f[k]}
                    onChange={(e) => {
                      const v = Math.min(max, Math.max(min, Number(e.target.value) || min));
                      update((d) => ({ ...d, focus: { ...d.focus, [k]: v } }));
                    }}
                    className="h-7 w-full rounded-md border bg-background px-1.5 text-[12.5px] text-foreground tabular-nums"
                  />
                </label>
              ))}
            </div>
          ) : (
            <Option label={`Focus ${f.work} · Break ${f.brk} · Long break ${f.long}`}>
              <button
                type="button"
                className="text-muted-foreground hover:text-foreground"
                onClick={() => setEditing(true)}
              >
                Change
              </button>
            </Option>
          ))}
        {t.mode === "timer" && (
          <Option label="Minutes">
            <input
              type="number"
              min={1}
              max={600}
              aria-label="Timer minutes"
              value={o.timerMinutes}
              disabled={!idle}
              onChange={(e) => {
                const v = Math.min(600, Math.max(1, Number(e.target.value) || 1));
                update((d) => ({ ...d, options: { ...d.options, timerMinutes: v } }));
              }}
              className="h-7 w-16 rounded-md border bg-background px-1.5 text-right tabular-nums"
            />
          </Option>
        )}
        {t.mode !== "timer" && (
          <Option label="Count it as reading this book">
            <Toggle
              label="Count it as reading this book"
              on={o.countReading}
              set={(v) => setOption("countReading", v)}
            />
          </Option>
        )}
        {t.mode === "focus" && (
          <>
            <Option label="Pause read aloud and listening at breaks">
              <Toggle
                label="Pause read aloud and listening at breaks"
                on={o.pauseAudio}
                set={(v) => setOption("pauseAudio", v)}
              />
            </Option>
            <Option label="Ask what I took away">
              <Toggle
                label="Ask what I took away"
                on={o.askTakeaway}
                set={(v) => setOption("askTakeaway", v)}
              />
            </Option>
          </>
        )}
      </div>
    </div>
  );
}

function Option({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 py-2">
      <span>{label}</span>
      {children}
    </div>
  );
}

function Toggle({ label, on, set }: { label: string; on: boolean; set: (v: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      onClick={() => set(!on)}
      className={cn(
        "relative h-[18px] w-8 shrink-0 rounded-full transition-colors",
        on ? "bg-primary" : "bg-muted-foreground/30",
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 left-0.5 size-3.5 rounded-full bg-background shadow transition-transform",
          on && "translate-x-3.5",
        )}
      />
    </button>
  );
}
