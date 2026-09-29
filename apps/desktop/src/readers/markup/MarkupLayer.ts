/**
 * Markup mode for fixed pages: one controller per open book. Renderers
 * call `mount(page, div)` whenever they (re)draw a page; the controller
 * keeps an SVG layer on it, draws the marks and, while a tool is chosen,
 * turns pointer input into new marks. Saving is the caller's job
 * (`onSave` / `onDelete`), so marks are stored like any annotation.
 */
import {
  DEFAULT_LAYER,
  defaultScale,
  hits,
  moved,
  pointsNeeded,
  resized,
  round,
  snapStroke,
  type Box,
  type Dash,
  type Mark,
  type MarkupItem,
  type MeasureKind,
  type Pt,
  type Scale,
  type ToolName,
} from "./model";
import { NS, W, fontFamily, inkPath, renderItem, renderSelection } from "./render";

export interface MarkupStyle {
  color: string;
  highlighter: string;
  width: number;
  opacity: number;
  dash: Dash;
  fill: boolean;
  snap: boolean;
  font: "sans" | "serif" | "hand";
  /** Text size as a fraction of the page width. */
  textSize: number;
  stamp: string;
  /** The picture or signature to place, with its width / height. */
  image: { src: string; aspect: number; signature: boolean } | null;
  measure: MeasureKind;
  /** Calibrated scale for this book, if any. */
  scale: Scale | null;
}

export const DEFAULT_STYLE: MarkupStyle = {
  color: "#dc2626",
  highlighter: "#fde047",
  width: 0.0025,
  opacity: 1,
  dash: "solid",
  fill: false,
  snap: true,
  font: "sans",
  textSize: 0.018,
  stamp: "APPROVED",
  image: null,
  measure: "distance",
  scale: null,
};

export interface MarkupEvents {
  onSave(mark: Mark): void;
  onDelete(id: string): void;
  onSelect(mark: Mark | null): void;
  /** A sticky note was placed or double-clicked: ask for its text. */
  onEditNote(mark: Mark, at: DOMRect): void;
  /** The picture tool was used with no picture chosen yet. */
  onNeedImage(): void;
  /** A calibration line was drawn: ask for its real length. */
  onCalibrate(from: Pt, to: Pt, page: number, size: [number, number]): void;
  /** Undo or redo became possible or impossible. */
  onHistory(canUndo: boolean, canRedo: boolean): void;
  /** A text box is being typed in (spell check attaches here). Returns
   * what to do when it closes. */
  onEditorOpen?(area: HTMLTextAreaElement): (() => void) | void;
}

interface PageView {
  div: HTMLElement;
  svg: SVGSVGElement;
  H: number;
}

type Op = { before: Mark | null; after: Mark | null };

type Gesture =
  | { kind: "ink"; page: number; points: [number, number, number][]; path: SVGPathElement }
  | {
      kind: "shape";
      page: number;
      tool: "rect" | "ellipse" | "line" | "arrow" | "text";
      from: Pt;
      to: Pt;
      preview: SVGGElement;
    }
  | { kind: "erase"; page: number; removed: Mark[] }
  | { kind: "move"; page: number; start: Pt; original: Mark; last: Mark }
  | { kind: "resize"; page: number; original: Mark; last: Mark }
  | { kind: "calibrate"; page: number; from: Pt; to: Pt; preview: SVGGElement };

const newId = () => crypto.randomUUID();

export class MarkupLayer {
  private marks = new Map<string, Mark>();
  private pages = new Map<number, PageView>();
  private tool: ToolName | null = null;
  style: MarkupStyle = { ...DEFAULT_STYLE };
  private selected: string | null = null;
  private hiddenLayers = new Set<string>();
  private hideAll = false;
  private undoStack: Op[][] = [];
  private redoStack: Op[][] = [];
  private gesture: Gesture | null = null;
  /** A measurement being drawn point by point. */
  private measuring: { page: number; points: Pt[]; preview: SVGGElement } | null = null;
  private editor: {
    id: string;
    area: HTMLTextAreaElement;
    isNew: boolean;
    detach?: (() => void) | void;
  } | null = null;
  /** Next drag with the measure tool calibrates instead. */
  private calibrating = false;
  layer = DEFAULT_LAYER;
  /** Scales pages carry themselves (PDF measure dictionaries). */
  private pageScales = new Map<number, Scale>();
  /** False while this book's tab is not the one in use: keys are left alone. */
  active = true;

