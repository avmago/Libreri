import { useEffect, useMemo, useState } from "react";
import { save } from "@tauri-apps/plugin-dialog";
import { BookOpen, Brain, Download, Flame } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { commands, unwrap } from "@/lib/ipc";
import { useTabs } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import {
  ankiNotes,
  answer,
  clozeParts,
  forecast,
  GRADES,
  KIND_NAME,
  opening,
  previews,
  streak,
  today,
  type ReviewCard,
  type ReviewSettings,
} from "./model";
import { useCards, useReview } from "./store";
import type { Grade } from "ts-fsrs";

/**
 * Daily review: today's cards from your highlights, the days ahead, which
 * books take part, and the export to Anki. "Start" turns it into a session.
 */
export function ReviewView() {
  const [now, setNow] = useState(() => new Date());
  useEffect(() => {
    const t = setInterval(() => setNow(new Date()), 60_000);
    return () => clearInterval(t);
  }, []);
  const r = useCards(now);
  const [session, setSession] = useState<ReviewCard[] | null>(null);

  if (session)
    return (
      <Session
        queue={session}
        onDone={() => {
          setSession(null);
          setNow(new Date());
        }}
      />
    );
  return <Overview r={r} now={now} onStart={() => setSession(r.queue)} />;
}

function Overview({
  r,
  now,
  onStart,
}: {
  r: ReturnType<typeof useCards>;
  now: Date;
  onStart: () => void;
}) {
  const update = useReview((s) => s.update);
  const done = today(r.data, now);
  const ahead = forecast(r.cards, r.data, now);
  const days = streak(r.data, now);
  const max = Math.max(1, ...ahead);
  const set = (k: keyof ReviewSettings, v: number | boolean) =>
    update((d) => ({ ...d, settings: { ...d.settings, [k]: v } }));

  const books = useMemo(() => {
    const m = new Map<string, { title: string; cards: number }>();
    for (const c of r.cards) {
      const b = m.get(c.bookId) ?? { title: c.bookTitle, cards: 0 };
      b.cards++;
      m.set(c.bookId, b);
    }
    return [...m.entries()].sort((a, b) => b[1].cards - a[1].cards);
  }, [r.cards]);
  const offBooks = r.data.booksOff;

  const exportAnki = async () => {
    if (!r.cards.length) return;
    const path = await save({
      defaultPath: "Libreri highlights.apkg",
      filters: [{ name: "Anki deck", extensions: ["apkg"] }],
    });
    if (!path) return;
    try {
      const n = await unwrap(commands.exportAnki(path, "Libreri highlights", ankiNotes(r.cards)));
      toast.success(`${n} cards saved for Anki`, {
        description: "Open the file in Anki to add them. Importing again updates them.",
      });
    } catch (e) {
      toast.error("Could not save the Anki deck", {
        description: e instanceof Error ? e.message : String(e),
      });
    }
  };

  const kinds = { passage: 0, qa: 0, cloze: 0 };
  for (const c of r.cards) kinds[c.kind]++;

  return (
    <div className="flex h-full min-h-0 flex-col gap-4 overflow-auto p-5 @container">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="mr-2 text-[20px] font-semibold tracking-tight">Daily review</h1>
        <span className="flex-1" />
        <Button variant="outline" size="sm" disabled={!r.cards.length} onClick={exportAnki}>
          <Download /> Export to Anki
        </Button>
      </div>

      <div className="grid gap-4 @3xl:grid-cols-[minmax(0,1fr)_300px]">
        <div className="flex flex-col gap-4">
          <section className="flex flex-col items-center gap-3 rounded-xl border p-6 text-center">
            <Brain className="size-8 text-muted-foreground" aria-hidden />
            {r.loading ? (
              <p className="text-muted-foreground">Loading…</p>
            ) : r.queue.length ? (
              <>
                <div>
                  <div className="text-[34px] leading-tight font-semibold tabular-nums">
                    {r.queue.length}
                  </div>
                  <p className="text-[13px] text-muted-foreground">
                    {r.reviews.length} to review · {r.news.length} new
                  </p>
                </div>
                <Button onClick={onStart}>Start review</Button>
              </>
            ) : r.cards.length ? (
              <>
                <p className="text-[15px] font-medium">All done for today</p>
                <p className="max-w-sm text-[13px] text-muted-foreground">
                  {done.reviewed ? `${done.reviewed} cards reviewed today. ` : ""}
                  {ahead[1] ? `${ahead[1]} come back tomorrow.` : "Nothing is due tomorrow."}
                </p>
              </>
            ) : (
              <>
                <p className="text-[15px] font-medium">No cards yet</p>
                <p className="max-w-sm text-[13px] text-muted-foreground">
                  Highlight passages while you read. Each highlight comes back here as a card; a
                  comment on it makes a question, and you can hide words for a cloze.
                </p>
              </>
            )}
            {days > 0 && (
              <p className="flex items-center gap-1 text-[12.5px] text-muted-foreground">
                <Flame className="size-3.5 text-orange-500" aria-hidden />
                {days} {days === 1 ? "day" : "days"} in a row
              </p>
            )}
          </section>

          <section className="rounded-xl border p-3.5" aria-label="Coming up">
            <h2 className="mb-2 text-[13.5px] font-semibold">The next 7 days</h2>
            <div className="flex h-24 items-end gap-2">
              {ahead.map((n, i) => {
                const d = new Date(now);
                d.setDate(d.getDate() + i);
                return (
                  <div key={i} className="flex flex-1 flex-col items-center gap-1">
                    <span className="text-[11px] text-muted-foreground tabular-nums">
                      {n || ""}
                    </span>
                    <div
                      className={cn("w-full rounded-t", i === 0 ? "bg-primary" : "bg-primary/40")}
                      style={{ height: `${Math.max(2, (n / max) * 56)}px` }}
                    />
                    <span className="text-[11px] text-muted-foreground">
                      {i === 0 ? "Today" : d.toLocaleDateString(undefined, { weekday: "short" })}
                    </span>
                  </div>
                );
              })}
            </div>
            <p className="mt-2 text-[12px] text-muted-foreground">
              {r.cards.length} cards: {kinds.passage} passages, {kinds.qa} questions, {kinds.cloze}{" "}
              cloze
              {r.waitingNew > r.news.length
                ? ` · ${r.waitingNew - r.news.length} new ones wait for the coming days`
                : ""}
            </p>
          </section>
        </div>

        <div className="flex flex-col gap-4">
          <section className="rounded-xl border p-3.5" aria-label="Settings">
            <h2 className="mb-1 text-[13.5px] font-semibold">Each day</h2>
            <div className="flex flex-col divide-y text-[12.5px]">
              <Row label="New cards">
                <NumberBox
                  label="New cards a day"
                  value={r.data.settings.newPerDay}
                  min={0}
                  max={200}
                  set={(v) => set("newPerDay", v)}
                />
              </Row>
              <Row label="Reviews at most">
                <NumberBox
                  label="Reviews a day at most"
                  value={r.data.settings.maxReviews}
                  min={1}
                  max={2000}
                  set={(v) => set("maxReviews", v)}
                />
              </Row>
              <Row label="Remember">
                <select
                  aria-label="How likely you should be to remember"
                  value={r.data.settings.retention}
                  onChange={(e) => set("retention", Number(e.target.value))}
                  className="h-7 rounded-md border bg-background px-1.5"
                >
                  {[0.8, 0.85, 0.9, 0.95].map((v) => (
                    <option key={v} value={v}>
                      {Math.round(v * 100)}% (
                      {v < 0.85 ? "fewer reviews" : v > 0.9 ? "more reviews" : "balanced"})
                    </option>
                  ))}
                </select>
              </Row>
              <Row label="Every new highlight becomes a card">
                <Switch
                  label="Every new highlight becomes a card"
                  on={r.data.settings.autoNew}
                  set={(v) => set("autoNew", v)}
                />
              </Row>
            </div>
          </section>

          <section className="rounded-xl border p-3.5" aria-label="Books">
            <h2 className="mb-1 text-[13.5px] font-semibold">Books</h2>
            {books.length === 0 && offBooks.length === 0 ? (
              <p className="py-1.5 text-[12.5px] text-muted-foreground">
                Books with highlights show here.
              </p>
            ) : (
              <div className="flex flex-col divide-y text-[12.5px]">
                {books.map(([id, b]) => (
                  <Row key={id} label={`${b.title} · ${b.cards}`}>
                    <Switch
                      label={`Review ${b.title}`}
                      on
                      set={() => update((d) => ({ ...d, booksOff: [...d.booksOff, id] }))}
                    />
                  </Row>
                ))}
                {offBooks.length > 0 && (
                  <Row label={`${offBooks.length} left out`}>
                    <button
                      type="button"
                      className="text-muted-foreground hover:text-foreground"
                      onClick={() => update((d) => ({ ...d, booksOff: [] }))}
                    >
                      Bring back
                    </button>
                  </Row>
                )}
              </div>
            )}
          </section>
        </div>
      </div>
    </div>
  );
}

