/**
 * Study (the reading calendar and the timer): what is kept per profile,
 * and the sums the calendar shows. Plain functions, tested without a UI.
 */

export interface StudySession {
  id: string;
  /** When it started (ISO). */
  start: string;
  minutes: number;
  kind: "focus" | "timer" | "stopwatch";
  bookId?: string | null;
  title?: string | null;
  pageFrom?: number | null;
  pageTo?: number | null;
}

export interface Goal {
  id: string;
  /** "finish": read a book (or up to a page) by a date; "due": something
   * to have read by a date (a chapter, a paper for a seminar). */
  kind: "finish" | "due";
  title: string;
  bookId?: string | null;
  /** The day it is due, "2026-10-15". */
  due: string;
  /** Finish: the page to reach; where reading stood when it was set; the
   * furthest page reached since. */
  pages?: number | null;
  startPage?: number | null;
  reached?: number | null;
  done?: boolean;
  /** When it was set (ISO). */
  created: string;
}

export interface FocusLengths {
  work: number;
  brk: number;
  long: number;
  rounds: number;
}

export interface StudyOptions {
  /** Count focus time as reading the open book. */
  countReading: boolean;
  /** Pause read aloud, the audiobook and podcasts at breaks. */
  pauseAudio: boolean;
  /** Ask what you took away at each break (saved to the book's notebook). */
  askTakeaway: boolean;
  /** Plain timer length, minutes. */
  timerMinutes: number;
}

export interface StudyData {
  version: 1;
  sessions: StudySession[];
  goals: Goal[];
  focus: FocusLengths;
  options: StudyOptions;
}

export const DEFAULT_STUDY: StudyData = {
  version: 1,
  sessions: [],
  goals: [],
  focus: { work: 25, brk: 5, long: 15, rounds: 4 },
  options: { countReading: true, pauseAudio: true, askTakeaway: false, timerMinutes: 10 },
};

/** A saved document read leniently: missing parts get their defaults. */
export function parseStudy(json: string | null | undefined): StudyData {
  if (!json) return structuredCopy(DEFAULT_STUDY);
  try {
    const raw = JSON.parse(json) as Partial<StudyData>;
    return {
      version: 1,
      sessions: Array.isArray(raw.sessions) ? raw.sessions : [],
      goals: Array.isArray(raw.goals) ? raw.goals : [],
      focus: { ...DEFAULT_STUDY.focus, ...raw.focus },
      options: { ...DEFAULT_STUDY.options, ...raw.options },
    };
  } catch {
    return structuredCopy(DEFAULT_STUDY);
  }
}

function structuredCopy<T>(v: T): T {
  return JSON.parse(JSON.stringify(v)) as T;
}

/** "2026-10-02" for a time, in this computer's time zone. */
export function dayKey(d: Date | string | number): string {
  const x = new Date(d);
  const m = String(x.getMonth() + 1).padStart(2, "0");
  const day = String(x.getDate()).padStart(2, "0");
  return `${x.getFullYear()}-${m}-${day}`;
}

/** The day `key` as a local date at midnight. */
export function fromKey(key: string): Date {
  const [y, m, d] = key.split("-").map(Number);
  return new Date(y!, (m ?? 1) - 1, d ?? 1);
}

/** Whole days from `a` to `b` (keys). */
export function daysBetween(a: string, b: string): number {
  return Math.round((fromKey(b).getTime() - fromKey(a).getTime()) / 86_400_000);
}

export function addDays(key: string, n: number): string {
  const d = fromKey(key);
  d.setDate(d.getDate() + n);
  return dayKey(d);
}

/** Pages a session covered. */
export function sessionPages(s: StudySession): number {
  if (s.pageFrom == null || s.pageTo == null) return 0;
  return Math.max(0, s.pageTo - s.pageFrom);
}

export interface DayTotal {
  minutes: number;
  pages: number;
  sessions: StudySession[];
}

/** Reading per day. */
export function byDay(sessions: StudySession[]): Map<string, DayTotal> {
  const m = new Map<string, DayTotal>();
  for (const s of sessions) {
    const k = dayKey(s.start);
    const t = m.get(k) ?? { minutes: 0, pages: 0, sessions: [] };
    t.minutes += s.minutes;
    t.pages += sessionPages(s);
    t.sessions.push(s);
    m.set(k, t);
  }
  return m;
}

/** Days in a row with reading, ending today (or yesterday, when today
 * has none yet). */
export function streak(days: Map<string, DayTotal>, today: string): number {
  let k = days.get(today)?.minutes ? today : addDays(today, -1);
  let n = 0;
  while (days.get(k)?.minutes) {
    n++;
    k = addDays(k, -1);
  }
  return n;
}

/** Minutes and pages in one month ("2026-10"). */
export function monthTotals(days: Map<string, DayTotal>, month: string) {
  let minutes = 0;
  let pages = 0;
  for (const [k, t] of days)
    if (k.startsWith(month)) {
      minutes += t.minutes;
      pages += t.pages;
    }
  return { minutes, pages };
}

/** "9 h 40", "50 min". */
export function duration(minutes: number): string {
  const m = Math.round(minutes);
  if (m < 60) return `${m} min`;
  const h = Math.floor(m / 60);
  const r = m % 60;
  return r ? `${h} h ${String(r).padStart(2, "0")}` : `${h} h`;
}

