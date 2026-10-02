/**
 * Bionic reading on pages that are pictures (PDF, DjVu, scans with OCR):
 * the start of each word is thickened on the page itself. The words come
 * from the page's text layer (PDF text, a DjVu text layer or OCR words);
 * each word start is copied from the page picture a pixel or two to the
 * right and darkened into place, on a canvas laid over the page. The page
 * picture itself is never changed, so turning it off just removes the layer.
 */
import { boldLength } from "./focus";

const LAYER = "lb-bionic-page";
const WORD = /[\p{L}\p{N}][\p{L}\p{N}\p{M}'’]*/gu;

/** The drawn page: PDF.js's canvas, or the page image. */
function pictureOf(page: HTMLElement): HTMLCanvasElement | HTMLImageElement | null {
  return (
    page.querySelector<HTMLCanvasElement>(".canvasWrapper canvas") ??
    page.querySelector<HTMLImageElement>("img.lb-page-img, img")
  );
}

/** Text nodes of the page's text layers. */
function textNodes(page: HTMLElement): Text[] {
  const out: Text[] = [];
  for (const layer of page.querySelectorAll<HTMLElement>(".textLayer, .lb-page-text")) {
    const walk = document.createTreeWalker(layer, NodeFilter.SHOW_TEXT);
    for (let n = walk.nextNode(); n; n = walk.nextNode()) out.push(n as Text);
  }
  return out;
}

export function clearPageBionic(page: HTMLElement) {
  page.querySelector(`canvas.${LAYER}`)?.remove();
}

/**
 * Draws (or draws again) the thickened word starts on one page. Returns
 * false when the page has no words or no picture yet.
 */
export function drawPageBionic(page: HTMLElement, fixation: number): boolean {
  clearPageBionic(page);
  const pic = pictureOf(page);
  if (!pic) return false;
  if (pic instanceof HTMLImageElement && !pic.complete) {
    pic.addEventListener("load", () => drawPageBionic(page, fixation), { once: true });
    return false;
  }
  const nodes = textNodes(page);
  if (!nodes.length) return false;
  const box = pic.getBoundingClientRect();
  const pageBox = page.getBoundingClientRect();
  if (!box.width || !box.height) return false;
  const srcW = pic instanceof HTMLCanvasElement ? pic.width : pic.naturalWidth;
  const srcH = pic instanceof HTMLCanvasElement ? pic.height : pic.naturalHeight;
  if (!srcW || !srcH) return false;
  // Drawn at the picture's own pixels (sharp on Retina), shown at its size.
  const sx = srcW / box.width;
  const sy = srcH / box.height;
  const layer = document.createElement("canvas");
  layer.className = LAYER;
  layer.width = srcW;
  layer.height = srcH;
  layer.setAttribute("aria-hidden", "true");
  layer.style.cssText = `position:absolute;pointer-events:none;z-index:2;left:${box.left - pageBox.left - page.clientLeft}px;top:${box.top - pageBox.top - page.clientTop}px;width:${box.width}px;height:${box.height}px`;
  const ctx = layer.getContext("2d");
  if (!ctx) return false;
  ctx.globalCompositeOperation = "darken";
  let drawn = 0;
  const range = document.createRange();
  for (const t of nodes) {
    WORD.lastIndex = 0;
    for (const m of t.data.matchAll(WORD)) {
      const letters = Array.from(m[0]);
      const n = boldLength(letters.length, fixation);
      const end = letters.slice(0, n).join("").length;
      range.setStart(t, m.index);
      range.setEnd(t, m.index + end);
      for (const r of range.getClientRects()) {
        const x = Math.max(0, (r.left - box.left) * sx);
        const y = Math.max(0, (r.top - box.top) * sy);
        const w = Math.min(srcW - x, r.width * sx);
        const h = Math.min(srcH - y, r.height * sy);
        if (w < 1 || h < 1) continue;
        // About 4.5% of the line height: a clear but light thickening.
        const d = Math.max(1, h * 0.045);
        ctx.drawImage(pic, x, y, w, h, x, y, w, h);
        for (const off of [d * 0.5, d]) ctx.drawImage(pic, x, y, w, h, x + off, y, w, h);
        drawn++;
      }
    }
  }
  if (!drawn) return false;
  // Page themes recolour the picture with a filter: the layer gets the same.
  layer.style.filter = getComputedStyle(pic).filter;
  // Over the picture; highlights stay on top, and the text layer still
  // takes the pointer (the layer lets it through).
  page.append(layer);
  return true;
}
