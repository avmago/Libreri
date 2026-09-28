/**
 * Draws marks as SVG. Each page has one `<svg>` whose view box is 1000
 * units wide and as tall as the page's shape, so strokes keep their width
 * however the page is zoomed.
 */
import { getStroke } from "perfect-freehand";
import { bounds, measure, type Mark, type MarkupItem, type Pt } from "./model";

export const NS = "http://www.w3.org/2000/svg";
export const W = 1000;

const FONTS: Record<"sans" | "serif" | "hand", string> = {
  sans: '"Geist Variable", "Helvetica Neue", Arial, sans-serif',
  serif: 'Georgia, "Times New Roman", serif',
  hand: '"Segoe Print", "Bradley Hand", "Comic Sans MS", cursive',
};

export function fontFamily(font: "sans" | "serif" | "hand"): string {
  return FONTS[font];
}

function el<K extends keyof SVGElementTagNameMap>(
  name: K,
  attrs: Record<string, string | number>,
): SVGElementTagNameMap[K] {
  const e = document.createElementNS(NS, name);
  for (const [k, v] of Object.entries(attrs)) e.setAttribute(k, String(v));
  return e;
}

/** A filled outline of a pressure-sensitive stroke, as an SVG path. */
export function inkPath(
  points: [number, number, number][],
  width: number,
  highlighter: boolean,
  H: number,
): string {
  const pressured = points.some((p) => Math.abs(p[2] - 0.5) > 0.01);
  const outline = getStroke(
    points.map(([x, y, pr]) => [x * W, y * H, pr]),
    {
      size: width * W,
      thinning: highlighter ? 0 : 0.55,
      smoothing: 0.5,
      streamline: 0.45,
      simulatePressure: !pressured && !highlighter,
      last: true,
    },
  );
  if (!outline.length) return "";
  const d = outline.reduce<(string | number)[]>(
    (acc, [x0, y0], i, arr) => {
      const [x1, y1] = arr[(i + 1) % arr.length]!;
      acc.push(
        x0!.toFixed(2),
        y0!.toFixed(2),
        ((x0! + x1!) / 2).toFixed(2),
        ((y0! + y1!) / 2).toFixed(2),
      );
      return acc;
    },
    ["M", outline[0]![0]!.toFixed(2), outline[0]![1]!.toFixed(2), "Q"],
  );
  return `${d.join(" ")} Z`;
}

function dashArray(dash: "solid" | "dashed" | "dotted", w: number): string {
  if (dash === "dashed") return `${w * 4} ${w * 3}`;
  if (dash === "dotted") return `0.1 ${w * 2.2}`;
  return "none";
}

/** Head of an arrow at `to`, as a filled triangle. */
export function arrowHead(from: Pt, to: Pt, w: number, H: number): string {
  const x1 = from[0] * W;
  const y1 = from[1] * H;
  const x2 = to[0] * W;
  const y2 = to[1] * H;
  const angle = Math.atan2(y2 - y1, x2 - x1);
  const size = Math.max(10, w * 5);
  const a = angle + Math.PI - 0.45;
  const b = angle + Math.PI + 0.45;
  return `M ${x2} ${y2} L ${x2 + size * Math.cos(a)} ${y2 + size * Math.sin(a)} L ${x2 + size * Math.cos(b)} ${y2 + size * Math.sin(b)} Z`;
}

/** Where a measurement's label goes: the middle of its line or shape. */
function labelAt(points: Pt[], H: number): [number, number] {
  if (points.length === 3) return [points[1]![0] * W + 14, points[1]![1] * H - 14];
  const xs = points.map((p) => p[0] * W);
  const ys = points.map((p) => p[1] * H);
  return [
    xs.reduce((a, b) => a + b, 0) / xs.length,
    ys.reduce((a, b) => a + b, 0) / ys.length - 10,
  ];
}