/** The weeks shown for a month, Monday first: each a row of 7 day keys. */
export function monthGrid(year: number, month: number): string[][] {
  const first = new Date(year, month, 1);
  const back = (first.getDay() + 6) % 7;
  let k = addDays(dayKey(first), -back);
  const weeks: string[][] = [];
  do {
    const w: string[] = [];
    for (let i = 0; i < 7; i++) {
      w.push(k);
      k = addDays(k, 1);
    }
    weeks.push(w);
  } while (fromKey(k).getMonth() === month);
  return weeks;
}

/** The week (Monday first) holding `key`. */
export function weekOf(key: string): string[] {
  const back = (fromKey(key).getDay() + 6) % 7;
  const start = addDays(key, -back);
  return Array.from({ length: 7 }, (_, i) => addDays(start, i));
}

export interface Pace {
  /** Pages still to read. */
  left: number;
  /** Pages a day to finish on time (from today). */
  perDay: number;
  /** 0–1. */
  progress: number;
  /** Fewer pages read than an even pace would have by now. */
  behind: boolean;
  /** Past the date and not done. */
  overdue: boolean;
  daysLeft: number;
}

/** How a goal stands on `today`. */
export function pace(g: Goal, today: string): Pace {
  const daysLeft = daysBetween(today, g.due);
  const overdue = !g.done && daysLeft < 0;
  if (g.kind !== "finish" || !g.pages) {
    return { left: 0, perDay: 0, progress: g.done ? 1 : 0, behind: overdue, overdue, daysLeft };
  }
  const start = g.startPage ?? 0;
  const reached = Math.max(start, g.reached ?? start);
  const total = Math.max(1, g.pages - start);
  const left = Math.max(0, g.pages - reached);
  const progress = g.done ? 1 : Math.min(1, (reached - start) / total);
  const perDay = left ? Math.ceil(left / Math.max(1, daysLeft + 1)) : 0;
  const span = Math.max(1, daysBetween(dayKey(g.created), g.due) + 1);
  const elapsed = Math.min(span, Math.max(0, daysBetween(dayKey(g.created), today)));
  const expected = (total * elapsed) / span;
  const behind = !g.done && (overdue || reached - start + 1 < expected);
  return { left, perDay, progress, behind, overdue, daysLeft };
}

/** Records how far a book was read, for its open goals. Returns the goals
 * changed (or null when nothing changed). */
export function noteReached(
  goals: Goal[],
  bookId: string,
  page: number | null,
  progress = 0,
): Goal[] | null {
  let changed = false;
  const next = goals.map((g) => {
    if (g.kind !== "finish" || g.done || g.bookId !== bookId) return g;
    // Books without pages (EPUB): as far through the goal's pages.
    const at = page ?? (g.pages ? Math.round(progress * g.pages) : 0);
    if (!at || (g.reached ?? 0) >= at) return g;
    const page_ = at;
    changed = true;
    const done = !!g.pages && page_ >= g.pages;
    return { ...g, reached: page_, done };
  });
  return changed ? next : null;
}

function icsText(s: string): string {
  return s
    .replace(/\\/g, "\\\\")
    .replace(/\n/g, "\\n")
    .replace(/([,;])/g, "\\$1");
}

function icsStamp(d: Date): string {
  return d
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\.\d{3}/, "");
}

/** Goals (all-day, on their date) and reading sessions as an .ics file. */
export function toIcs(data: StudyData, now = new Date()): string {
  const lines = [
    "BEGIN:VCALENDAR",
    "VERSION:2.0",
    "PRODID:-//Libreri//Reading calendar//EN",
    "CALSCALE:GREGORIAN",
    "X-WR-CALNAME:Libreri reading",
  ];
  const stamp = icsStamp(now);
  for (const g of data.goals) {
    const day = g.due.replace(/-/g, "");
    const next = addDays(g.due, 1).replace(/-/g, "");
    const what = g.kind === "finish" ? `Finish: ${g.title}` : `Due: ${g.title}`;
    lines.push(
      "BEGIN:VEVENT",
      `UID:goal-${g.id}@libreri`,
      `DTSTAMP:${stamp}`,
      `DTSTART;VALUE=DATE:${day}`,
      `DTEND;VALUE=DATE:${next}`,
      `SUMMARY:${icsText(what)}`,
      ...(g.done ? ["STATUS:CONFIRMED", "DESCRIPTION:Done"] : []),
      "END:VEVENT",
    );
  }
  for (const s of data.sessions) {
    const start = new Date(s.start);
    const end = new Date(start.getTime() + s.minutes * 60_000);
    const pages = sessionPages(s);
    lines.push(
      "BEGIN:VEVENT",
      `UID:session-${s.id}@libreri`,
      `DTSTAMP:${stamp}`,
      `DTSTART:${icsStamp(start)}`,
      `DTEND:${icsStamp(end)}`,
      `SUMMARY:${icsText(`Reading${s.title ? `: ${s.title}` : ""}`)}`,
      ...(pages ? [`DESCRIPTION:${icsText(`${pages} pages`)}`] : []),
      "END:VEVENT",
    );
  }
  lines.push("END:VCALENDAR");
  return lines.join("\r\n") + "\r\n";
}

/** "15:32", "1:02:03". */
export function clock(ms: number): string {
  const s = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}

export function newId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
}
