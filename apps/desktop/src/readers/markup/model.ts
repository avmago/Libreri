/**
 * Markup on fixed pages (PDF, DjVu, comics): pen and highlighter ink,
 * shapes, text boxes, sticky notes, stamps, pictures and measurements.
 *
 * Every position is a fraction of the page as shown (0–1, top-left
 * origin), so marks stay put at any zoom. Widths and text sizes are
 * fractions of the page width. Items are stored as annotations of kind
 * "markup" whose locator is `{type:"markup", page, layer, item}`; the book
 * file is never changed (user, 2026-09-28).
 */

export type Pt = [x: number, y: number];
/** x, y, width, height as fractions of the page. */
export type Box = [x: number, y: number, w: number, h: number];

export type Dash = "solid" | "dashed" | "dotted";
export type Unit = "mm" | "cm" | "m" | "in" | "ft" | "pt" | "px";

/** Real-world length per page unit (PDF point, or pixel for images). */
export interface Scale {
  perUnit: number;
  unit: Unit;
}

export type MeasureKind = "distance" | "perimeter" | "area" | "angle";

interface Stroke {
  color: string;
  /** Fraction of the page width. */
  width: number;
  opacity: number;
}

export type MarkupItem =
  | ({ tool: "pen" | "highlighter"; points: [number, number, number][] } & Stroke)
  | ({ tool: "rect" | "ellipse"; box: Box; dash: Dash; fill: string | null } & Stroke)
  | ({ tool: "line" | "arrow"; from: Pt; to: Pt; dash: Dash } & Stroke)
  | {
      tool: "text";
      box: Box;
      text: string;
      color: string;
      /** Fraction of the page width. */
      size: number;
      font: "sans" | "serif" | "hand";
      background: string | null;
    }
  | { tool: "note"; at: Pt; color: string }
  | { tool: "stamp"; box: Box; text: string; color: string }
  | { tool: "image"; box: Box; src: string; signature: boolean }
  | ({
      tool: "measure";
      kind: MeasureKind;
      points: Pt[];
      /** Page size in page units when measured (points or pixels). */
      page: [number, number];
      scale: Scale;
    } & Stroke);

export type ToolName =
  | "select"
  | "pen"
  | "highlighter"
  | "eraser"
  | "rect"
  | "ellipse"
  | "line"
  | "arrow"
  | "text"
  | "note"
  | "stamp"
  | "image"
  | "measure";

/** One stored mark. `note` is the sticky note's text (or a comment on any mark). */
export interface Mark {
  id: string;
  page: number;
  layer: string;
  item: MarkupItem;
  note: string | null;
}

export interface MarkupLocator {
  type: "markup";
  page: number;
  layer: string;
  item: MarkupItem;
}

export const DEFAULT_LAYER = "Markup";

export const COLORS = ["#111827", "#dc2626", "#2563eb", "#16a34a", "#f59e0b", "#9333ea"];
export const HIGHLIGHTER_COLORS = ["#fde047", "#86efac", "#93c5fd", "#f9a8d4"];
/** Stroke widths offered, as fractions of the page width. */
export const WIDTHS = [0.0012, 0.0025, 0.004, 0.007, 0.012];
export const STAMPS = ["APPROVED", "REVIEWED", "DRAFT", "CONFIDENTIAL", "NOT APPROVED"];

export const round = (v: number) => Math.round(v * 10000) / 10000;

const clamp01 = (v: number) => Math.min(1, Math.max(0, v));

// ---------- geometry ----------

/** The rectangle a mark covers (fractions of the page). */
export function bounds(item: MarkupItem): Box {
  const pts = (list: Pt[]): Box => {
    const xs = list.map((p) => p[0]);
    const ys = list.map((p) => p[1]);
    const x0 = Math.min(...xs);
    const y0 = Math.min(...ys);
    return [x0, y0, Math.max(...xs) - x0, Math.max(...ys) - y0];
  };
  switch (item.tool) {
    case "pen":
    case "highlighter":
      return item.points.length ? pts(item.points.map(([x, y]) => [x, y])) : [0, 0, 0, 0];
    case "line":
    case "arrow":
      return pts([item.from, item.to]);
    case "note":
      return [item.at[0], item.at[1], 0.03, 0.03];
    case "measure":
      return item.points.length ? pts(item.points) : [0, 0, 0, 0];
    default:
      return item.box;
  }
}