  constructor(
    private readonly events: MarkupEvents,
    /** Page size in points (PDF) or pixels (images), and which it is. */
    private readonly pageSize: (page: number) => { size: [number, number]; points: boolean } | null,
  ) {
    window.addEventListener("keydown", this.onKey);
  }

  destroy() {
    window.removeEventListener("keydown", this.onKey);
    this.closeEditor(true);
    for (const v of this.pages.values()) v.svg.remove();
    this.pages.clear();
  }

  // ---------- pages ----------

  /** Called by the renderer each time a page is drawn. */
  mount(page: number, div: HTMLElement) {
    const rect = div.getBoundingClientRect();
    // A hidden page has no size: the renderer mounts it again once shown.
    if (rect.width <= 0 || rect.height <= 0) return;
    let svg = div.querySelector<SVGSVGElement>(":scope > svg.lb-markup");
    const aspect = rect.height / rect.width;
    const H = W * aspect;
    if (!svg) {
      svg = document.createElementNS(NS, "svg");
      svg.classList.add("lb-markup");
      svg.addEventListener("pointerdown", (e) => this.down(e, page));
      svg.addEventListener("pointermove", (e) => this.move(e, page));
      svg.addEventListener("pointerup", (e) => this.up(e, page));
      svg.addEventListener("dblclick", (e) => this.dblclick(e, page));
      div.append(svg);
    }
    svg.setAttribute("viewBox", `0 0 ${W} ${H}`);
    svg.setAttribute("preserveAspectRatio", "none");
    svg.classList.toggle("lb-markup-active", this.tool !== null);
    svg.dataset.tool = this.tool ?? "";
    this.pages.set(page, { div, svg, H });
    this.draw(page);
  }

  private draw(page: number) {
    const v = this.pages.get(page);
    if (!v || !v.svg.isConnected) return;
    // Keep what is being drawn right now.
    const live = [...v.svg.querySelectorAll(":scope > .lb-markup-preview")];
    v.svg.replaceChildren();
    if (this.hideAll) {
      v.svg.append(...live);
      return;
    }
    for (const m of this.marks.values()) {
      if (m.page !== page || this.hiddenLayers.has(m.layer)) continue;
      if (this.editor?.id === m.id) continue;
      const g = renderItem(m.item, v.H, m.note);
      g.dataset.mark = m.id;
      if (m.item.tool === "note") g.classList.add("lb-markup-note");
      v.svg.append(g);
    }
    const sel = this.selected ? this.marks.get(this.selected) : null;
    if (sel && sel.page === page && this.tool === "select") v.svg.append(renderSelection(sel, v.H));
    v.svg.append(...live);
  }

  redrawAll() {
    for (const p of this.pages.keys()) this.draw(p);
  }

  // ---------- state from outside ----------

  /** Replaces the marks with what is stored (keeps a text being edited). */
  setMarks(list: Mark[]) {
    const editing = this.editor ? this.marks.get(this.editor.id) : null;
    this.marks = new Map(list.map((m) => [m.id, m]));
    if (editing) this.marks.set(editing.id, editing);
    if (this.selected && !this.marks.has(this.selected)) this.select(null);
    this.redrawAll();
  }

  setTool(tool: ToolName | null) {
    if (tool === this.tool) return;
    this.finishMeasure(false);
    this.closeEditor(true);
    this.tool = tool;
    this.calibrating = false;
    if (tool !== "select") this.select(null);
    for (const v of this.pages.values()) {
      v.svg.classList.toggle("lb-markup-active", tool !== null);
      v.svg.dataset.tool = tool ?? "";
    }
    this.redrawAll();
  }

