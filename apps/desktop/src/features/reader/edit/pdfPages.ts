/**
 * Pictures of PDF pages for the page editor: small ones for the grid and
 * large ones for cropping, redacting and correcting. Pages are drawn one
 * at a time so a long book does not stall the window.
 */
import type { PDFDocumentProxy, PDFPageProxy } from "pdfjs-dist";
import { loadPdfJs, PDF_ASSETS } from "@/readers";

export interface PageText {
  /** Words or runs of text with their boxes (fractions of the shown page). */
  items: { text: string; box: [number, number, number, number]; font?: string }[];
}

let measurer: CanvasRenderingContext2D | null = null;

/** Where characters `from`–`to` of a run lie, as fractions of its width. */
function span(t: string, from: number, to: number, font = "serif"): [number, number] {
  measurer ??= document.createElement("canvas").getContext("2d");
  if (!measurer) return [from / Math.max(1, t.length), to / Math.max(1, t.length)];
  measurer.font = `100px ${font}`;
  const whole = measurer.measureText(t).width || 1;
  return [
    measurer.measureText(t.slice(0, from)).width / whole,
    measurer.measureText(t.slice(0, to)).width / whole,
  ];
}

export class PdfPages {
  private doc: PDFDocumentProxy | null = null;
  private cache = new Map<string, string>();
  private queue: Promise<unknown> = Promise.resolve();
  private closed = false;

  static async open(url: string): Promise<PdfPages> {
    const { pdfjs } = await loadPdfJs();
    const p = new PdfPages();
    p.doc = await pdfjs.getDocument({ url, ...PDF_ASSETS }).promise;
    return p;
  }

  get count(): number {
    return this.doc?.numPages ?? 0;
  }

  close() {
    this.closed = true;
    for (const url of this.cache.values()) URL.revokeObjectURL(url);
    this.cache.clear();
    void this.doc?.loadingTask.destroy();
  }

  private page(n: number): Promise<PDFPageProxy> {
    return this.doc!.getPage(n);
  }

  /** The page's own turning (degrees). */
  async ownRotation(n: number): Promise<number> {
    return (await this.page(n)).rotate;
  }

  /** Height over width of the page as shown, turned `extra` more degrees. */
  async aspect(n: number, extra = 0): Promise<number> {
    const page = await this.page(n);
    const vp = page.getViewport({ scale: 1, rotation: (page.rotate + extra) % 360 });
    return vp.height / vp.width;
  }

  /** Size in points as shown (turned `extra` more degrees). */
  async size(n: number, extra = 0): Promise<[number, number]> {
    const page = await this.page(n);
    const vp = page.getViewport({ scale: 1, rotation: (page.rotate + extra) % 360 });
    return [vp.width, vp.height];
  }

  /** A picture of the page `width` CSS pixels wide, as an object URL. */
  picture(n: number, width: number, extra = 0): Promise<string> {
    const key = `${n}:${width}:${extra}`;
    const hit = this.cache.get(key);
    if (hit) return Promise.resolve(hit);
    const job = this.queue.then(async () => {
      if (this.closed) throw new Error("closed");
      const again = this.cache.get(key);
      if (again) return again;
      const page = await this.page(n);
      const rotation = (page.rotate + extra) % 360;
      const base = page.getViewport({ scale: 1, rotation });
      const ratio = Math.min(2, window.devicePixelRatio || 1);
      const vp = page.getViewport({ scale: (width * ratio) / base.width, rotation });
      const canvas = document.createElement("canvas");
      canvas.width = Math.ceil(vp.width);
      canvas.height = Math.ceil(vp.height);
      const ctx = canvas.getContext("2d")!;
      ctx.fillStyle = "#fff";
      ctx.fillRect(0, 0, canvas.width, canvas.height);
      await page.render({ canvas, canvasContext: ctx, viewport: vp }).promise;
      const blob = await new Promise<Blob | null>((ok) => canvas.toBlob(ok, "image/png"));
      const url = blob ? URL.createObjectURL(blob) : canvas.toDataURL("image/png");
      if (this.closed) {
        URL.revokeObjectURL(url);
        throw new Error("closed");
      }
      this.cache.set(key, url);
      return url;
    });
    this.queue = job.catch(() => undefined);
    return job;
  }

  /** The text of a page as shown (the page's own turning only). */
  async text(n: number): Promise<PageText> {
    const page = await this.page(n);
    const vp = page.getViewport({ scale: 1, rotation: page.rotate });
    const content = await page.getTextContent();
    const styles = content.styles as Record<string, { fontFamily?: string }>;
    const items: PageText["items"] = [];
    for (const it of content.items) {
      if (!("str" in it) || !it.str.trim()) continue;
      const [a, b, , , e, f] = it.transform as number[];
      const height = Math.hypot(a!, b!) || it.height;
      // The run's box in PDF space, then on the shown page.
      const [x1, y1] = vp.convertToViewportPoint(e!, f! - height * 0.22) as number[];
      const [x2, y2] = vp.convertToViewportPoint(e! + it.width, f! + height * 0.9) as number[];
      const x = Math.min(x1!, x2!) / vp.width;
      const y = Math.min(y1!, y2!) / vp.height;
      const w = Math.abs(x2! - x1!) / vp.width;
      const h = Math.abs(y2! - y1!) / vp.height;
      items.push({ text: it.str, box: [x, y, w, h], font: styles[it.fontName]?.fontFamily });
    }
    return { items };
  }
}

/** Boxes around every place `query` is found in the text runs. */
export function findBoxes(text: PageText, query: string): [number, number, number, number][] {
  const q = query.trim().toLowerCase();
  if (!q) return [];
  const out: [number, number, number, number][] = [];
  for (const { text: t, box, font } of text.items) {
    const lower = t.toLowerCase();
    let at = lower.indexOf(q);
    while (at >= 0) {
      // Measured with a similar font, and padded a little.
      const [a, b] = span(t, at, at + q.length, font);
      const pad = (box[2] / Math.max(1, t.length)) * 0.2;
      out.push([box[0] + box[2] * a - pad, box[1], box[2] * (b - a) + pad * 2, box[3]]);
      at = lower.indexOf(q, at + q.length);
    }
  }
  return out;
}

/** The text under a box, for a correction's starting text. */
export function textUnder(text: PageText, [x, y, w, h]: [number, number, number, number]): string {
  const hits = text.items.filter(({ box }) => {
    const cy = box[1] + box[3] / 2;
    return cy >= y && cy <= y + h && box[0] + box[2] > x && box[0] < x + w;
  });
  return hits
    .map(({ text: t, box }) => {
      // Only the characters inside the box.
      const per = box[2] / Math.max(1, t.length);
      const from = Math.max(0, Math.round((x - box[0]) / per));
      const to = Math.min(t.length, Math.round((x + w - box[0]) / per));
      return t.slice(from, to);
    })
    .join(" ")
    .replace(/\s+/g, " ")
    .trim();
}