/** SVG for one item (without selection). */
export function renderItem(item: MarkupItem, H: number, note: string | null = null): SVGGElement {
  const g = el("g", {});
  switch (item.tool) {
    case "pen":
    case "highlighter": {
      const path = el("path", {
        d: inkPath(item.points, item.width, item.tool === "highlighter", H),
        fill: item.color,
        "fill-opacity": item.opacity,
      });
      if (item.tool === "highlighter") path.style.mixBlendMode = "multiply";
      g.append(path);
      break;
    }
    case "rect":
    case "ellipse": {
      const [x, y, w, h] = item.box;
      const sw = item.width * W;
      const common = {
        stroke: item.color,
        "stroke-width": sw,
        "stroke-opacity": item.opacity,
        fill: item.fill ?? "none",
        "fill-opacity": item.fill ? item.opacity * 0.35 : 0,
        "stroke-dasharray": dashArray(item.dash, sw),
        "stroke-linecap": "round",
      };
      g.append(
        item.tool === "rect"
          ? el("rect", { x: x * W, y: y * H, width: w * W, height: h * H, ...common })
          : el("ellipse", {
              cx: (x + w / 2) * W,
              cy: (y + h / 2) * H,
              rx: (w / 2) * W,
              ry: (h / 2) * H,
              ...common,
            }),
      );
      break;
    }
    case "line":
    case "arrow": {
      const sw = item.width * W;
      g.append(
        el("line", {
          x1: item.from[0] * W,
          y1: item.from[1] * H,
          x2: item.to[0] * W,
          y2: item.to[1] * H,
          stroke: item.color,
          "stroke-width": sw,
          "stroke-opacity": item.opacity,
          "stroke-linecap": "round",
          "stroke-dasharray": dashArray(item.dash, sw),
        }),
      );
      if (item.tool === "arrow")
        g.append(
          el("path", {
            d: arrowHead(item.from, item.to, sw, H),
            fill: item.color,
            "fill-opacity": item.opacity,
          }),
        );
      break;
    }
    case "text": {
      const [x, y, w, h] = item.box;
      const fo = el("foreignObject", { x: x * W, y: y * H, width: w * W, height: h * H });
      const div = document.createElement("div");
      div.className = "lb-markup-text";
      div.style.cssText = `color:${item.color};font-size:${item.size * W}px;font-family:${fontFamily(item.font)};${item.background ? `background:${item.background};` : ""}`;
      div.textContent = item.text;
      fo.append(div);
      g.append(fo);
      break;
    }
    case "note": {
      const s = 26;
      const x = item.at[0] * W;
      const y = item.at[1] * H;
      g.append(
        el("path", {
          d: `M ${x} ${y} h ${s} v ${s * 0.7} l ${-s * 0.3} ${s * 0.3} h ${-s * 0.7} Z`,
          fill: item.color,
          stroke: "rgba(0,0,0,0.35)",
          "stroke-width": 1,
        }),
        el("path", {
          d: `M ${x + s} ${y + s * 0.7} h ${-s * 0.3} v ${s * 0.3}`,
          fill: "rgba(0,0,0,0.15)",
        }),
      );
      for (let i = 0; i < 3; i++)
        g.append(
          el("line", {
            x1: x + 5,
            x2: x + s - 5,
            y1: y + 6 + i * 5,
            y2: y + 6 + i * 5,
            stroke: "rgba(0,0,0,0.4)",
            "stroke-width": 1.2,
          }),
        );
      const title = el("title", {});
      title.textContent = note ?? "Sticky note";
      g.append(title);
      break;
    }
    case "stamp": {
      const [x, y, w, h] = item.box;
      g.append(
        el("rect", {
          x: x * W,
          y: y * H,
          width: w * W,
          height: h * H,
          rx: 6,
          fill: "none",
          stroke: item.color,
          "stroke-width": 3,
        }),
      );
      const t = el("text", {
        x: (x + w / 2) * W,
        y: (y + h / 2) * H,
        "text-anchor": "middle",
        "dominant-baseline": "central",
        fill: item.color,
        "font-weight": 700,
        "font-family": FONTS.sans,
        "font-size": Math.min(h * H * 0.55, (w * W) / Math.max(4, item.text.length * 0.62)),
        "letter-spacing": 1,
      });
      t.textContent = item.text;
      g.append(t);
      break;
    }
    case "image": {
      const [x, y, w, h] = item.box;
      g.append(
        el("image", {
          x: x * W,
          y: y * H,
          width: w * W,
          height: h * H,
          href: item.src,
          preserveAspectRatio: "none",
        }),
      );
      break;
    }
    case "measure": {
      const sw = item.width * W;
      const pts = item.points.map(([x, y]) => `${x * W},${y * H}`).join(" ");
      const shape =
        item.kind === "area"
          ? el("polygon", { points: pts, fill: item.color, "fill-opacity": 0.12 })
          : el("polyline", { points: pts, fill: "none" });
      shape.setAttribute("stroke", item.color);
      shape.setAttribute("stroke-width", String(sw));
      shape.setAttribute("stroke-linejoin", "round");
      g.append(shape);
      for (const [x, y] of item.points)
        g.append(
          el("circle", { cx: x * W, cy: y * H, r: Math.max(3, sw * 1.2), fill: item.color }),
        );
      if (item.points.length >= 2) {
        const [lx, ly] = labelAt(item.points, H);
        const label = measure(item).label;
        const size = 13;
        const bw = label.length * size * 0.6 + 10;
        g.append(
          el("rect", {
            x: lx - bw / 2,
            y: ly - size,
            width: bw,
            height: size * 1.6,
            rx: 4,
            fill: "white",
            "fill-opacity": 0.92,
            stroke: item.color,
            "stroke-width": 1,
          }),
        );
        const t = el("text", {
          x: lx,
          y: ly - size * 0.2,
          "text-anchor": "middle",
          "dominant-baseline": "central",
          fill: "#111",
          "font-size": size,
          "font-family": FONTS.sans,
        });
        t.textContent = label;
        g.append(t);
      }
      break;
    }
  }
  return g;
}

/** A dashed box around the selected mark, with a resize handle when it has a box. */
export function renderSelection(m: Mark, H: number): SVGGElement {
  const g = el("g", { class: "lb-markup-selection" });
  const [x, y, w, h] = bounds(m.item);
  const pad = 6;
  g.append(
    el("rect", {
      x: x * W - pad,
      y: y * H - pad,
      width: w * W + pad * 2,
      height: h * H + pad * 2,
      fill: "none",
      stroke: "#2563eb",
      "stroke-width": 1.5,
      "stroke-dasharray": "5 4",
    }),
  );
  if ("box" in m.item) {
    g.append(
      el("rect", {
        class: "lb-markup-handle",
        x: (x + w) * W - 5,
        y: (y + h) * H - 5,
        width: 10,
        height: 10,
        fill: "#2563eb",
        stroke: "white",
        "stroke-width": 1.5,
      }),
    );
  }
  return g;
}