/** A review session: one card at a time, Show answer, then how well. */
function Session({ queue, onDone }: { queue: ReviewCard[]; onDone: () => void }) {
  const update = useReview((s) => s.update);
  const data = useReview((s) => s.data);
  const [list, setList] = useState(queue);
  const [i, setI] = useState(0);
  const [shown, setShown] = useState(false);
  const [again, setAgain] = useState(0);
  const card = list[i];
  const pv = useMemo(
    () => (card ? previews(card, data, new Date()) : null),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [card?.key, data.settings.retention],
  );

  const rate = (g: Grade) => {
    if (!card) return;
    const at = new Date();
    update((d) => answer(d, card, g, at));
    // A card due again within minutes (Again, or a new card's first step)
    // comes back at the end of this session.
    const due = new Date(answer(data, card, g, at).sched[card.key]!.due).getTime();
    if (due - at.getTime() <= 20 * 60_000) {
      setList((l) => [...l, card]);
      setAgain((n) => n + 1);
    }
    setShown(false);
    setI((n) => n + 1);
  };

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest("input, textarea, select")) return;
      if (e.key === "Escape") return onDone();
      if (!shown && (e.key === " " || e.key === "Enter")) {
        e.preventDefault();
        setShown(true);
      } else if (shown) {
        const g = GRADES.find((x) => x.key === e.key);
        if (g) rate(g.grade);
        else if (e.key === " " || e.key === "Enter") {
          e.preventDefault();
          rate(GRADES[2]!.grade);
        }
      }
    };
    window.addEventListener("keydown", key);
    return () => window.removeEventListener("keydown", key);
  });

  if (!card)
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 p-5 text-center">
        <p className="text-[17px] font-semibold">Done for now</p>
        <p className="text-[13px] text-muted-foreground">
          {queue.length} {queue.length === 1 ? "card" : "cards"} reviewed
          {again ? `, ${again} shown again` : ""}.
        </p>
        <Button onClick={onDone}>Back to the overview</Button>
      </div>
    );

  return (
    <div className="flex h-full min-h-0 flex-col overflow-auto p-5">
      <div className="mx-auto flex w-full max-w-2xl flex-col gap-4 pt-[6vh]">
        <div className="flex items-center gap-3 text-[12.5px] text-muted-foreground">
          <Button variant="ghost" size="sm" onClick={onDone}>
            End
          </Button>
          <div className="h-1.5 flex-1 overflow-hidden rounded-full bg-muted">
            <div
              className="h-full bg-primary transition-all"
              style={{ width: `${(i / list.length) * 100}%` }}
            />
          </div>
          <span className="tabular-nums">
            {i + 1} / {list.length}
          </span>
        </div>

        <article
          aria-label="Card"
          className="flex min-h-72 flex-col gap-4 rounded-2xl border bg-card p-6 shadow-sm"
        >
          <div className="flex items-center gap-2 text-[11.5px] font-medium tracking-wide text-muted-foreground uppercase">
            {KIND_NAME[card.kind]}
            {!data.sched[card.key] && (
              <span className="rounded bg-sky-100 px-1.5 py-px text-[10.5px] text-sky-800 dark:bg-sky-950 dark:text-sky-200">
                New
              </span>
            )}
          </div>
          <Face card={card} shown={shown} />
          <span className="flex-1" />
          <div className="flex items-center justify-between gap-2 text-[12px] text-muted-foreground">
            <span className="truncate">
              {card.bookTitle}
              {card.label ? ` · ${card.label}` : ""}
            </span>
            <button
              type="button"
              className="flex shrink-0 items-center gap-1 hover:text-foreground"
              onClick={() =>
                useTabs.getState().open({
                  bookId: card.bookId,
                  title: card.bookTitle,
                  fileType: card.fileType,
                  jumpTo: card.annotationId,
                })
              }
            >
              <BookOpen className="size-3.5" /> Open in the book
            </button>
          </div>
        </article>

        <div className="flex justify-center gap-2 pb-2">
          {!shown ? (
            <Button className="min-w-48" onClick={() => setShown(true)}>
              Show answer
              <kbd className="ml-1 text-[11px] opacity-60">Space</kbd>
            </Button>
          ) : (
            GRADES.map(({ grade, name, key }) => (
              <button
                key={grade}
                type="button"
                onClick={() => rate(grade)}
                className={cn(
                  "flex min-w-24 flex-col items-center rounded-lg border px-3 py-1.5 text-[13px] font-medium hover:bg-muted",
                  grade === GRADES[0]!.grade && "text-red-700 dark:text-red-300",
                  grade === GRADES[2]!.grade && "border-primary",
                )}
              >
                {name}
                <span className="text-[11px] font-normal text-muted-foreground">
                  {pv?.[grade]} · {key}
                </span>
              </button>
            ))
          )}
        </div>
      </div>
    </div>
  );
}

