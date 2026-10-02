import { useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import {
  ChevronLeft,
  ChevronRight,
  Check,
  Download,
  MoreHorizontal,
  Plus,
  Timer,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { DropdownMenu, menuContent, menuItem } from "@/components/ui/menu";
import { openBook } from "@/features/feeds";
import { commands, unwrap } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { GoalDialog } from "./GoalDialog";
import {
  byDay,
  dayKey,
  duration,
  fromKey,
  monthGrid,
  monthTotals,
  pace,
  sessionPages,
  streak,
  toIcs,
  weekOf,
  type Goal,
} from "./model";
import { useStudy } from "./store";

const WEEKDAYS = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];

/**
 * The reading calendar (per profile): days you read, timer sessions,
 * goals and due dates; today, goals and this month beside it.
 */
export function CalendarView() {
  const data = useStudy((s) => s.data);
  const loaded = useStudy((s) => s.loaded);
  const update = useStudy((s) => s.update);
  const today = dayKey(new Date());
  const [cursor, setCursor] = useState(today);
  const [view, setView] = useState<"month" | "week">("month");
  const [selected, setSelected] = useState(today);
  const [goalOpen, setGoalOpen] = useState<Goal | "new" | null>(null);

  const days = useMemo(() => byDay(data.sessions), [data.sessions]);
  const c = fromKey(cursor);
  const month = cursor.slice(0, 7);
  const weeks = view === "month" ? monthGrid(c.getFullYear(), c.getMonth()) : [weekOf(cursor)];
  const maxMinutes = Math.max(60, ...[...days.values()].map((d) => d.minutes));
  const goalsOn = (k: string) => data.goals.filter((g) => g.due === k);

  const move = (dir: 1 | -1) => {
    const d = fromKey(cursor);
    if (view === "month") d.setMonth(d.getMonth() + dir, 1);
    else d.setDate(d.getDate() + dir * 7);
    setCursor(dayKey(d));
  };

  const title =
    view === "month"
      ? c.toLocaleDateString(undefined, { month: "long", year: "numeric" })
      : `${fromKey(weeks[0]![0]!).toLocaleDateString(undefined, { day: "numeric", month: "short" })} – ${fromKey(weeks[0]![6]!).toLocaleDateString(undefined, { day: "numeric", month: "short", year: "numeric" })}`;

  const exportIcs = async () => {
    const path = await save({
      defaultPath: "Libreri reading.ics",
      filters: [{ name: "Calendar", extensions: ["ics"] }],
    });
    if (!path) return;
    try {
      await unwrap(commands.saveCalendarFile(path, toIcs(data)));
      toast.success("Calendar saved", {
        description: "Open it in Apple Calendar, Google Calendar or Outlook to add it.",
      });
    } catch (e) {
      toast.error("Could not save the calendar", {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 overflow-auto p-5 @container">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="mr-2 text-[20px] font-semibold tracking-tight">{title}</h1>
        <Button variant="outline" size="icon" aria-label="Back" onClick={() => move(-1)}>
          <ChevronLeft />
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => {
            setCursor(today);
            setSelected(today);
          }}
        >
          Today
        </Button>
        <Button variant="outline" size="icon" aria-label="Forward" onClick={() => move(1)}>
          <ChevronRight />
        </Button>
        <span className="flex-1" />
        <div role="tablist" aria-label="View" className="flex rounded-lg bg-muted p-0.5">
          {(["month", "week"] as const).map((v) => (
            <button
              key={v}
              type="button"
              role="tab"
              aria-selected={view === v}
              onClick={() => setView(v)}
              className={cn(
                "rounded-md px-3 py-1 text-[12.5px] font-medium capitalize",
                view === v ? "bg-background shadow-sm" : "text-muted-foreground",
              )}
            >
              {v}
            </button>
          ))}
        </div>
        <Button size="sm" onClick={() => setGoalOpen("new")}>
          <Plus /> Reading goal
        </Button>
        <Button
          variant="outline"
          size="sm"
          onClick={() => void exportIcs()}
          title="Save as an .ics file for other calendars"
        >
          <Download /> <span className="@max-3xl:hidden">Export .ics</span>
        </Button>
      </div>

      <div className="flex min-h-0 flex-1 gap-4 @max-4xl:flex-col">
        <div
          role="grid"
          aria-label="Reading calendar"
          className={cn(
            "grid min-h-[420px] flex-1 grid-cols-7 overflow-hidden rounded-xl border",
            view === "week" && "min-h-[260px]",
          )}
          style={{ gridTemplateRows: `auto repeat(${weeks.length}, minmax(0, 1fr))` }}
        >
          {WEEKDAYS.map((d) => (
            <div
              key={d}
              className="border-b bg-muted/40 px-2.5 py-1.5 text-[11px] font-semibold text-muted-foreground"
            >
              {d}
            </div>
          ))}
          {weeks.flat().map((k) => {
            const d = days.get(k);
            const other = view === "month" && !k.startsWith(month);
            const goals = goalsOn(k);
            const books = new Set(d?.sessions.map((s) => s.title).filter(Boolean));
            return (
              <button
                key={k}
                type="button"
                role="gridcell"
                aria-selected={selected === k}
                aria-label={`${fromKey(k).toLocaleDateString(undefined, { weekday: "long", day: "numeric", month: "long" })}${d ? `, read ${duration(d.minutes)}` : ""}${goals.length ? `, ${goals.length} due` : ""}`}
                onClick={() => setSelected(k)}
                className={cn(
                  "flex min-h-0 flex-col items-stretch gap-1 overflow-hidden border-r border-b px-1.5 py-1.5 text-left text-[11.5px] last:border-r-0 hover:bg-muted/40 [&:nth-child(7n)]:border-r-0",
                  selected === k && "bg-muted/60",
                )}
              >
                <span
                  className={cn(
                    "self-start rounded-full px-1.5 text-[12px] font-semibold",
                    other ? "text-muted-foreground/50" : "text-muted-foreground",
                    k === today && "bg-foreground text-background",
                  )}
                >
                  {fromKey(k).getDate()}
                </span>
                {d && d.minutes > 0 && (
                  <span
                    aria-hidden
                    className="h-1.5 rounded-full bg-emerald-600"
                    style={{
                      width: `${Math.max(12, Math.min(100, (d.minutes / maxMinutes) * 100))}%`,
                    }}
                  />
                )}
                {d && d.minutes > 0 && (
                  <span className="truncate rounded bg-emerald-50 px-1.5 py-0.5 text-emerald-800 dark:bg-emerald-950 dark:text-emerald-200">
                    ⏱ {duration(d.minutes)}
                    {books.size === 1
                      ? ` · ${[...books][0]}`
                      : books.size > 1
                        ? ` · ${books.size} books`
                        : ""}
                  </span>
                )}
                {goals.map((g) => (
                  <span
                    key={g.id}
                    className={cn(
                      "truncate rounded px-1.5 py-0.5",
                      g.done
                        ? "bg-muted text-muted-foreground line-through"
                        : g.kind === "finish"
                          ? "bg-indigo-50 text-indigo-800 dark:bg-indigo-950 dark:text-indigo-200"
                          : "bg-red-50 text-red-700 dark:bg-red-950 dark:text-red-200",
                    )}
                  >
                    {g.kind === "finish" ? `Finish ${g.title}` : `Due: ${g.title}`}
                  </span>
                ))}
              </button>
            );
          })}
        </div>

        <aside className="flex w-[300px] shrink-0 flex-col gap-3 @max-4xl:w-full">
          <DayCard day={selected} today={today} />
          <GoalsCard today={today} onEdit={(g) => setGoalOpen(g)} />
          <section className="rounded-xl border p-3.5">
            <h2 className="mb-2 text-[13.5px] font-semibold">
              {c.toLocaleDateString(undefined, { month: "long" })}
            </h2>
            <div className="grid grid-cols-3 gap-2">
              <Stat value={duration(monthTotals(days, month).minutes)} label="read" />
              <Stat value={String(streak(days, today))} label="day streak" />
              <Stat value={String(monthTotals(days, month).pages)} label="pages" />
            </div>
          </section>
          {!loaded && <p className="text-[12px] text-muted-foreground">Loading…</p>}
        </aside>
      </div>

      {goalOpen && (
        <GoalDialog
          goal={goalOpen === "new" ? null : goalOpen}
          defaultDue={selected >= today ? selected : today}
          onClose={() => setGoalOpen(null)}
          onSave={(g) =>
            update((d) => ({
              ...d,
              goals: d.goals.some((x) => x.id === g.id)
                ? d.goals.map((x) => (x.id === g.id ? g : x))
                : [...d.goals, g],
            }))
          }
        />
      )}
    </div>
  );
}

function Stat({ value, label }: { value: string; label: string }) {
  return (
    <div className="rounded-lg bg-muted/50 px-2.5 py-2">
      <div className="text-[17px] font-semibold tabular-nums">{value}</div>
      <div className="text-[11.5px] text-muted-foreground">{label}</div>
    </div>
  );
}

/** What happened (and is due) on a day; today also says what to read next. */
function DayCard({ day, today }: { day: string; today: string }) {
  const data = useStudy((s) => s.data);
  const update = useStudy((s) => s.update);
  const d = byDay(data.sessions).get(day);
  const due = data.goals.filter((g) => g.due === day);
  const next =
    day === today
      ? data.goals
          .filter((g) => g.kind === "finish" && !g.done && g.bookId)
          .map((g) => ({ g, p: pace(g, today) }))
          .sort(
            (a, b) => Number(b.p.behind) - Number(a.p.behind) || a.g.due.localeCompare(b.g.due),
          )[0]
      : undefined;
  const label =
    day === today
      ? `Today · ${fromKey(day).toLocaleDateString(undefined, { weekday: "short", day: "numeric", month: "short" })}`
      : fromKey(day).toLocaleDateString(undefined, {
          weekday: "long",
          day: "numeric",
          month: "long",
        });
  return (
    <section className="rounded-xl border p-3.5" aria-label="Day">
      <h2 className="mb-1.5 text-[13.5px] font-semibold">{label}</h2>
      <div className="flex flex-col divide-y text-[12.5px]">
        {d && d.minutes > 0 ? (
          d.sessions.map((s) => (
            <div key={s.id} className="flex items-center justify-between gap-2 py-1.5">
              <span className="flex min-w-0 items-center gap-1.5">
                <Timer className="size-3.5 shrink-0 text-muted-foreground" aria-hidden />
                <span className="truncate">
                  {new Date(s.start).toLocaleTimeString(undefined, {
                    hour: "2-digit",
                    minute: "2-digit",
                  })}{" "}
                  · {duration(s.minutes)}
                  {s.title ? ` · ${s.title}` : ""}
                </span>
              </span>
              <span className="flex shrink-0 items-center gap-1 text-muted-foreground">
                {sessionPages(s) ? `${sessionPages(s)} p.` : ""}
                <button
                  type="button"
                  aria-label="Delete this session"
                  title="Delete this session"
                  className="rounded p-0.5 hover:bg-muted hover:text-foreground"
                  onClick={() =>
                    update((x) => ({ ...x, sessions: x.sessions.filter((y) => y.id !== s.id) }))
                  }
                >
                  <Trash2 className="size-3.5" />
                </button>
              </span>
            </div>
          ))
        ) : (
          <p className="py-1.5 text-muted-foreground">
            {day > today
              ? "Nothing yet."
              : "No reading recorded. Start the timer in a book to count it."}
          </p>
        )}
        {due.map((g) => (
          <div key={g.id} className="flex justify-between gap-2 py-1.5">
            <span
              className={cn(
                "truncate",
                !g.done && "text-red-700 dark:text-red-300",
                g.done && "line-through",
              )}
            >
              {g.kind === "finish" ? `Finish: ${g.title}` : `Due: ${g.title}`}
            </span>
          </div>
        ))}
        {next && (
          <button
            type="button"
            className="flex justify-between gap-2 py-1.5 text-left hover:text-foreground"
            onClick={() => next.g.bookId && void openBook(next.g.bookId)}
          >
            <span className="truncate">Read next: {next.g.title}</span>
            <span className="shrink-0 text-muted-foreground">
              {next.p.perDay ? `${next.p.perDay} p. today →` : "→"}
            </span>
          </button>
        )}
      </div>
    </section>
  );
}

function GoalsCard({ today, onEdit }: { today: string; onEdit: (g: Goal) => void }) {
  const goals = useStudy((s) => s.data.goals);
  const update = useStudy((s) => s.update);
  const [showDone, setShowDone] = useState(false);
  const open = goals.filter((g) => !g.done).sort((a, b) => a.due.localeCompare(b.due));
  const done = goals.filter((g) => g.done);
  const change = (g: Goal) =>
    update((d) => ({ ...d, goals: d.goals.map((x) => (x.id === g.id ? g : x)) }));
  const remove = (g: Goal) =>
    update((d) => ({ ...d, goals: d.goals.filter((x) => x.id !== g.id) }));

  return (
    <section className="rounded-xl border p-3.5" aria-label="Goals">
      <h2 className="mb-2 text-[13.5px] font-semibold">Goals</h2>
      {!open.length && (
        <p className="text-[12.5px] text-muted-foreground">
          Set a date to finish a book, or something to have read by a day.
        </p>
      )}
      <div className="flex flex-col gap-3">
        {open.map((g) => {
          const p = pace(g, today);
          const when = fromKey(g.due).toLocaleDateString(undefined, {
            day: "numeric",
            month: "short",
          });
          return (
            <div key={g.id} className="flex flex-col gap-1 text-[12.5px]">
              <div className="flex items-center gap-2">
                <button
                  type="button"
                  className="min-w-0 flex-1 truncate text-left font-medium hover:underline"
                  onClick={() => (g.bookId ? void openBook(g.bookId) : onEdit(g))}
                >
                  {g.kind === "due" ? `Due: ${g.title}` : g.title}
                </button>
                <span className={cn("shrink-0 text-muted-foreground", p.overdue && "text-red-600")}>
                  by {when}
                </span>
                <GoalMenu
                  onDone={() => change({ ...g, done: true })}
                  onEdit={() => onEdit(g)}
                  onDelete={() => remove(g)}
                />
              </div>
              {g.kind === "finish" && g.pages ? (
                <>
                  <div className="h-1.5 overflow-hidden rounded-full bg-muted" aria-hidden>
                    <div
                      className={cn(
                        "h-full rounded-full",
                        p.behind ? "bg-amber-600" : "bg-foreground/80",
                      )}
                      style={{ width: `${Math.round(p.progress * 100)}%` }}
                    />
                  </div>
                  <span className="text-[11.5px] text-muted-foreground">
                    {p.overdue
                      ? `The date has passed: ${p.left} pages left`
                      : p.behind
                        ? `A little behind: ${p.perDay} pages a day to catch up`
                        : `${g.reached ?? g.startPage ?? 0} of ${g.pages} pages · ${p.perDay} a day to stay on track`}
                  </span>
                </>
              ) : (
                <span
                  className={cn("text-[11.5px] text-muted-foreground", p.overdue && "text-red-600")}
                >
                  {p.overdue
                    ? "The date has passed"
                    : p.daysLeft === 0
                      ? "Due today"
                      : `${p.daysLeft} days left`}
                </span>
              )}
            </div>
          );
        })}
      </div>
      {done.length > 0 && (
        <div className="mt-3 border-t pt-2 text-[12px]">
          <button
            type="button"
            className="text-muted-foreground hover:text-foreground"
            onClick={() => setShowDone(!showDone)}
          >
            {showDone ? "Hide" : "Show"} {done.length} done
          </button>
          {showDone &&
            done.map((g) => (
              <div key={g.id} className="flex items-center gap-2 py-1">
                <Check className="size-3.5 text-emerald-600" aria-hidden />
                <span className="flex-1 truncate line-through">{g.title}</span>
                <button
                  type="button"
                  className="text-muted-foreground hover:text-foreground"
                  onClick={() => change({ ...g, done: false })}
                >
                  Undo
                </button>
                <button
                  type="button"
                  aria-label={`Delete ${g.title}`}
                  className="text-muted-foreground hover:text-foreground"
                  onClick={() => remove(g)}
                >
                  <Trash2 className="size-3.5" />
                </button>
              </div>
            ))}
        </div>
      )}
    </section>
  );
}

function GoalMenu({
  onDone,
  onEdit,
  onDelete,
}: {
  onDone: () => void;
  onEdit: () => void;
  onDelete: () => void;
}) {
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button variant="ghost" size="icon" className="size-6" aria-label="Goal actions">
          <MoreHorizontal />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content className={menuContent} align="end">
          <DropdownMenu.Item className={menuItem} onSelect={onDone}>
            <Check /> Mark as done
          </DropdownMenu.Item>
          <DropdownMenu.Item className={menuItem} onSelect={onEdit}>
            Edit…
          </DropdownMenu.Item>
          <DropdownMenu.Item className={menuItem} onSelect={onDelete}>
            <Trash2 /> Delete
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
