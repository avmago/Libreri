/**
 * Clipping a figure from a page: the part of a page under a
 * rectangle drawn on the screen, as a picture for a canvas.
 */
import type { Rect } from "./types";

export interface PageClip {
  /** A data: URL (WebP where the web view can make it, otherwise PNG). */
  dataUrl: string;
  mimeType: string;
  /** Size on the screen when clipped (CSS pixels). */
  width: number;
  height: number;
  page: number;
  /** Where on the page, as fractions. */
  box: Rect;
}

export interface ClipSource {
  page: number;
  div: HTMLElement;
  /** The page's picture at its best resolution. */
  image: () => Promise<{ source: CanvasImageSource; width: number; height: number } | null>;
}

/** Longest side of a clipped picture, in pixels. */
const LONGEST = 1800;

/** Clips from the page under most of `rect`. */
export async function clipFrom(rect: DOMRect, pages: ClipSource[]): Promise<PageClip | null> {
  let best: { s: ClipSource; area: number; r: DOMRect } | null = null;
  for (const s of pages) {
    const r = s.div.getBoundingClientRect();
    const w = Math.min(rect.right, r.right) - Math.max(rect.left, r.left);
    const h = Math.min(rect.bottom, r.bottom) - Math.max(rect.top, r.top);
    if (w > 4 && h > 4 && (!best || w * h > best.area)) best = { s, area: w * h, r };
  }
  if (!best) return null;
  const { s, r } = best;
  const x0 = Math.max(rect.left, r.left);
  const y0 = Math.max(rect.top, r.top);
  const x1 = Math.min(rect.right, r.right);
  const y1 = Math.min(rect.bottom, r.bottom);
  const box: Rect = [
    (x0 - r.left) / r.width,
    (y0 - r.top) / r.height,
    (x1 - x0) / r.width,
    (y1 - y0) / r.height,
  ];
  const img = await s.image();
  if (!img) return null;
  const sx = box[0] * img.width;
  const sy = box[1] * img.height;
  const sw = box[2] * img.width;
  const sh = box[3] * img.height;
  const k = Math.min(1, LONGEST / Math.max(sw, sh));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(sw * k));
  canvas.height = Math.max(1, Math.round(sh * k));
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(img.source, sx, sy, sw, sh, 0, 0, canvas.width, canvas.height);
  let dataUrl = canvas.toDataURL("image/webp", 0.9);
  if (!dataUrl.startsWith("data:image/webp")) dataUrl = canvas.toDataURL("image/png");
  const mimeType = dataUrl.slice(5, dataUrl.indexOf(";"));
  return { dataUrl, mimeType, width: x1 - x0, height: y1 - y0, page: s.page, box };
}

/** A page picture shown with <img>, fetched again so it can be read. */
export async function imageOf(img: HTMLImageElement | undefined | null) {
  if (!img?.src) return null;
  try {
    const blob = await (await fetch(img.src)).blob();
    const bitmap = await createImageBitmap(blob);
    return { source: bitmap, width: bitmap.width, height: bitmap.height };
  } catch {
    return null;
  }
}
