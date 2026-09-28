/**
 * "Export marked-up copy": turns marks into the drawing list the Rust side
 * draws onto PDF pages. Lines, shapes and ink stay vector; text boxes,
 * stamps, sticky notes and measurement labels are drawn here as pictures,
 * so every font and script looks exactly as on screen.
 */
import type { DrawOp, DrawPage } from "@/lib/ipc";
import { measure, type Mark, type MarkupItem, type Pt } from "./model";
import { W, arrowHead, fontFamily, inkPath } from "./render";

/** Rewrites SVG path data from view-box units (1000 × H) to page fractions. */
function toFractions(d: string, H: number): string {
  let i = 0;
  return d.replace(/-?\d+(\.\d+)?(e-?\d+)?/g, (n) => {
    const v = Number(n) / (i++ % 2 === 0 ? W : H);
    return v.toFixed(5);
  });
}

function dash(style: "solid" | "dashed" | "dotted", w: number): number[] {
  if (style === "dashed") return [w * 4, w * 3];
  if (style === "dotted") return [w * 0.05, w * 2.2];
  return [];
}

function ellipsePath([x, y, w, h]: [number, number, number, number]): string {
  const k = 0.5522848;
  const cx = x + w / 2;
  const cy = y + h / 2;
  const rx = w / 2;
  const ry = h / 2;
  return [
    `M ${cx + rx} ${cy}`,
    `C ${cx + rx} ${cy + k * ry} ${cx + k * rx} ${cy + ry} ${cx} ${cy + ry}`,
    `C ${cx - k * rx} ${cy + ry} ${cx - rx} ${cy + k * ry} ${cx - rx} ${cy}`,
    `C ${cx - rx} ${cy - k * ry} ${cx - k * rx} ${cy - ry} ${cx} ${cy - ry}`,
    `C ${cx + k * rx} ${cy - ry} ${cx + rx} ${cy - k * ry} ${cx + rx} ${cy}`,
    "Z",
  ].join(" ");
}

const poly = (pts: Pt[], close = false) =>
  pts.map(([x, y], i) => `${i ? "L" : "M"} ${x} ${y}`).join(" ") + (close ? " Z" : "");

/** Draws text in a box onto a canvas and returns it as a PNG data URL. */
function textPicture(
  box: [number, number, number, number],
  aspect: number,
  draw: (ctx: CanvasRenderingContext2D, w: number, h: number, px: number) => void,
): string {
  const px = 3; // canvas pixels per view-box unit: sharp when printed
  const w = Math.max(1, Math.round(box[2] * W * px));
  const h = Math.max(1, Math.round(box[3] * W * aspect * px));
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d")!;
  draw(ctx, w, h, px);
  return canvas.toDataURL("image/png");
}

function wrap(ctx: CanvasRenderingContext2D, text: string, width: number): string[] {
  const lines: string[] = [];
  for (const para of text.split("\n")) {
    let line = "";
    for (const word of para.split(/(\s+)/)) {
      const next = line + word;
      if (line && ctx.measureText(next).width > width) {
        lines.push(line.trimEnd());
        line = word.trimStart();
      } else line = next;
    }
    lines.push(line);
  }
  return lines;
}