/** Moves a mark by `dx, dy`, keeping it on the page. */
export function moved(item: MarkupItem, dx: number, dy: number): MarkupItem {
  const p = ([x, y]: Pt): Pt => [round(clamp01(x + dx)), round(clamp01(y + dy))];
  const b = ([x, y, w, h]: Box): Box => [round(clamp01(x + dx)), round(clamp01(y + dy)), w, h];
  switch (item.tool) {
    case "pen":
    case "highlighter":
      return { ...item, points: item.points.map(([x, y, pr]) => [...p([x, y]), pr]) };
    case "line":
    case "arrow":
      return { ...item, from: p(item.from), to: p(item.to) };
    case "note":
      return { ...item, at: p(item.at) };
    case "measure":
      return { ...item, points: item.points.map(p) };
    default:
      return { ...item, box: b(item.box) };
  }
}

/** Resizes a box-shaped mark by moving its bottom-right corner. */
export function resized(item: MarkupItem, to: Pt): MarkupItem {
  if (!("box" in item)) return item;
  const [x, y] = item.box;
  const w = Math.max(0.01, to[0] - x);
  let h = Math.max(0.01, to[1] - y);
  // Pictures keep their shape.
  if (item.tool === "image" && item.box[2] > 0) h = (w * item.box[3]) / item.box[2];
  return { ...item, box: [x, y, round(w), round(h)] };
}

function segDist(p: Pt, a: Pt, b: Pt, aspect: number): number {
  // Measured in page-width units so distances are the same in x and y.
  const ax = a[0];
  const ay = a[1] * aspect;
  const bx = b[0];
  const by = b[1] * aspect;
  const px = p[0];
  const py = p[1] * aspect;
  const dx = bx - ax;
  const dy = by - ay;
  const len = dx * dx + dy * dy;
  const t = len ? Math.max(0, Math.min(1, ((px - ax) * dx + (py - ay) * dy) / len)) : 0;
  return Math.hypot(px - (ax + t * dx), py - (ay + t * dy));
}

function inBox(p: Pt, [x, y, w, h]: Box, pad = 0): boolean {
  return p[0] >= x - pad && p[0] <= x + w + pad && p[1] >= y - pad && p[1] <= y + h + pad;
}

/**
 * Whether the point `p` touches the mark. `aspect` is the page's height
 * over its width; `tolerance` is in page widths.
 */
export function hits(item: MarkupItem, p: Pt, aspect: number, tolerance = 0.008): boolean {
  switch (item.tool) {
    case "pen":
    case "highlighter": {
      const reach = item.width / 2 + tolerance;
      const pts = item.points;
      if (pts.length === 1)
        return segDist(p, [pts[0]![0], pts[0]![1]], [pts[0]![0], pts[0]![1]], aspect) <= reach;
      for (let i = 1; i < pts.length; i++) {
        const a: Pt = [pts[i - 1]![0], pts[i - 1]![1]];
        const b: Pt = [pts[i]![0], pts[i]![1]];
        if (segDist(p, a, b, aspect) <= reach) return true;
      }
      return false;
    }
    case "line":
    case "arrow":
      return segDist(p, item.from, item.to, aspect) <= item.width / 2 + tolerance;
    case "rect": {
      if (item.fill && inBox(p, item.box)) return true;
      const [x, y, w, h] = item.box;
      const corners: Pt[] = [
        [x, y],
        [x + w, y],
        [x + w, y + h],
        [x, y + h],
      ];
      return corners.some(
        (c, i) => segDist(p, c, corners[(i + 1) % 4]!, aspect) <= tolerance + item.width / 2,
      );
    }
    case "ellipse": {
      const [x, y, w, h] = item.box;
      const rx = w / 2 || 1e-6;
      const ry = h / 2 || 1e-6;
      const nx = (p[0] - (x + rx)) / rx;
      const ny = (p[1] - (y + ry)) / ry;
      const r = Math.hypot(nx, ny);
      if (item.fill && r <= 1) return true;
      return Math.abs(r - 1) * Math.min(rx, ry * aspect) <= tolerance + item.width / 2;
    }
    case "measure":
      for (let i = 1; i < item.points.length; i++) {
        if (segDist(p, item.points[i - 1]!, item.points[i]!, aspect) <= tolerance + item.width / 2)
          return true;
      }
      return item.kind === "area" && item.points.length > 2 && inPolygon(p, item.points);
    case "note":
      return inBox(p, bounds(item), tolerance);
    default:
      return inBox(p, item.box, tolerance / 2);
  }
}

