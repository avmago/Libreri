/**
 * Daily review: highlights come back as cards on a spaced schedule (FSRS).
 * What is kept per profile, how cards are made from highlights, which are
 * due today, and the Anki notes. Plain functions, tested without a UI.
 */
import {
  createEmptyCard,
  fsrs,
  generatorParameters,
  Rating,
  TypeConvert,
  type Card,
  type Grade,
} from "ts-fsrs";
import type { AnkiNote, FileType, NoteDto } from "@/lib/ipc";

export type CardKind = "passage" | "qa" | "cloze";
export const KIND_NAME: Record<CardKind, string> = {
  passage: "Passage",
  qa: "Question and answer",
  cloze: "Cloze",
};

/** What one highlight gives (none set: the defaults). */
export interface CardOptions {
  kinds?: CardKind[];
  /** Cloze: the words hidden. */
  cloze?: string[];
  /** Left out of review. */
  off?: boolean;
}

export interface ReviewSettings {
  /** New cards a day. */
  newPerDay: number;
  /** Reviews a day at most. */
  maxReviews: number;
  /** How likely you should be to remember (0.8–0.97). */
  retention: number;
  /** Every highlight becomes a card by itself. */
  autoNew: boolean;
}

export interface ReviewLog {
  /** When (ISO). */
  t: string;
  key: string;
  rating: Grade;
  /** The card's first review. */
  new?: boolean;
}

/** A schedule as saved: ts-fsrs's card with dates as text. */
export type SavedCard = Omit<Card, "due" | "last_review"> & { due: string; last_review?: string };

export interface ReviewData {
  version: 1;
  settings: ReviewSettings;
  /** Books whose highlights are not reviewed. */
  booksOff: string[];
  /** By highlight id. */
  cards: Record<string, CardOptions>;
  /** By card key (`<highlight id>:<kind>`). */
  sched: Record<string, SavedCard>;
  log: ReviewLog[];
}

export const DEFAULT_SETTINGS: ReviewSettings = {
  newPerDay: 10,
  maxReviews: 100,
  retention: 0.9,
  autoNew: true,
};

export const DEFAULT_REVIEW: ReviewData = {
  version: 1,
  settings: DEFAULT_SETTINGS,
  booksOff: [],
  cards: {},
  sched: {},
  log: [],
};

/** Reads the saved document, filling what is missing. */
export function parseReview(json: string | null | undefined): ReviewData {
  if (!json) return DEFAULT_REVIEW;
  try {
    const d = JSON.parse(json) as Partial<ReviewData>;
    return {
      version: 1,
      settings: { ...DEFAULT_SETTINGS, ...(d.settings ?? {}) },
      booksOff: Array.isArray(d.booksOff) ? d.booksOff : [],
      cards: d.cards && typeof d.cards === "object" ? d.cards : {},
      sched: d.sched && typeof d.sched === "object" ? d.sched : {},
      log: Array.isArray(d.log) ? d.log : [],
    };
  } catch {
    return DEFAULT_REVIEW;
  }
}

/** One card to review. */
export interface ReviewCard {
  key: string;
  kind: CardKind;
  annotationId: string;
  bookId: string;
  bookTitle: string;
  fileType: FileType;
  /** "p. 4" or a chapter. */
  label: string | null;
  quote: string;
  /** The comment (Q&A: the question). */
  note: string | null;
  cloze: string[];
  created: string;
}

export const cardKey = (annotationId: string, kind: CardKind) => `${annotationId}:${kind}`;

/** The kinds a highlight can give: Q&A needs a comment, cloze chosen words. */
export function possibleKinds(note: string | null | undefined, o: CardOptions = {}): CardKind[] {
  return [
    "passage",
    ...(note?.trim() ? (["qa"] as const) : []),
    ...(o.cloze?.length ? (["cloze"] as const) : []),
  ];
}

/** The kinds a highlight gives now. */
export function kindsOf(
  note: string | null | undefined,
  o: CardOptions | undefined,
  autoNew: boolean,
): CardKind[] {
  if (o?.off) return [];
  const can = possibleKinds(note, o);
  if (o?.kinds) return o.kinds.filter((k) => can.includes(k));
  return autoNew ? can : [];
}

