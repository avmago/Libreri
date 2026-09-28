/**
 * Markup mode in the reader: owns the page markup layer of the open book,
 * turns stored annotations into marks and marks back into annotations.
 */
import { useCallback, useEffect, useMemo, useRef, useState, type RefObject } from "react";
import { useProfilePrefs } from "@/features/profiles";
import { attachSpell } from "@/features/spell";
import { commands, unwrap, type Annotation } from "@/lib/ipc";
import {
  DEFAULT_STYLE,
  MarkupLayer,
  markupModel,
  type Mark,
  type MarkupStyle,
  type Pt,
  type Renderer,
  type Scale,
  type ToolName,
} from "@/readers";

const { bounds, parseLocator, DEFAULT_LAYER } = markupModel;

export function marksOf(annotations: Annotation[]): Mark[] {
  const out: Mark[] = [];
  for (const a of annotations) {
    if (a.kind !== "markup") continue;
    const loc = parseLocator(a.locator);
    if (loc)
      out.push({
        id: a.id,
        page: loc.page,
        layer: loc.layer || DEFAULT_LAYER,
        item: loc.item,
        note: a.note,
      });
  }
  return out;
}

export function annotationOf(m: Mark, bookId: string, pages: number): Annotation {
  const y = bounds(m.item)[1];
  return {
    id: m.id,
    bookId,
    kind: "markup",
    color: null,
    locator: JSON.stringify({ type: "markup", page: m.page, layer: m.layer, item: m.item }),
    quote: null,
    note: m.note,
    label: `p. ${m.page}`,
    position: pages > 0 ? Math.min(1, (m.page - 1 + y) / pages) : 0,
    createdAt: "",
    modifiedAt: "",
  };
}

const storageKey = (bookId: string, what: string) => `libreri.markup.${what}.${bookId}`;