function ops(m: Mark, aspect: number): DrawOp[] {
  const i: MarkupItem = m.item;
  const H = W * aspect;
  switch (i.tool) {
    case "pen":
    case "highlighter":
      return [
        {
          type: "fill",
          d: toFractions(inkPath(i.points, i.width, i.tool === "highlighter", H), H),
          color: i.color,
          opacity: i.opacity,
          multiply: i.tool === "highlighter",
        },
      ];
    case "rect":
    case "ellipse": {
      const [x, y, w, h] = i.box;
      const d =
        i.tool === "rect"
          ? poly(
              [
                [x, y],
                [x + w, y],
                [x + w, y + h],
                [x, y + h],
              ],
              true,
            )
          : ellipsePath(i.box);
      const out: DrawOp[] = [];
      if (i.fill)
        out.push({ type: "fill", d, color: i.fill, opacity: i.opacity * 0.35, multiply: false });
      out.push({
        type: "stroke",
        d,
        color: i.color,
        opacity: i.opacity,
        width: i.width,
        dash: dash(i.dash, i.width),
      });
      return out;
    }
    case "line":
    case "arrow": {
      const out: DrawOp[] = [
        {
          type: "stroke",
          d: poly([i.from, i.to]),
          color: i.color,
          opacity: i.opacity,
          width: i.width,
          dash: dash(i.dash, i.width),
        },
      ];
      if (i.tool === "arrow")
        out.push({
          type: "fill",
          d: toFractions(arrowHead(i.from, i.to, i.width * W, H), H),
          color: i.color,
          opacity: i.opacity,
          multiply: false,
        });
      return out;
    }
    case "measure": {
      const out: DrawOp[] = [
        {
          type: "stroke",
          d: poly(i.points, i.kind === "area"),
          color: i.color,
          opacity: 1,
          width: i.width,
          dash: [],
        },
      ];
      const r = Math.max(0.003, i.width * 1.2);
      for (const [x, y] of i.points)
        out.push({
          type: "fill",
          d: ellipsePath([x - r, y - r / aspect, 2 * r, (2 * r) / aspect]),
          color: i.color,
          opacity: 1,
          multiply: false,
        });
      if (i.points.length >= 2) {
        const label = measure(i).label;
        const cx = i.points.reduce((a, p) => a + p[0], 0) / i.points.length;
        const cy = i.points.reduce((a, p) => a + p[1], 0) / i.points.length;
        const bw = (label.length * 13 * 0.6 + 10) / W;
        const bh = (13 * 1.6) / (W * aspect);
        const box: [number, number, number, number] = [cx - bw / 2, cy - bh, bw, bh];
        out.push({
          type: "image",
          rect: box,
          src: textPicture(box, aspect, (ctx, w, h, px) => {
            ctx.fillStyle = "rgba(255,255,255,0.92)";
            ctx.fillRect(0, 0, w, h);
            ctx.strokeStyle = i.color;
            ctx.lineWidth = px;
            ctx.strokeRect(0, 0, w, h);
            ctx.fillStyle = "#111";
            ctx.font = `${13 * px}px ${fontFamily("sans")}`;
            ctx.textAlign = "center";
            ctx.textBaseline = "middle";
            ctx.fillText(label, w / 2, h / 2);
          }),
        });
      }
      return out;
    }
    case "text":
      return [
        {
          type: "image",
          rect: i.box,
          src: textPicture(i.box, aspect, (ctx, w, _h, px) => {
            if (i.background) {
              ctx.fillStyle = i.background;
              ctx.fillRect(0, 0, w, _h);
            }
            const size = i.size * W * px;
            ctx.font = `${size}px ${fontFamily(i.font)}`;
            ctx.fillStyle = i.color;
            ctx.textBaseline = "top";
            const lines = wrap(ctx, i.text, w - 6 * px);
            lines.forEach((l, n) => ctx.fillText(l, 3 * px, 2 * px + n * size * 1.25));
          }),
        },
      ];
    case "stamp":
      return [
        {
          type: "image",
          rect: i.box,
          src: textPicture(i.box, aspect, (ctx, w, h, px) => {
            ctx.strokeStyle = i.color;
            ctx.lineWidth = 3 * px;
            ctx.beginPath();
            ctx.roundRect(1.5 * px, 1.5 * px, w - 3 * px, h - 3 * px, 6 * px);
            ctx.stroke();
            const size = Math.min(h * 0.55, w / Math.max(4, i.text.length * 0.62));
            ctx.font = `700 ${size}px ${fontFamily("sans")}`;
            ctx.fillStyle = i.color;
            ctx.textAlign = "center";
            ctx.textBaseline = "middle";
            ctx.fillText(i.text, w / 2, h / 2);
          }),
        },
      ];
    case "note": {
      const s = 26 / W;
      const box: [number, number, number, number] = [i.at[0], i.at[1], s, s / aspect];
      return [
        {
          type: "image",
          rect: box,
          src: textPicture(box, aspect, (ctx, w, h) => {
            ctx.fillStyle = i.color;
            ctx.beginPath();
            ctx.moveTo(0, 0);
            ctx.lineTo(w, 0);
            ctx.lineTo(w, h * 0.7);
            ctx.lineTo(w * 0.7, h);
            ctx.lineTo(0, h);
            ctx.closePath();
            ctx.fill();
            ctx.strokeStyle = "rgba(0,0,0,0.4)";
            for (let n = 0; n < 3; n++) {
              ctx.beginPath();
              ctx.moveTo(w * 0.2, h * (0.25 + n * 0.2));
              ctx.lineTo(w * 0.8, h * (0.25 + n * 0.2));
              ctx.stroke();
            }
          }),
        },
      ];
    }
    case "image":
      return [{ type: "image", rect: i.box, src: i.src }];
  }
}

/**
 * The drawing list for every page with marks. `aspect(page)` is the page's
 * height over its width as shown.
 */
export function drawList(marks: Mark[], aspect: (page: number) => number): DrawPage[] {
  const byPage = new Map<number, DrawOp[]>();
  for (const m of marks) {
    const list = byPage.get(m.page) ?? [];
    list.push(...ops(m, aspect(m.page)));
    byPage.set(m.page, list);
  }
  return [...byPage.entries()]
    .sort((a, b) => a[0] - b[0])
    .map(([page, list]) => ({ page, ops: list }));
}