function inPolygon(p: Pt, poly: Pt[]): boolean {
  let inside = false;
  for (let i = 0, j = poly.length - 1; i < poly.length; j = i++) {
    const [xi, yi] = poly[i]!;
    const [xj, yj] = poly[j]!;
    if (yi > p[1] !== yj > p[1] && p[0] < ((xj - xi) * (p[1] - yi)) / (yj - yi) + xi)
      inside = !inside;
  }
  return inside;
}

// ---------- measuring ----------

const UNIT_PER_POINT: Record<Exclude<Unit, "px">, number> = {
  pt: 1,
  in: 1 / 72,
  ft: 1 / 864,
  mm: 25.4 / 72,
  cm: 2.54 / 72,
  m: 0.0254 / 72,
};

/** The scale of a page with no calibration: true size for PDFs, pixels otherwise. */
export function defaultScale(unitIsPoint: boolean, unit: Unit = "mm"): Scale {
  if (!unitIsPoint) return { perUnit: 1, unit: "px" };
  return { perUnit: unit === "px" ? 1 : UNIT_PER_POINT[unit], unit };
}

/** Calibrates from a line drawn over something of known length. */
export function calibrate(
  from: Pt,
  to: Pt,
  page: [number, number],
  length: number,
  unit: Unit,
): Scale {
  const d = Math.hypot((to[0] - from[0]) * page[0], (to[1] - from[1]) * page[1]);
  return { perUnit: d > 0 ? length / d : 1, unit };
}

/** Converts a scale to another unit (same physical size). */
export function convertScale(s: Scale, unit: Unit): Scale {
  if (s.unit === "px" || unit === "px") return { ...s, unit };
  const perPoint = s.perUnit / UNIT_PER_POINT[s.unit];
  return { perUnit: perPoint * UNIT_PER_POINT[unit], unit };
}

/** The measurement's value and a label such as "12.4 cm" or "38.2 cm²". */
export function measure(item: Extract<MarkupItem, { tool: "measure" }>): {
  value: number;
  label: string;
} {
  const [pw, ph] = item.page;
  const real = (a: Pt, b: Pt) =>
    Math.hypot((b[0] - a[0]) * pw, (b[1] - a[1]) * ph) * item.scale.perUnit;
  const pts = item.points;
  const fmt = (v: number) => (v >= 100 ? v.toFixed(0) : v >= 10 ? v.toFixed(1) : v.toFixed(2));
  const u = item.scale.unit;
  switch (item.kind) {
    case "distance": {
      const v = pts.length >= 2 ? real(pts[0]!, pts[1]!) : 0;
      return { value: v, label: `${fmt(v)} ${u}` };
    }
    case "perimeter": {
      let v = 0;
      for (let i = 1; i < pts.length; i++) v += real(pts[i - 1]!, pts[i]!);
      return { value: v, label: `${fmt(v)} ${u}` };
    }
    case "area": {
      let twice = 0;
      for (let i = 0; i < pts.length; i++) {
        const [x1, y1] = pts[i]!;
        const [x2, y2] = pts[(i + 1) % pts.length]!;
        twice += x1 * pw * (y2 * ph) - x2 * pw * (y1 * ph);
      }
      const v = (Math.abs(twice) / 2) * item.scale.perUnit * item.scale.perUnit;
      return { value: v, label: `${fmt(v)} ${u}²` };
    }
    case "angle": {
      if (pts.length < 3) return { value: 0, label: "0°" };
      const [a, o, b] = pts as [Pt, Pt, Pt];
      const v1 = [(a[0] - o[0]) * pw, (a[1] - o[1]) * ph];
      const v2 = [(b[0] - o[0]) * pw, (b[1] - o[1]) * ph];
      const cos =
        (v1[0]! * v2[0]! + v1[1]! * v2[1]!) /
        (Math.hypot(v1[0]!, v1[1]!) * Math.hypot(v2[0]!, v2[1]!) || 1);
      const v = (Math.acos(Math.max(-1, Math.min(1, cos))) * 180) / Math.PI;
      return { value: v, label: `${v.toFixed(1)}°` };
    }
  }
}