  getTool() {
    return this.tool;
  }

  setStyle(change: Partial<MarkupStyle>) {
    this.style = { ...this.style, ...change };
    // Changing the colour or width applies to the selected mark too.
    const sel = this.selected ? this.marks.get(this.selected) : null;
    if (sel && (change.color || change.width || change.opacity !== undefined || change.dash)) {
      const i = sel.item as MarkupItem & Record<string, unknown>;
      const next = { ...i } as Record<string, unknown>;
      if (change.color && "color" in i && i.tool !== "highlighter") next.color = change.color;
      if (change.width && "width" in i) next.width = change.width;
      if (change.opacity !== undefined && "opacity" in i) next.opacity = change.opacity;
      if (change.dash && "dash" in i) next.dash = change.dash;
      this.commit([{ before: sel, after: { ...sel, item: next as unknown as MarkupItem } }]);
    }
  }

  setVisibility(hideAll: boolean, hiddenLayers: string[]) {
    this.hideAll = hideAll;
    this.hiddenLayers = new Set(hiddenLayers);
    this.redrawAll();
  }

  setPageScales(scales: Map<number, Scale>) {
    this.pageScales = scales;
  }

  /** The scale a new measurement on `page` uses. */
  scaleFor(page: number): Scale {
    const size = this.pageSize(page);
    return this.style.scale ?? this.pageScales.get(page) ?? defaultScale(size?.points ?? false);
  }

  /** Height over width of a page as shown. */
  pageAspect(page: number): number {
    const v = this.pages.get(page);
    if (v) return v.H / W;
    const s = this.pageSize(page)?.size;
    return s && s[0] > 0 ? s[1] / s[0] : 1.294;
  }

  /** Every mark, for lists and export. */
  allMarks(): Mark[] {
    return [...this.marks.values()];
  }

  /** Marks that are shown (not on a hidden layer). */
  visibleMarks(): Mark[] {
    if (this.hideAll) return [];
    return this.allMarks().filter((m) => !this.hiddenLayers.has(m.layer));
  }

  /** Starts calibrating: the next line drawn with the measure tool sets the scale. */
  startCalibrate() {
    this.setTool("measure");
    this.calibrating = true;
  }

  select(id: string | null) {
    this.selected = id;
    this.events.onSelect(id ? (this.marks.get(id) ?? null) : null);
    this.redrawAll();
  }

  selectedMark(): Mark | null {
    return this.selected ? (this.marks.get(this.selected) ?? null) : null;
  }

  deleteSelected() {
    const m = this.selectedMark();
    if (!m) return;
    this.select(null);
    this.commit([{ before: m, after: null }]);
  }

  /** Saves a changed mark (e.g. a sticky note's text) with undo. */
  update(mark: Mark) {
    const before = this.marks.get(mark.id) ?? null;
    this.commit([{ before, after: mark }]);
  }

  // ---------- undo ----------

  private apply(ops: Op[], reverse: boolean) {
    for (const op of reverse ? [...ops].reverse() : ops) {
      const target = reverse ? op.before : op.after;
      const id = (op.after ?? op.before)!.id;
      if (target) {
        this.marks.set(id, target);
        this.events.onSave(target);
      } else {
        this.marks.delete(id);
        if (this.selected === id) this.select(null);
        this.events.onDelete(id);
      }
    }
    const pages = new Set(ops.flatMap((o) => [o.before?.page, o.after?.page]));
    for (const p of pages) if (p) this.draw(p);
  }

  private commit(ops: Op[]) {
    if (!ops.length) return;
    this.apply(ops, false);
    this.undoStack.push(ops);
    if (this.undoStack.length > 200) this.undoStack.shift();
    this.redoStack = [];
    this.events.onHistory(true, false);
  }

  undo() {
    const ops = this.undoStack.pop();
    if (!ops) return;
    this.apply(ops, true);
    this.redoStack.push(ops);
    this.events.onHistory(this.undoStack.length > 0, true);
  }

