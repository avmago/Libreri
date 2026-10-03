/**
 * Edit pages: the pages as they will be, and what else changes,
 * kept in the interface until Save. Everything here is plain data so it
 * can be undone step by step and tested without a PDF.
 *
 * Boxes are fractions of a page (x, y, w, h, top-left origin). Crops are
 * of the page as shown after turning; redactions and corrections are of
 * the book's page as it is now (before any turning), because they are
 * applied first.
 */
import type { Correction, EditPlan, OutPage, Quality } from "@/lib/ipc";

export type Box = [x: number, y: number, w: number, h: number];

export type Source =
  | { kind: "page"; page: number }
  | { kind: "file"; file: number; page: number }
  | { kind: "blank"; width: number; height: number }
  | { kind: "picture"; src: string };

export interface EditPage {
  /** Stable id for React and drag and drop. */
  uid: string;
  source: Source;
  /** Clockwise, 0/90/180/270, on top of the page's own turning. */
  rotate: number;
  crop: Box | null;
}

/** Another PDF that pages come from: a path, or `book:<id>`. */
export interface OtherFile {
  ref: string;
  name: string;
  pages: number;
  /** Where the interface can show its pages (books in the library). */
  url?: string;
}

export interface EditState {
  pages: EditPage[];
  files: OtherFile[];
  /** Book page → boxes to black out. */
  redactions: Record<number, Box[]>;
  corrections: Correction[];
  compress: Quality | null;
  embedOcr: boolean;
}

let counter = 0;
export const uid = () => `p${Date.now().toString(36)}${(counter++).toString(36)}`;

export function initial(pageCount: number): EditState {
  return {
    pages: Array.from({ length: pageCount }, (_, i) => ({
      uid: `book-${i + 1}`,
      source: { kind: "page", page: i + 1 },
      rotate: 0,
      crop: null,
    })),
    files: [],
    redactions: {},
    corrections: [],
    compress: null,
    embedOcr: false,
  };
}

const norm = (deg: number) => ((deg % 360) + 360) % 360;

/** A crop box turned with its page (clockwise by `deg`). */
export function turnBox([x, y, w, h]: Box, deg: number): Box {
  switch (norm(deg)) {
    case 90:
      return [1 - y - h, x, h, w];
    case 180:
      return [1 - x - w, 1 - y - h, w, h];
    case 270:
      return [y, 1 - x - w, h, w];
    default:
      return [x, y, w, h];
  }
}

const round = (v: number) => Math.round(v * 10000) / 10000;
const roundBox = (b: Box): Box => b.map(round) as Box;

/** Moves the pages `uids` (in their current order) to before `index`. */
export function move(s: EditState, uids: string[], index: number): EditState {
  const set = new Set(uids);
  const moving = s.pages.filter((p) => set.has(p.uid));
  if (!moving.length) return s;
  const before = s.pages.slice(0, index).filter((p) => !set.has(p.uid));
  const after = s.pages.slice(index).filter((p) => !set.has(p.uid));
  return { ...s, pages: [...before, ...moving, ...after] };
}

export function rotate(s: EditState, uids: string[], deg: 90 | -90 | 180): EditState {
  const set = new Set(uids);
  return {
    ...s,
    pages: s.pages.map((p) =>
      set.has(p.uid)
        ? {
            ...p,
            rotate: norm(p.rotate + deg),
            crop: p.crop ? roundBox(turnBox(p.crop, deg)) : null,
          }
        : p,
    ),
  };
}

export function remove(s: EditState, uids: string[]): EditState {
  const set = new Set(uids);
  return { ...s, pages: s.pages.filter((p) => !set.has(p.uid)) };
}

export function duplicate(s: EditState, uids: string[]): EditState {
  const set = new Set(uids);
  return {
    ...s,
    pages: s.pages.flatMap((p) => (set.has(p.uid) ? [p, { ...p, uid: uid() }] : [p])),
  };
}

export function insert(s: EditState, index: number, sources: Source[]): EditState {
  const added = sources.map((source) => ({ uid: uid(), source, rotate: 0, crop: null }));
  return {
    ...s,
    pages: [...s.pages.slice(0, index), ...added, ...s.pages.slice(index)],
  };
}

/** Adds another PDF; returns its index and the new state. */
export function addFile(s: EditState, file: OtherFile): [number, EditState] {
  const at = s.files.findIndex((f) => f.ref === file.ref);
  if (at >= 0) return [at, s];
  return [s.files.length, { ...s, files: [...s.files, file] }];
}

export function setCrop(s: EditState, uids: string[], crop: Box | null): EditState {
  const set = new Set(uids);
  const full = crop && crop[2] > 0.995 && crop[3] > 0.995 && crop[0] < 0.005 && crop[1] < 0.005;
  return {
    ...s,
    pages: s.pages.map((p) =>
      set.has(p.uid) ? { ...p, crop: crop && !full ? roundBox(crop) : null } : p,
    ),
  };
}

export function setRedactions(s: EditState, page: number, boxes: Box[]): EditState {
  const redactions = { ...s.redactions };
  if (boxes.length) redactions[page] = boxes.map(roundBox);
  else delete redactions[page];
  return { ...s, redactions };
}