/** The front, and the back once shown. */
function Face({ card, shown }: { card: ReviewCard; shown: boolean }) {
  const q = "font-serif text-[19px] leading-relaxed";
  if (card.kind === "qa")
    return (
      <div className="flex flex-col gap-4">
        <p className="text-[18px] font-medium">{card.note}</p>
        {shown && <blockquote className={cn(q, "border-l-2 pl-4")}>{card.quote}</blockquote>}
      </div>
    );
  if (card.kind === "cloze")
    return (
      <p className={q}>
        {clozeParts(card.quote, card.cloze).map((p, i) =>
          p.hidden ? (
            shown ? (
              <mark key={i} className="rounded bg-amber-200/70 px-0.5 dark:bg-amber-500/40">
                {p.text}
              </mark>
            ) : (
              <span
                key={i}
                aria-label="hidden word"
                className="inline-block min-w-12 rounded border-b-2 border-dashed border-primary bg-muted px-1 text-transparent"
              >
                {p.text}
              </span>
            )
          ) : (
            <span key={i}>{p.text}</span>
          ),
        )}
      </p>
    );
  const { start, rest } = opening(card.quote);
  return (
    <div className="flex flex-col gap-3">
      <p className={q}>
        {start}
        {shown ? (
          <span className="rounded bg-amber-200/50 dark:bg-amber-500/30">{rest}</span>
        ) : rest ? (
          <span className="text-muted-foreground"> …</span>
        ) : null}
      </p>
      {!shown && rest && (
        <p className="text-[12.5px] text-muted-foreground">How does the passage go on?</p>
      )}
      {shown && card.note && (
        <p className="border-l-2 pl-3 text-[13.5px] text-muted-foreground">{card.note}</p>
      )}
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-center justify-between gap-3 py-2">
      <span className="min-w-0 truncate">{label}</span>
      {children}
    </div>
  );
}

function NumberBox({
  label,
  value,
  min,
  max,
  set,
}: {
  label: string;
  value: number;
  min: number;
  max: number;
  set: (v: number) => void;
}) {
  return (
    <input
      type="number"
      aria-label={label}
      min={min}
      max={max}
      value={value}
      onChange={(e) => set(Math.min(max, Math.max(min, Number(e.target.value) || min)))}
      className="h-7 w-16 rounded-md border bg-background px-1.5 text-right tabular-nums"
    />
  );
}

export function Switch({
  label,
  on,
  set,
}: {
  label: string;
  on: boolean;
  set: (v: boolean) => void;
}) {
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