  redo() {
    const ops = this.redoStack.pop();
    if (!ops) return;
    this.apply(ops, false);
    this.undoStack.push(ops);
    this.events.onHistory(true, this.redoStack.length > 0);
  }

  // ---------- pointer input ----------

  private at(e: PointerEvent | MouseEvent, page: number): Pt {
    const r = this.pages.get(page)!.svg.getBoundingClientRect();
    return [
      Math.min(1, Math.max(0, (e.clientX - r.left) / r.width)),
      Math.min(1, Math.max(0, (e.clientY - r.top) / r.height)),
    ];
  }

  private aspect(page: number) {
    const v = this.pages.get(page);
    return v ? v.H / W : 1.294;
  }

  private newMark(page: number, item: MarkupItem, note: string | null = null): Mark {
    return { id: newId(), page, layer: this.layer, item, note };
  }

  /** The top-most visible mark under the point. */
  private hitTest(page: number, p: Pt): Mark | null {
    const list = [...this.marks.values()].filter(
      (m) => m.page === page && !this.hiddenLayers.has(m.layer),
    );
    for (let i = list.length - 1; i >= 0; i--) {
      if (hits(list[i]!.item, p, this.aspect(page))) return list[i]!;
    }
    return null;
  }

  private preview(page: number, g: SVGGElement) {
    g.classList.add("lb-markup-preview");
    this.pages.get(page)?.svg.append(g);
    return g;
  }

  private down(e: PointerEvent, page: number) {
    // Reading: sticky notes can be opened.
    if (!this.tool) {
      const g = (e.target as Element).closest?.("[data-mark]");
      const m = g ? this.marks.get((g as SVGElement).dataset.mark!) : null;
      if (m?.item.tool === "note") {
        e.preventDefault();
        this.events.onEditNote(m, new DOMRect(e.clientX, e.clientY, 1, 1));
      }
      return;
    }
    if (e.button !== 0) return;
    // Two fingers or a mouse wheel click pan instead.
    if (e.pointerType === "touch" && !e.isPrimary) return;
    const p = this.at(e, page);
    const v = this.pages.get(page)!;
    if (this.editor) {
      this.closeEditor(true);
      return;
    }
    e.preventDefault();
    const s = this.style;
    switch (this.tool) {
      case "pen":
      case "highlighter": {
        v.svg.setPointerCapture(e.pointerId);
        const path = document.createElementNS(NS, "path");
        path.setAttribute("fill", this.tool === "pen" ? s.color : s.highlighter);
        path.setAttribute("fill-opacity", String(this.tool === "pen" ? s.opacity : 0.4));
        this.preview(page, path as unknown as SVGGElement);
        this.gesture = {
          kind: "ink",
          page,
          points: [[round(p[0]), round(p[1]), pressure(e)]],
          path,
        };
        break;
      }
      case "eraser": {
        v.svg.setPointerCapture(e.pointerId);
        this.gesture = { kind: "erase", page, removed: [] };
        this.erase(page, p);
        break;
      }
      case "rect":
      case "ellipse":
      case "line":
      case "arrow":
      case "text": {
        v.svg.setPointerCapture(e.pointerId);
        const g = document.createElementNS(NS, "g");
        this.gesture = {
          kind: "shape",
          page,
          tool: this.tool,
          from: p,
          to: p,
          preview: this.preview(page, g),
        };
        break;
      }
      case "note": {
        const mark = this.newMark(page, {
          tool: "note",
          at: [round(p[0] - 0.012), round(p[1] - 0.012)],
          color: "#fde047",
        });
        this.commit([{ before: null, after: mark }]);
        this.events.onEditNote(mark, new DOMRect(e.clientX, e.clientY, 1, 1));
        break;
      }
      case "stamp": {
        const w = 0.26;
        const h = (w * 0.24) / this.aspect(page);
        const box: Box = [round(p[0] - w / 2), round(p[1] - h / 2), w, round(h)];
        this.commit([
          {
            before: null,
            after: this.newMark(page, {
              tool: "stamp",
              box,
              text: s.stamp,
              color: stampColor(s.stamp),
            }),
          },
        ]);
        break;
      }
      case "image": {
        if (!s.image) {
          this.events.onNeedImage();
          return;
        }
        const w = s.image.signature ? 0.24 : 0.3;
        const h = w / s.image.aspect / this.aspect(page);
        const box: Box = [round(p[0] - w / 2), round(p[1] - h / 2), w, round(h)];
        const mark = this.newMark(page, {
          tool: "image",
          box,
          src: s.image.src,
          signature: s.image.signature,
        });
        this.commit([{ before: null, after: mark }]);
        this.setTool("select");
        this.select(mark.id);
        break;
      }
      case "measure": {
        if (this.calibrating) {
          v.svg.setPointerCapture(e.pointerId);
          const g = document.createElementNS(NS, "g");
          this.gesture = {
            kind: "calibrate",
            page,
            from: p,
            to: p,
            preview: this.preview(page, g),
          };
          return;
        }
        if (this.measuring && this.measuring.page !== page) this.finishMeasure(false);
        if (!this.measuring) {
          const g = document.createElementNS(NS, "g");
          this.measuring = { page, points: [], preview: this.preview(page, g) };
        }
        this.measuring.points.push([round(p[0]), round(p[1])]);
        const need = pointsNeeded(s.measure);
        if (need && this.measuring.points.length >= need) this.finishMeasure(true);
        else this.drawMeasurePreview(p);
        break;
      }
      case "select": {
        const sel = this.selectedMark();
        const target = e.target as Element;
        if (sel && sel.page === page && target.classList.contains("lb-markup-handle")) {
          v.svg.setPointerCapture(e.pointerId);
          this.gesture = { kind: "resize", page, original: sel, last: sel };
          return;
        }
        const hit = this.hitTest(page, p);
        this.select(hit?.id ?? null);
        if (hit) {
          v.svg.setPointerCapture(e.pointerId);
          this.gesture = { kind: "move", page, start: p, original: hit, last: hit };
        }
        break;
      }
    }
  }