export function setCorrections(s: EditState, page: number, list: Correction[]): EditState {
  return {
    ...s,
    corrections: [...s.corrections.filter((c) => c.page !== page), ...list],
  };
}

/** Book pages kept (in any place), for "a page with redactions was deleted". */
export function keptBookPages(s: EditState): Set<number> {
  return new Set(s.pages.flatMap((p) => (p.source.kind === "page" ? [p.source.page] : [])));
}

function outPage(p: EditPage): OutPage {
  const crop = p.crop ?? null;
  switch (p.source.kind) {
    case "page":
      return { kind: "page", page: p.source.page, rotate: p.rotate, crop };
    case "file":
      return { kind: "file", file: p.source.file, page: p.source.page, rotate: p.rotate, crop };
    case "blank":
      return { kind: "blank", width: p.source.width, height: p.source.height };
    case "picture":
      return { kind: "picture", src: p.source.src };
  }
}

/** The plan for the Rust side, for all pages or just `only` (in order). */
export function toPlan(s: EditState, only?: string[]): EditPlan {
  const set = only ? new Set(only) : null;
  const pages = s.pages.filter((p) => !set || set.has(p.uid));
  const kept = new Set(pages.flatMap((p) => (p.source.kind === "page" ? [p.source.page] : [])));
  return {
    pages: pages.map(outPage),
    files: s.files.map((f) => f.ref),
    redactions: Object.entries(s.redactions)
      .filter(([page]) => kept.has(Number(page)))
      .map(([page, boxes]) => ({ page: Number(page), boxes, color: "#000000" })),
    corrections: s.corrections.filter((c) => kept.has(c.page)),
    compress: s.compress,
    embedOcr: s.embedOcr,
  };
}

/** Splits the pages into parts that start at `starts` (uids). */
export function splitAt(s: EditState, starts: string[]): string[][] {
  const set = new Set(starts);
  const parts: string[][] = [];
  for (const p of s.pages) {
    if (!parts.length || (set.has(p.uid) && parts[parts.length - 1]!.length)) parts.push([]);
    parts[parts.length - 1]!.push(p.uid);
  }
  return parts;
}

/** Splits into parts of `size` pages. */
export function splitEvery(s: EditState, size: number): string[][] {
  const n = Math.max(1, Math.floor(size));
  const parts: string[][] = [];
  s.pages.forEach((p, i) => {
    if (i % n === 0) parts.push([]);
    parts[parts.length - 1]!.push(p.uid);
  });
  return parts;
}

/** What will change, for people. Empty when nothing does. */
export function summary(s: EditState, pageCount: number): string[] {
  const out: string[] = [];
  const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
  const kept = keptBookPages(s);
  const removed = pageCount - kept.size;
  if (removed > 0) out.push(`${plural(removed, "page")} removed`);
  const added = s.pages.filter((p) => p.source.kind !== "page").length;
  const copies = s.pages.filter((p) => p.source.kind === "page").length - kept.size;
  if (added + copies > 0) out.push(`${plural(added + copies, "page")} added`);
  const bookOrder = s.pages.flatMap((p) => (p.source.kind === "page" ? [p.source.page] : []));
  const sorted = [...bookOrder].sort((a, b) => a - b);
  if (bookOrder.some((p, i) => p !== sorted[i])) out.push("pages reordered");
  const turned = s.pages.filter((p) => p.rotate).length;
  if (turned) out.push(`${plural(turned, "page")} turned`);
  const cropped = s.pages.filter((p) => p.crop).length;
  if (cropped) out.push(`${plural(cropped, "page")} cropped`);
  const boxes = Object.entries(s.redactions)
    .filter(([p]) => kept.has(Number(p)))
    .reduce((n, [, b]) => n + b.length, 0);
  if (boxes) out.push(`${plural(boxes, "redaction")}`);
  const fixes = s.corrections.filter((c) => kept.has(c.page)).length;
  if (fixes) out.push(`${plural(fixes, "text correction")}`);
  if (s.compress) out.push(`pictures made smaller (${s.compress})`);
  if (s.embedOcr) out.push("OCR text written into the PDF");
  return out;
}

/** Parses "1-3, 5, 8-" into page numbers (1-based) up to `max`. */
export function parseRanges(text: string, max: number): number[] {
  const out: number[] = [];
  // "1 - 3" is one range: join the dash to its numbers before splitting.
  const joined = text.replace(/\s*[-–—]\s*/g, "-");
  for (const part of joined.split(/[,;\s]+/).filter(Boolean)) {
    const m = /^(\d*)-(\d*)$/.exec(part);
    if (m) {
      const a = m[1] ? Number(m[1]) : 1;
      const b = m[2] ? Number(m[2]) : max;
      for (let p = Math.max(1, a); p <= Math.min(max, b); p++) out.push(p);
    } else if (/^\d+$/.test(part)) {
      const p = Number(part);
      if (p >= 1 && p <= max) out.push(p);
    } else {
      return [];
    }
  }
  return out;
}

/** Paper sizes for blank pages, in points. */
export const PAPER: { id: string; label: string; size: [number, number] }[] = [
  { id: "a4", label: "A4", size: [595.28, 841.89] },
  { id: "letter", label: "Letter", size: [612, 792] },
  { id: "a5", label: "A5", size: [419.53, 595.28] },
  { id: "legal", label: "Legal", size: [612, 1008] },
];