/** How many points a measurement needs before it is finished (0 = until double-click). */
export function pointsNeeded(kind: MeasureKind): number {
  return kind === "distance" ? 2 : kind === "angle" ? 3 : 0;
}

// ---------- snapping rough shapes ----------

/**
 * Turns a rough pen stroke into a clean line, rectangle or ellipse when it
 * clearly is one; otherwise returns null. `aspect` is page height / width.
 */
export function snapStroke(
  points: [number, number, number][],
  aspect: number,
): { tool: "line"; from: Pt; to: Pt } | { tool: "rect" | "ellipse"; box: Box } | null {
  if (points.length < 4) return null;
  const P: Pt[] = points.map(([x, y]) => [x, y * aspect]);
  const first = P[0]!;
  const last = P[P.length - 1]!;
  let length = 0;
  for (let i = 1; i < P.length; i++)
    length += Math.hypot(P[i]![0] - P[i - 1]![0], P[i]![1] - P[i - 1]![1]);
  if (length < 0.02) return null;
  const chord = Math.hypot(last[0] - first[0], last[1] - first[1]);
  const back = (p: Pt): Pt => [round(p[0]), round(p[1] / aspect)];

  // A line: every point close to the chord, and the stroke not much longer.
  if (chord > 0.03 && length < chord * 1.08) {
    const far = Math.max(...P.map((p) => segDist([p[0], p[1]], first, last, 1)));
    if (far < chord * 0.04) return { tool: "line", from: back(first), to: back(last) };
  }

  // A closed shape: the ends meet.
  if (chord > Math.max(0.03, length * 0.18)) return null;
  const xs = P.map((p) => p[0]);
  const ys = P.map((p) => p[1]);
  const x0 = Math.min(...xs);
  const y0 = Math.min(...ys);
  const w = Math.max(...xs) - x0;
  const h = Math.max(...ys) - y0;
  if (w < 0.02 || h < 0.02) return null;
  const size = Math.min(w, h);
  const cx = x0 + w / 2;
  const cy = y0 + h / 2;
  let rectErr = 0;
  let ellErr = 0;
  for (const [x, y] of P) {
    rectErr += Math.min(
      Math.abs(x - x0),
      Math.abs(x - x0 - w),
      Math.abs(y - y0),
      Math.abs(y - y0 - h),
    );
    const r = Math.hypot((x - cx) / (w / 2), (y - cy) / (h / 2));
    ellErr += Math.abs(r - 1) * (size / 2);
  }
  rectErr /= P.length * size;
  ellErr /= P.length * size;
  const box: Box = [round(x0), round(y0 / aspect), round(w), round(h / aspect)];
  if (rectErr < ellErr && rectErr < 0.06) return { tool: "rect", box };
  if (ellErr <= rectErr && ellErr < 0.06) return { tool: "ellipse", box };
  return null;
}

// ---------- people-facing names ----------

export function describe(m: Mark): string {
  const i = m.item;
  switch (i.tool) {
    case "pen":
      return "Pen";
    case "highlighter":
      return "Highlighter";
    case "rect":
      return "Rectangle";
    case "ellipse":
      return "Ellipse";
    case "line":
      return "Line";
    case "arrow":
      return "Arrow";
    case "text":
      return i.text.trim() ? `Text: ${i.text.trim().slice(0, 40)}` : "Text box";
    case "note":
      return m.note ? `Note: ${m.note.slice(0, 40)}` : "Sticky note";
    case "stamp":
      return `Stamp: ${i.text}`;
    case "image":
      return i.signature ? "Signature" : "Picture";
    case "measure":
      return `${i.kind[0]!.toUpperCase()}${i.kind.slice(1)}: ${measure(i).label}`;
  }
}

/** Reads a stored annotation's locator; null if it is not markup. */
export function parseLocator(locator: string): MarkupLocator | null {
  try {
    const v = JSON.parse(locator) as MarkupLocator;
    return v?.type === "markup" && v.item && typeof v.page === "number" ? v : null;
  } catch {
    return null;
  }
}