  private move(e: PointerEvent, page: number) {
    const g = this.gesture;
    if (this.measuring && this.measuring.page === page && !g) {
      this.drawMeasurePreview(this.at(e, page));
      return;
    }
    if (!g || g.page !== page) return;
    const p = this.at(e, page);
    const v = this.pages.get(page)!;
    switch (g.kind) {
      case "ink": {
        const events = typeof e.getCoalescedEvents === "function" ? e.getCoalescedEvents() : [e];
        for (const ev of events.length ? events : [e]) {
          const q = this.at(ev, page);
          g.points.push([round(q[0]), round(q[1]), pressure(ev)]);
        }
        const hl = this.tool === "highlighter";
        const width = hl ? Math.max(this.style.width * 6, 0.018) : this.style.width;
        g.path.setAttribute("d", inkPath(g.points, width, hl, v.H));
        break;
      }
      case "erase":
        this.erase(page, p);
        break;
      case "shape":
      case "calibrate": {
        g.to = e.shiftKey ? constrain(g.from, p, this.aspect(page)) : p;
        g.preview.replaceChildren(this.shapeItemPreview(g, v.H));
        break;
      }
      case "move": {
        const dx = p[0] - g.start[0];
        const dy = p[1] - g.start[1];
        g.last = { ...g.original, item: moved(g.original.item, dx, dy) };
        this.drawGhost(page, g.last);
        break;
      }
      case "resize": {
        g.last = { ...g.original, item: resized(g.original.item, p) };
        this.drawGhost(page, g.last);
        break;
      }
    }
  }

