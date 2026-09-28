/**
 * The Excalidraw editor, loaded only when a canvas opens (it is large).
 * Saving, paper, clipping from the page and reading handwriting live here
 * because they need Excalidraw's own functions.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import {
  CaptureUpdateAction,
  Excalidraw,
  convertToExcalidrawElements,
  exportToBlob,
  getCommonBounds,
  getSceneVersion,
  serializeAsJSON,
} from "@excalidraw/excalidraw";
import type {
  BinaryFileData,
  DataURL,
  ExcalidrawImperativeAPI,
  ExcalidrawInitialDataState,
} from "@excalidraw/excalidraw/types";
import type { FileId } from "@excalidraw/excalidraw/element/types";
import "@excalidraw/excalidraw/index.css";
import { paperStyle } from "./paper";
import type { Paper } from "./api";
import type { PageClip } from "@/readers";
import { bookUrl } from "@/lib/ipc";

declare global {
  interface Window {
    EXCALIDRAW_ASSET_PATH?: string | string[];
  }
}
// Fonts are served by Libreri, never a CDN: the ones shipped with the app,
// then extra ones downloaded in Settings › Writing (served from app data).
window.EXCALIDRAW_ASSET_PATH = ["/excalidraw/", bookUrl(".extras/")];

export interface CanvasHandle {
  /** Adds a clipped figure, linked back to its page. */
  addClip(clip: PageClip, link: string | null, label: string): Promise<void>;
  /** The selected handwriting as a PNG (base64), and where it is. */
  selectedInk(): Promise<{ png: string; below: { x: number; y: number } } | null>;
  /** Adds typed text on the canvas. */
  addText(text: string, at: { x: number; y: number }): Promise<void>;
  /** Writes now (before closing). */
  flush(): void;
}

export interface HostProps {
  content: string;
  paper: Paper;
  dark: boolean;
  onSave: (json: string) => void;
  onLink: (href: string) => void;
  onReady: (h: CanvasHandle) => void;
}

/** Excalidraw measures text when it is added: make sure its handwriting
 * font is there first, or the text box comes out too narrow. */
async function fontReady(size: number) {
  try {
    await document.fonts.load(`${size}px Excalifont`);
  } catch {
    /* measured with the fallback font */
  }
}

function blobToBase64(blob: Blob): Promise<string> {
  return new Promise((ok, fail) => {
    const r = new FileReader();
    r.onload = () => ok(String(r.result).split(",")[1] ?? "");
    r.onerror = () => fail(r.error);
    r.readAsDataURL(blob);
  });
}