/** Every card the highlights give. */
export function buildCards(notes: NoteDto[], data: ReviewData): ReviewCard[] {
  const off = new Set(data.booksOff);
  const out: ReviewCard[] = [];
  for (const { annotation: a, bookTitle, fileType } of notes) {
    const quote = a.quote?.exact?.trim();
    if (a.kind !== "highlight" || !quote || off.has(a.bookId)) continue;
    const o = data.cards[a.id];
    for (const kind of kindsOf(a.note, o, data.settings.autoNew))
      out.push({
        key: cardKey(a.id, kind),
        kind,
        annotationId: a.id,
        bookId: a.bookId,
        bookTitle,
        fileType,
        label: a.label,
        quote,
        note: a.note?.trim() || null,
        cloze: o?.cloze ?? [],
        created: a.createdAt,
      });
  }
  return out;
}

/** The scheduler for a retention. */
export function scheduler(retention: number) {
  return fsrs(
    generatorParameters({
      request_retention: Math.min(0.97, Math.max(0.7, retention)),
      enable_fuzz: true,
    }),
  );
}

export function loadCard(s: SavedCard | undefined, now: Date): Card {
  return s ? TypeConvert.card(s as unknown as Card) : createEmptyCard(now);
}

export function saveCard(c: Card): SavedCard {
  return {
    ...c,
    due: new Date(c.due).toISOString(),
    last_review: c.last_review ? new Date(c.last_review).toISOString() : undefined,
  };
}

const sameDay = (a: Date, b: Date) =>
  a.getFullYear() === b.getFullYear() &&
  a.getMonth() === b.getMonth() &&
  a.getDate() === b.getDate();

/** End of the local day: anything due by then counts as due today. */
export function endOfDay(now: Date) {
  const d = new Date(now);
  d.setHours(23, 59, 59, 999);
  return d;
}

/** What was done today. */
export function today(data: ReviewData, now: Date) {
  let reviewed = 0;
  let fresh = 0;
  for (const l of data.log) {
    if (!sameDay(new Date(l.t), now)) continue;
    reviewed++;
    if (l.new) fresh++;
  }
  return { reviewed, fresh };
}

/** Today's cards: those due (oldest first), then new ones (oldest highlight
 * first), within the day's limits. */
export function dueQueue(cards: ReviewCard[], data: ReviewData, now: Date) {
  const end = endOfDay(now).getTime();
  const done = today(data, now);
  const due: { c: ReviewCard; at: number }[] = [];
  const fresh: ReviewCard[] = [];
  for (const c of cards) {
    const s = data.sched[c.key];
    if (!s) fresh.push(c);
    else {
      const at = new Date(s.due).getTime();
      if (at <= end) due.push({ c, at });
    }
  }
  due.sort((a, b) => a.at - b.at);
  fresh.sort((a, b) => a.created.localeCompare(b.created) || a.key.localeCompare(b.key));
  const reviews = due
    .map((d) => d.c)
    .slice(0, Math.max(0, data.settings.maxReviews - done.reviewed));
  const news = fresh.slice(0, Math.max(0, data.settings.newPerDay - done.fresh));
  return { reviews, news, queue: [...reviews, ...news], waitingNew: fresh.length };
}

/** How many cards fall due on each of the next `days` days (today first). */
export function forecast(cards: ReviewCard[], data: ReviewData, now: Date, days = 7): number[] {
  const out = new Array<number>(days).fill(0);
  const start = new Date(now);
  start.setHours(0, 0, 0, 0);
  for (const c of cards) {
    const s = data.sched[c.key];
    if (!s) continue;
    const i = Math.floor((new Date(s.due).getTime() - start.getTime()) / 86_400_000);
    if (i < days) out[Math.max(0, i)]! += 1;
  }
  return out;
}

export const GRADES: { grade: Grade; name: string; key: string }[] = [
  { grade: Rating.Again, name: "Again", key: "1" },
  { grade: Rating.Hard, name: "Hard", key: "2" },
  { grade: Rating.Good, name: "Good", key: "3" },
  { grade: Rating.Easy, name: "Easy", key: "4" },
];

/** "10 min", "3 d", "2 mo": when the card comes back. */
export function interval(from: Date, to: Date): string {
  const min = Math.max(1, Math.round((to.getTime() - from.getTime()) / 60_000));
  if (min < 60) return `${min} min`;
  const h = Math.round(min / 60);
  if (h < 24) return `${h} h`;
  const d = Math.round(min / 1440);
  if (d < 30) return `${d} d`;
  if (d < 365) return `${Math.round(d / 30)} mo`;
  return `${(d / 365).toFixed(1).replace(/\.0$/, "")} y`;
}