  private up(_e: PointerEvent, page: number) {
    const g = this.gesture;
    this.gesture = null;
    if (!g || g.page !== page) return;
    const s = this.style;
    switch (g.kind) {
      case "ink": {
        g.path.remove();
        const hl = this.tool === "highlighter";
        let points = g.points;
        if (points.length === 1)
          points = [points[0]!, [points[0]![0] + 0.0005, points[0]![1], points[0]![2]]];
        let item: MarkupItem = hl
          ? {
              tool: "highlighter",
              points,
              color: s.highlighter,
              width: Math.max(s.width * 6, 0.018),
              opacity: 0.4,
            }
          : { tool: "pen", points, color: s.color, width: s.width, opacity: s.opacity };
        if (!hl && s.snap) {
          const snapped = snapStroke(points, this.aspect(page));
          if (snapped?.tool === "line")
            item = {
              tool: "line",
              from: snapped.from,
              to: snapped.to,
              color: s.color,
              width: s.width,
              opacity: s.opacity,
              dash: s.dash,
            };
          else if (snapped)
            item = {
              tool: snapped.tool,
              box: snapped.box,
              color: s.color,
              width: s.width,
              opacity: s.opacity,
              dash: s.dash,
              fill: null,
            };
        }
        this.commit([{ before: null, after: this.newMark(page, item) }]);
        break;
      }
      case "erase":
        if (g.removed.length) {
          const ops = g.removed.map((m) => ({ before: m, after: null }));
          this.undoStack.push(ops);
          this.redoStack = [];
          this.events.onHistory(true, false);
        }
        break;
      case "shape": {
        g.preview.remove();
        const item = this.shapeItem(g);
        if (!item) break;
        const mark = this.newMark(page, item);
        this.commit([{ before: null, after: mark }]);
        if (item.tool === "text") this.openEditor(mark, true);
        break;
      }
      case "calibrate": {
        g.preview.remove();
        this.calibrating = false;
        const size = this.pageSize(page)?.size ?? [1, 1];
        if (Math.hypot(g.to[0] - g.from[0], g.to[1] - g.from[1]) > 0.01)
          this.events.onCalibrate(g.from, g.to, page, size);
        break;
      }
      case "move":
      case "resize":
        if (JSON.stringify(g.last.item) !== JSON.stringify(g.original.item))
          this.commit([{ before: g.original, after: g.last }]);
        else this.drawGhost(page, g.original);
        break;
    }
  }

  private dblclick(e: MouseEvent, page: number) {
    if (this.tool === "measure" && this.measuring) {
      // The double-click added a point twice.
      this.measuring.points.pop();
      this.finishMeasure(true);
      return;
    }
    if (this.tool !== "select") return;
    const hit = this.hitTest(page, this.at(e, page));
    if (hit?.item.tool === "text") this.openEditor(hit, false);
    else if (hit?.item.tool === "note")
      this.events.onEditNote(hit, new DOMRect(e.clientX, e.clientY, 1, 1));
  }

  /** True when some page of this layer is on screen (not in a hidden tab). */
  private shown(): boolean {
    for (const v of this.pages.values()) {
      if (v.div.isConnected && v.div.offsetParent !== null) return true;
    }
    return false;
  }