export default function ExcalidrawHost({
  content,
  paper,
  dark,
  onSave,
  onLink,
  onReady,
}: HostProps) {
  const api = useRef<ExcalidrawImperativeAPI | null>(null);
  const [view, setView] = useState({ x: 0, y: 0, zoom: 1 });
  const saved = useRef<string>("");
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const save = useRef(onSave);
  const link = useRef(onLink);
  useEffect(() => {
    save.current = onSave;
    link.current = onLink;
  });

  const initial = useMemo<ExcalidrawInitialDataState>(() => {
    try {
      const doc = JSON.parse(content) as {
        elements?: ExcalidrawInitialDataState["elements"];
        appState?: Record<string, unknown>;
        files?: ExcalidrawInitialDataState["files"];
      };
      return {
        elements: doc.elements ?? [],
        appState: { ...doc.appState, viewBackgroundColor: "transparent" },
        files: doc.files ?? {},
        scrollToContent: true,
      };
    } catch {
      return { elements: [], appState: { viewBackgroundColor: "transparent" } };
    }
  }, [content]);

  const write = () => {
    const a = api.current;
    if (!a) return;
    const elements = a.getSceneElementsIncludingDeleted();
    const files = a.getFiles();
    const key = `${getSceneVersion(elements)}:${Object.keys(files).length}`;
    if (key === saved.current) return;
    saved.current = key;
    const appState = { ...a.getAppState(), viewBackgroundColor: "#ffffff" };
    save.current(serializeAsJSON(elements, appState, files, "local"));
  };

  useEffect(
    () => () => {
      clearTimeout(timer.current);
      write();
    },
    // Only when the editor goes away.
    [],
  );

  const handle: CanvasHandle = {
    async addClip(clip, href, label) {
      const a = api.current;
      if (!a) return;
      await fontReady(14);
      const id = crypto.randomUUID() as FileId;
      a.addFiles([
        {
          id,
          dataURL: clip.dataUrl as DataURL,
          mimeType: clip.mimeType as BinaryFileData["mimeType"],
          created: Date.now(),
        },
      ]);
      const st = a.getAppState();
      const k = Math.min(1, 420 / clip.width);
      const w = clip.width * k;
      const h = clip.height * k;
      const x = st.width / 2 / st.zoom.value - st.scrollX - w / 2;
      const y = st.height / 2 / st.zoom.value - st.scrollY - h / 2;
      const added = convertToExcalidrawElements([
        { type: "image", fileId: id, x, y, width: w, height: h, link: href },
        {
          type: "text",
          x,
          y: y + h + 6,
          text: label,
          fontSize: 14,
          strokeColor: "#64748b",
          link: href,
        },
      ]);
      a.updateScene({
        elements: [...a.getSceneElementsIncludingDeleted(), ...added],
        captureUpdate: CaptureUpdateAction.IMMEDIATELY,
      });
    },
    async selectedInk() {
      const a = api.current;
      if (!a) return null;
      const chosen = a.getAppState().selectedElementIds;
      const ink = a.getSceneElements().filter((e) => chosen[e.id] && e.type === "freedraw");
      if (!ink.length) return null;
      const blob = await exportToBlob({
        elements: ink,
        appState: {
          exportBackground: true,
          viewBackgroundColor: "#ffffff",
          exportWithDarkMode: false,
        },
        files: a.getFiles(),
        mimeType: "image/png",
        exportPadding: 32,
        getDimensions: (w: number, h: number) => ({ width: w * 2, height: h * 2, scale: 2 }),
      });
      const [minX, , , maxY] = getCommonBounds(ink);
      return { png: await blobToBase64(blob), below: { x: minX, y: maxY + 16 } };
    },
    async addText(text, at) {
      const a = api.current;
      if (!a) return;
      await fontReady(20);
      const added = convertToExcalidrawElements([
        { type: "text", x: at.x, y: at.y, text, fontSize: 20, strokeColor: "#1d4ed8" },
      ]);
      a.updateScene({
        elements: [...a.getSceneElementsIncludingDeleted(), ...added],
        captureUpdate: CaptureUpdateAction.IMMEDIATELY,
      });
    },
    flush() {
      clearTimeout(timer.current);
      write();
    },
  };
  const handleRef = useRef(handle);
  useEffect(() => {
    handleRef.current = handle;
  });

  return (
    <div
      className="lb-canvas relative size-full"
      style={paperStyle(paper, view.x, view.y, view.zoom, dark)}
    >
      <Excalidraw
        initialData={initial}
        theme={dark ? "dark" : "light"}
        excalidrawAPI={(a) => {
          api.current = a;
          saved.current = `${getSceneVersion(a.getSceneElementsIncludingDeleted())}:${Object.keys(a.getFiles()).length}`;
          onReady({
            addClip: (...args) => handleRef.current.addClip(...args),
            selectedInk: () => handleRef.current.selectedInk(),
            addText: (...args) => handleRef.current.addText(...args),
            flush: () => handleRef.current.flush(),
          });
        }}
        onChange={() => {
          clearTimeout(timer.current);
          timer.current = setTimeout(write, 700);
        }}
        onScrollChange={(x, y, zoom) => setView({ x, y, zoom: zoom.value })}
        onLinkOpen={(element, event) => {
          if (!element.link) return;
          event.preventDefault();
          link.current(element.link);
        }}
        UIOptions={{
          canvasActions: {
            loadScene: false,
            saveToActiveFile: false,
            export: false,
            toggleTheme: false,
            changeViewBackgroundColor: false,
          },
        }}
        langCode="en"
      />
    </div>
  );
}