/** When each answer would bring the card back. */
export function previews(card: ReviewCard, data: ReviewData, now: Date): Record<Grade, string> {
  const f = scheduler(data.settings.retention);
  const p = f.repeat(loadCard(data.sched[card.key], now), now);
  const out = {} as Record<Grade, string>;
  for (const { grade } of GRADES) out[grade] = interval(now, new Date(p[grade].card.due));
  return out;
}

/** Answers a card: its new schedule and a line in the log. */
export function answer(data: ReviewData, card: ReviewCard, grade: Grade, now: Date): ReviewData {
  const f = scheduler(data.settings.retention);
  const isNew = !data.sched[card.key];
  const next = f.next(loadCard(data.sched[card.key], now), now, grade);
  return {
    ...data,
    sched: { ...data.sched, [card.key]: saveCard(next.card) },
    log: [
      ...data.log.slice(-5000),
      { t: now.toISOString(), key: card.key, rating: grade, ...(isNew ? { new: true } : {}) },
    ],
  };
}

/** Days in a row with at least one review, up to today. */
export function streak(data: ReviewData, now: Date): number {
  const days = new Set(data.log.map((l) => new Date(l.t).toDateString()));
  let n = 0;
  const d = new Date(now);
  if (!days.has(d.toDateString())) d.setDate(d.getDate() - 1);
  while (days.has(d.toDateString())) {
    n++;
    d.setDate(d.getDate() - 1);
  }
  return n;
}

// ── What a card shows ───────────────────────────────────────────────────

const escape = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/** The passage's opening (about 45% of its words), to recall the rest. */
export function opening(quote: string): { start: string; rest: string } {
  const words = quote.split(/(\s+)/);
  const count = words.filter((w) => w.trim()).length;
  if (count < 4) return { start: quote, rest: "" };
  const keep = Math.max(2, Math.round(count * 0.45));
  let seen = 0;
  let i = 0;
  for (; i < words.length && seen < keep; i++) if (words[i]?.trim()) seen++;
  return { start: words.slice(0, i).join(""), rest: words.slice(i).join("") };
}

/** The passage cut into text and hidden words. */
export function clozeParts(quote: string, words: string[]): { text: string; hidden: boolean }[] {
  const ws = words.map((w) => w.trim()).filter(Boolean);
  if (!ws.length) return [{ text: quote, hidden: false }];
  const re = new RegExp(
    `(?<![\\p{L}\\p{N}])(${ws
      .sort((a, b) => b.length - a.length)
      .map(escape)
      .join("|")})(?![\\p{L}\\p{N}])`,
    "gu",
  );
  const out: { text: string; hidden: boolean }[] = [];
  let last = 0;
  for (const m of quote.matchAll(re)) {
    if (m.index > last) out.push({ text: quote.slice(last, m.index), hidden: false });
    out.push({ text: m[0], hidden: true });
    last = m.index + m[0].length;
  }
  if (last < quote.length) out.push({ text: quote.slice(last), hidden: false });
  return out;
}

/** The words of a passage someone can pick for a cloze. */
export function pickableWords(quote: string): string[] {
  return quote.match(/[\p{L}\p{N}][\p{L}\p{N}'’-]*/gu) ?? [];
}

// ── Anki ────────────────────────────────────────────────────────────────

const html = (s: string) =>
  s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;").replace(/\n/g, "<br>");

const tag = (s: string) =>
  s
    .normalize("NFKD")
    .replace(/[^\p{L}\p{N}]+/gu, "_")
    .replace(/^_|_$/g, "")
    .slice(0, 60) || "book";

/** The cards as Anki notes (Basic and Cloze). */
export function ankiNotes(cards: ReviewCard[]): AnkiNote[] {
  return cards.map((c) => {
    const source = html(c.label ? `${c.bookTitle} · ${c.label}` : c.bookTitle);
    const tags = ["libreri", tag(c.bookTitle)];
    if (c.kind === "cloze") {
      const text = clozeParts(c.quote, c.cloze)
        .map((p) => (p.hidden ? `{{c1::${html(p.text)}}}` : html(p.text)))
        .join("");
      return { id: c.key, model: "cloze", fields: [text, html(c.note ?? ""), source], tags };
    }
    if (c.kind === "qa")
      return {
        id: c.key,
        model: "basic",
        fields: [html(c.note ?? ""), html(c.quote), source],
        tags,
      };
    const { start } = opening(c.quote);
    return { id: c.key, model: "basic", fields: [`${html(start)} …`, html(c.quote), source], tags };
  });
}