  private onKey = (e: KeyboardEvent) => {
    if (!this.tool || !this.active || !this.shown()) return;
    const t = e.target as HTMLElement | null;
    if (
      t &&
      (t.tagName === "INPUT" ||
        t.tagName === "TEXTAREA" ||
        t.tagName === "SELECT" ||
        t.isContentEditable)
    )
      return;
    if (e.key === "Escape") {
      if (this.measuring) this.finishMeasure(false);
      else if (this.selected) this.select(null);
      else return;
      e.preventDefault();
      e.stopPropagation();
    } else if (e.key === "Enter" && this.measuring) {
      e.preventDefault();
      this.finishMeasure(true);
    } else if ((e.key === "Delete" || e.key === "Backspace") && this.selected) {
      e.preventDefault();
      this.deleteSelected();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "z") {
      e.preventDefault();
      if (e.shiftKey) this.redo();
      else this.undo();
    } else if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "y") {
      e.preventDefault();
      this.redo();
    }
  };

  // ---------- helpers ----------

  private erase(page: number, p: Pt) {
    const g = this.gesture;
    if (g?.kind !== "erase") return;
    const hit = this.hitTest(page, p);
    if (!hit) return;
    g.removed.push(hit);
    this.marks.delete(hit.id);
    this.events.onDelete(hit.id);
    this.draw(page);
  }

  /** Shows a mark being moved or resized (saved when the drag ends). */
  private drawGhost(page: number, mark: Mark) {
    this.marks.set(mark.id, mark);
    this.draw(page);
  }

  private shapeItem(g: Extract<Gesture, { kind: "shape" }>): MarkupItem | null {
    const s = this.style;
    const [x0, y0] = g.from;
    const [x1, y1] = g.to;
    const box: Box = [
      round(Math.min(x0, x1)),
      round(Math.min(y0, y1)),
      round(Math.abs(x1 - x0)),
      round(Math.abs(y1 - y0)),
    ];
    const tiny = box[2] < 0.01 && box[3] < 0.01;
    switch (g.tool) {
      case "text": {
        const b: Box = tiny
          ? [box[0], box[1], 0.3, round((s.textSize * 2.2) / this.aspect(g.page))]
          : box;
        return {
          tool: "text",
          box: b,
          text: "",
          color: s.color,
          size: s.textSize,
          font: s.font,
          background: null,
        };
      }
      case "rect":
      case "ellipse":
        if (tiny) return null;
        return {
          tool: g.tool,
          box,
          color: s.color,
          width: s.width,
          opacity: s.opacity,
          dash: s.dash,
          fill: s.fill ? s.color : null,
        };
      case "line":
      case "arrow":
        if (tiny) return null;
        return {
          tool: g.tool,
          from: [round(x0), round(y0)],
          to: [round(x1), round(y1)],
          color: s.color,
          width: s.width,
          opacity: s.opacity,
          dash: s.dash,
        };
    }
  }

  private shapeItemPreview(
    g: Extract<Gesture, { kind: "shape" | "calibrate" }>,
    H: number,
  ): SVGElement {
    if (g.kind === "calibrate") {
      const grp = document.createElementNS(NS, "g");
      const line = document.createElementNS(NS, "line");
      for (const [k, val] of Object.entries({
        x1: g.from[0] * W,
        y1: g.from[1] * H,
        x2: g.to[0] * W,
        y2: g.to[1] * H,
        stroke: "#2563eb",
        "stroke-width": 2,
        "stroke-dasharray": "6 4",
      }))
        line.setAttribute(k, String(val));
      grp.append(line);
      return grp;
    }
    const item = this.shapeItem(g);
    if (!item) return document.createElementNS(NS, "g");
    if (item.tool === "text") {
      const r = document.createElementNS(NS, "rect");
      const [x, y, w, h] = item.box;
      for (const [k, val] of Object.entries({
        x: x * W,
        y: y * H,
        width: w * W,
        height: h * H,
        fill: "none",
        stroke: "#2563eb",
        "stroke-dasharray": "5 4",
      }))
        r.setAttribute(k, String(val));
      return r;
    }
    return renderItem(item, H);
  }

  private drawMeasurePreview(cursor: Pt) {
    const m = this.measuring;
    const v = m ? this.pages.get(m.page) : null;
    if (!m || !v) return;
    const size = this.pageSize(m.page);
    const item: MarkupItem = {
      tool: "measure",
      kind: this.style.measure,
      points: [...m.points, cursor],
      page: size?.size ?? [1, 1],
      scale: this.scaleFor(m.page),
      color: "#2563eb",
      width: 0.0015,
      opacity: 1,
    };
    m.preview.replaceChildren(renderItem(item, v.H));
  }

  private finishMeasure(save: boolean) {
    const m = this.measuring;
    if (!m) return;
    this.measuring = null;
    m.preview.remove();
    const kind = this.style.measure;
    const min = kind === "area" ? 3 : kind === "angle" ? 3 : 2;
    if (!save || m.points.length < min) return;
    const size = this.pageSize(m.page);
    this.commit([
      {
        before: null,
        after: this.newMark(m.page, {
          tool: "measure",
          kind,
          points: m.points,
          page: size?.size ?? [1, 1],
          scale: this.scaleFor(m.page),
          color: "#2563eb",
          width: 0.0015,
          opacity: 1,
        }),
      },
    ]);
  }

  // ---------- text boxes ----------

  private openEditor(mark: Mark, isNew: boolean) {
    if (mark.item.tool !== "text") return;
    const v = this.pages.get(mark.page);
    if (!v) return;
    this.closeEditor(true);
    const item = mark.item;
    const area = document.createElement("textarea");
    area.className = "lb-markup-editor";
    const [x, y, w, h] = item.box;
    const scale = v.div.clientWidth / W;
    area.style.cssText = `left:${x * 100}%;top:${y * 100}%;width:${w * 100}%;min-height:${h * 100}%;font-size:${item.size * W * scale}px;font-family:${fontFamily(item.font)};color:${item.color}`;
    area.value = item.text;
    area.setAttribute("aria-label", "Text box");
    area.addEventListener("keydown", (e) => {
      e.stopPropagation();
      if (e.key === "Escape") {
        e.preventDefault();
        this.closeEditor(true);
      }
    });
    area.addEventListener("input", () => {
      area.style.height = "auto";
      area.style.height = `${area.scrollHeight}px`;
    });
    area.addEventListener("blur", () => this.closeEditor(true));
    v.div.append(area);
    this.editor = { id: mark.id, area, isNew };
    this.draw(mark.page);
    requestAnimationFrame(() => {
      area.focus();
      if (this.editor?.area === area) this.editor.detach = this.events.onEditorOpen?.(area);
    });
  }

  private closeEditor(save: boolean) {
    const ed = this.editor;
    if (!ed) return;
    this.editor = null;
    const mark = this.marks.get(ed.id);
    ed.detach?.();
    ed.area.remove();
    if (!mark || mark.item.tool !== "text") return;
    const text = ed.area.value.replace(/\s+$/, "");
    const v = this.pages.get(mark.page);
    if (!save || (!text && ed.isNew)) {
      if (!text) {
        this.marks.delete(mark.id);
        this.events.onDelete(mark.id);
        // The creation is no longer something to undo.
        const last = this.undoStack[this.undoStack.length - 1];
        if (last?.length === 1 && last[0]!.after?.id === mark.id) this.undoStack.pop();
      }
      this.draw(mark.page);
      return;
    }
    // Grow the box to fit what was typed.
    const heightPx = ed.area.scrollHeight;
    const hFrac = v ? heightPx / v.div.clientHeight : mark.item.box[3];
    const box: Box = [
      mark.item.box[0],
      mark.item.box[1],
      mark.item.box[2],
      round(Math.max(mark.item.box[3], hFrac)),
    ];
    if (text === mark.item.text && box[3] === mark.item.box[3]) {
      this.draw(mark.page);
      return;
    }
    this.commit([{ before: mark, after: { ...mark, item: { ...mark.item, text, box } } }]);
  }
}

function pressure(e: PointerEvent): number {
  return e.pointerType === "pen" && e.pressure > 0 ? Math.round(e.pressure * 100) / 100 : 0.5;
}

/** Shift held: straight lines at 45° steps. */
function constrain(from: Pt, to: Pt, aspect: number): Pt {
  const dx = to[0] - from[0];
  const dy = (to[1] - from[1]) * aspect;
  const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4);
  const len = Math.hypot(dx, dy);
  return [
    round(from[0] + len * Math.cos(angle)),
    round(from[1] + (len * Math.sin(angle)) / aspect),
  ];
}

export function stampColor(text: string): string {
  const t = text.toUpperCase();
  if (t.includes("NOT") || t.includes("REJECT") || t.includes("CONFIDENTIAL")) return "#dc2626";
  if (t.includes("APPROVED")) return "#16a34a";
  if (t.includes("DRAFT")) return "#6b7280";
  return "#2563eb";
}
