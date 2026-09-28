/**
 * Read aloud over fixed pages (PDF, DjVu, scans with OCR): the words of
 * each page with their boxes, split into sentences, each marked by the
 * boxes of its lines.
 */
import { sentences } from "./sentences";
import type { SpeechPiece, SpeechSource } from "./types";

export type Box = [number, number, number, number];

export interface PageWord {
  text: string;
  rect: Box;
}

export interface WordPiece extends SpeechPiece {
  page: number;
  rects: Box[];
}

/** Line boxes around words (neighbours on a line are joined). */
export function lineBoxes(words: PageWord[]): Box[] {
  const out: Box[] = [];
  for (const { rect } of words) {
    const [x, y, w, h] = rect;
    const last = out[out.length - 1];
    if (last && Math.abs(last[1] - y) < Math.max(h, last[3]) * 0.5 && x >= last[0] - h) {
      const x1 = Math.max(last[0] + last[2], x + w);
      const y1 = Math.max(last[1] + last[3], y + h);
      last[0] = Math.min(last[0], x);
      last[1] = Math.min(last[1], y);
      last[2] = x1 - last[0];
      last[3] = y1 - last[1];
    } else out.push([x, y, w, h]);
  }
  return out;
}

/** Sentences of one page's words. */
export function pagePieces(page: number, words: PageWord[], lang?: string): WordPiece[] {
  let text = "";
  const spans: { from: number; to: number }[] = [];
  words.forEach((w, i) => {
    const prev = words[i - 1];
    if (prev) {
      // A new paragraph or heading: a different size, or a gap between lines.
      const [, py, , ph] = prev.rect;
      const [, y, , h] = w.rect;
      const newLine = Math.abs(y - py) > Math.max(h, ph) * 0.5;
      const newBlock =
        newLine &&
        (Math.max(h, ph) / Math.max(1e-6, Math.min(h, ph)) > 1.25 ||
          y - (py + ph) > Math.max(h, ph) * 0.8);
      text += newBlock ? "\n" : " ";
    }
    spans.push({ from: text.length, to: text.length + w.text.length });
    text += w.text;
  });
  // Words split at the end of a line ("light-" "house") are read whole.
  text = text.replace(/(\p{L})- (\p{Ll})/gu, "$1­$2");
  const out: WordPiece[] = [];
  for (const s of sentences(text, lang)) {
    const inside = words.filter((_, i) => spans[i]!.to > s.start && spans[i]!.from < s.end);
    const said = text.slice(s.start, s.end).replace(/­/g, "").trim();
    if (said) out.push({ text: said, lang, page, rects: lineBoxes(inside) });
  }
  return out;
}

/** Sentences from page `start` (and its part below `top`) to the end. */
export class WordSpeech implements SpeechSource {
  private page: number;
  private queue: WordPiece[] = [];
  private first = true;

  constructor(
    start: number,
    private readonly top: number,
    private readonly pages: number,
    private readonly wordsOf: (page: number) => Promise<PageWord[]>,
    private readonly draw: (page: number | null, rects: Box[]) => void,
    private readonly reveal: (page: number, top: number) => void,
    private readonly lang?: string,
  ) {
    this.page = start;
  }

  async next(): Promise<SpeechPiece | null> {
    while (!this.queue.length) {
      if (this.page > this.pages) return null;
      const words = await this.wordsOf(this.page).catch(() => [] as PageWord[]);
      let pieces = pagePieces(this.page, words, this.lang);
      if (this.first) {
        // Start with the first sentence at or below the top of the view.
        const at = pieces.findIndex((p) => (p.rects[0]?.[1] ?? 0) >= this.top - 0.01);
        pieces = at >= 0 ? pieces.slice(at) : [];
        this.first = false;
      }
      this.queue = pieces;
      this.page++;
    }
    return this.queue.shift()!;
  }

  show(piece: SpeechPiece, follow: boolean) {
    const p = piece as WordPiece;
    this.draw(p.page, p.rects);
    if (follow) this.reveal(p.page, p.rects[0]?.[1] ?? 0);
  }

  clear() {
    this.draw(null, []);
  }
}

/**
 * Words of a PDF.js text run, with boxes shared out by character count
 * (close enough to mark a sentence).
 */
export function runWords(text: string, box: Box): PageWord[] {
  const out: PageWord[] = [];
  const per = box[2] / Math.max(1, text.length);
  const re = /\S+/g;
  for (let m = re.exec(text); m; m = re.exec(text))
    out.push({ text: m[0], rect: [box[0] + per * m.index, box[1], per * m[0].length, box[3]] });
  return out;
}
