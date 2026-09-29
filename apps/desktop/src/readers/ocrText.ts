/**
 * Finding words in page text that came with boxes (DjVu text layers and
 * OCR text): shared by the page and PDF readers.
 */
import type { WordDto } from "@/lib/ipc";
import type { Rect } from "./types";

/** Lower case without accents, so "Café" finds "cafe". */
export function norm(s: string): string {
  return s.normalize("NFKD").replace(/\p{M}/gu, "").toLowerCase();
}

/** Every match of `query` in page texts: its page and its number on the page. */
export function findHits(texts: string[], query: string): { page: number; index: number }[] {
  const q = norm(query).replace(/\s+/g, " ").trim();
  const hits: { page: number; index: number }[] = [];
  if (!q) return hits;
  texts.forEach((t, i) => {
    const text = norm(t).replace(/\s+/g, " ");
    let at = text.indexOf(q);
    let k = 0;
    while (at >= 0) {
      hits.push({ page: i + 1, index: k++ });
      at = text.indexOf(q, at + 1);
    }
  });
  return hits;
}

/** Where the `index`th match of `query` is among `words`, as rectangles. */
export function matchRects(words: WordDto[], query: string, index: number): Rect[] {
  if (!words.length) return [];
  const q = norm(query).replace(/\s+/g, " ").trim();
  let text = "";
  const starts: number[] = [];
  for (const w of words) {
    starts.push(text.length);
    text += `${norm(w.text)} `;
  }
  let at = -1;
  for (let i = 0; i <= index; i++) {
    at = text.indexOf(q, at + 1);
    if (at < 0) return [];
  }
  const end = at + q.length;
  return words
    .filter((_, i) => starts[i]! < end && starts[i]! + norm(words[i]!.text).length > at)
    .map((w) => w.rect.map((v) => v ?? 0) as Rect);
}

/** Invisible words over a page image, stretched to their boxes, for selecting. */
export function wordLayer(page: HTMLElement, words: WordDto[], className: string): HTMLElement {
  const layer = document.createElement("div");
  layer.className = className;
  const h = page.clientHeight || 1000;
  for (const w of words) {
    const span = document.createElement("span");
    const [x, y, ww, wh] = w.rect.map((v) => v ?? 0) as Rect;
    span.textContent = `${w.text} `;
    span.style.cssText = `left:${x * 100}%;top:${y * 100}%;height:${wh * 100}%;font-size:${wh * h * 0.85}px;--w:${ww};--h:${wh}`;
    layer.append(span);
  }
  page.append(layer);
  requestAnimationFrame(() => fitWordLayer(page, layer));
  return layer;
}

/** Sizes and stretches the words of a layer to the page as it is now
 * drawn (again after zooming or resizing). */
export function fitWordLayer(page: HTMLElement, layer: HTMLElement) {
  const pw = page.clientWidth;
  const ph = page.clientHeight;
  // A hidden page has no size: fitted again when shown.
  if (!pw || !ph) return;
  const spans = Array.from(layer.children) as HTMLElement[];
  // Set every size first, then measure, so the page is laid out once.
  for (const span of spans) {
    const wh = parseFloat(span.style.getPropertyValue("--h"));
    if (wh > 0) span.style.fontSize = `${wh * ph * 0.85}px`;
    span.style.transform = "";
  }
  const natural = spans.map((span) => span.offsetWidth);
  spans.forEach((span, i) => {
    const target = parseFloat(span.style.getPropertyValue("--w")) * pw;
    if (natural[i]! > 0) span.style.transform = `scaleX(${target / natural[i]!})`;
  });
}