function readLocal<T>(key: string, fallback: T): T {
  try {
    const v = localStorage.getItem(key);
    return v ? (JSON.parse(v) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeLocal(key: string, v: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(v));
  } catch {
    /* private window */
  }
}

export interface MarkupState {
  available: boolean;
  active: boolean;
  setActive: (on: boolean) => void;
  tool: ToolName;
  setTool: (t: ToolName) => void;
  style: MarkupStyle;
  setStyle: (s: Partial<MarkupStyle>) => void;
  canUndo: boolean;
  canRedo: boolean;
  undo: () => void;
  redo: () => void;
  selected: Mark | null;
  deleteSelected: () => void;
  marks: Mark[];
  hideAll: boolean;
  hiddenLayers: string[];
  setVisibility: (hideAll: boolean, hidden: string[]) => void;
  layer: string;
  setLayer: (name: string) => void;
  noteEdit: { mark: Mark; rect: DOMRect } | null;
  closeNote: () => void;
  saveNote: (mark: Mark, text: string) => void;
  deleteMark: (id: string) => void;
  selectMark: (id: string) => void;
  calibration: { from: Pt; to: Pt; page: number; size: [number, number] } | null;
  finishCalibration: (scale: Scale | null) => void;
  startCalibrate: () => void;
  needImage: boolean;
  clearNeedImage: () => void;
  pageAspect: (page: number) => number;
  visibleMarks: () => Mark[];
}

export function useMarkup(opts: {
  bookId: string;
  enabled: boolean;
  ready: boolean;
  isPdf: boolean;
  renderer: RefObject<Renderer | null>;
  annotations: Annotation[];
  pages: number;
  save: (a: Annotation) => void;
  remove: (id: string) => void;
}): MarkupState {
  const { bookId, enabled, ready, renderer, annotations, pages } = opts;
  const prefs = useProfilePrefs((s) => s.prefs.markup);
  const updatePrefs = useProfilePrefs((s) => s.update);
  const layerRef = useRef<MarkupLayer | null>(null);
  const [active, setActiveState] = useState(false);
  const [tool, setToolState] = useState<ToolName>("pen");
  const [style, setStyleState] = useState<MarkupStyle>(() => ({
    ...DEFAULT_STYLE,
    color: prefs.color,
    width: prefs.width,
    snap: prefs.snap,
    scale: readLocal<Scale | null>(storageKey(bookId, "scale"), null),
  }));
  const [history, setHistory] = useState({ undo: false, redo: false });
  const [selected, setSelected] = useState<Mark | null>(null);
  const [noteEdit, setNoteEdit] = useState<{ mark: Mark; rect: DOMRect } | null>(null);
  const [calibration, setCalibration] = useState<MarkupState["calibration"]>(null);
  const [needImage, setNeedImage] = useState(false);
  const [visibility, setVisibilityState] = useState(() =>
    readLocal(storageKey(bookId, "visibility"), { hideAll: false, hidden: [] as string[] }),
  );
  const [layerName, setLayerName] = useState(DEFAULT_LAYER);

  // Latest callbacks for the layer, which lives across renders.
  const io = useRef({ save: opts.save, remove: opts.remove, pages });
  useEffect(() => {
    io.current = { save: opts.save, remove: opts.remove, pages };
  });

  const marks = useMemo(() => marksOf(annotations), [annotations]);

  useEffect(() => {
    const r = renderer.current;
    if (!enabled || !ready || !r?.attachMarkup) return;
    const layer = new MarkupLayer(
      {
        onSave: (m) => io.current.save(annotationOf(m, bookId, io.current.pages)),
        onDelete: (id) => io.current.remove(id),
        onSelect: setSelected,
        onEditNote: (mark, rect) => setNoteEdit({ mark, rect }),
        onNeedImage: () => setNeedImage(true),
        onCalibrate: (from, to, page, size) => setCalibration({ from, to, page, size }),
        onHistory: (u, rd) => setHistory({ undo: u, redo: rd }),
        onEditorOpen: (area) => attachSpell(area, () => ({ bookId })),
      },
      (page) => r.pageSize?.(page) ?? null,
    );
    layerRef.current = layer;
    r.attachMarkup(layer);
    return () => {
      r.attachMarkup?.(null);
      layer.destroy();
      layerRef.current = null;
    };
  }, [enabled, ready, renderer, bookId]);

  // Stored marks → layer.
  useEffect(() => {
    layerRef.current?.setMarks(marks);
  }, [marks, ready]);

  useEffect(() => {
    const l = layerRef.current;
    if (!l) return;
    l.style = style;
    l.setVisibility(visibility.hideAll, visibility.hidden);
    l.layer = layerName;
  }, [style, visibility, layerName, ready]);

  useEffect(() => {
    layerRef.current?.setTool(active ? tool : null);
  }, [active, tool, ready]);

  // Scales from the PDF itself.
  useEffect(() => {
    if (!enabled || !ready || !opts.isPdf) return;
    let stop = false;
    void unwrap(commands.measureScales(bookId))
      .then((list) => {
        if (stop) return;
        layerRef.current?.setPageScales(
          new Map(
            list.map((s) => [s.page, { perUnit: s.perPoint ?? 1, unit: s.unit as Scale["unit"] }]),
          ),
        );
      })
      .catch(() => {});
    return () => {
      stop = true;
    };
  }, [enabled, ready, opts.isPdf, bookId]);

  const setStyle = useCallback(
    (s: Partial<MarkupStyle>) => {
      setStyleState((cur) => ({ ...cur, ...s }));
      layerRef.current?.setStyle(s);
      if (s.color || s.width || s.snap !== undefined)
        updatePrefs({
          markup: {
            ...(s.color ? { color: s.color } : {}),
            ...(s.width ? { width: s.width } : {}),
            ...(s.snap !== undefined ? { snap: s.snap } : {}),
          },
        });
      if (s.scale !== undefined) writeLocal(storageKey(bookId, "scale"), s.scale);
    },
    [bookId, updatePrefs],
  );

  const setVisibility = useCallback(
    (hideAll: boolean, hidden: string[]) => {
      const v = { hideAll, hidden };
      setVisibilityState(v);
      writeLocal(storageKey(bookId, "visibility"), v);
    },
    [bookId],
  );

  return {
    available: enabled && ready,
    active,
    setActive: (on) => {
      setActiveState(on);
      if (!on) setSelected(null);
    },
    tool,
    setTool: (t) => {
      setActiveState(true);
      setToolState(t);
    },
    style,
    setStyle,
    canUndo: history.undo,
    canRedo: history.redo,
    undo: () => layerRef.current?.undo(),
    redo: () => layerRef.current?.redo(),
    selected,
    deleteSelected: () => layerRef.current?.deleteSelected(),
    marks,
    hideAll: visibility.hideAll,
    hiddenLayers: visibility.hidden,
    setVisibility,
    layer: layerName,
    setLayer: setLayerName,
    noteEdit,
    closeNote: () => setNoteEdit(null),
    saveNote: (mark, text) => {
      layerRef.current?.update({ ...mark, note: text.trim() || null });
      setNoteEdit(null);
    },
    deleteMark: (id) => {
      const m = layerRef.current?.allMarks().find((x) => x.id === id);
      if (!m) return;
      layerRef.current?.select(id);
      layerRef.current?.deleteSelected();
    },
    selectMark: (id) => {
      setActiveState(true);
      setToolState("select");
      layerRef.current?.setTool("select");
      layerRef.current?.select(id);
    },
    calibration,
    finishCalibration: (scale) => {
      setCalibration(null);
      if (scale) setStyle({ scale });
    },
    startCalibrate: () => {
      setActiveState(true);
      setToolState("measure");
      layerRef.current?.startCalibrate();
    },
    needImage,
    clearNeedImage: () => setNeedImage(false),
    pageAspect: (p) => layerRef.current?.pageAspect(p) ?? 1.294,
    visibleMarks: () => layerRef.current?.visibleMarks() ?? [],
  };
}
