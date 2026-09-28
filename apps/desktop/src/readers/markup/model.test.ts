import { describe, expect, it } from "vitest";
import {
  calibrate,
  convertScale,
  defaultScale,
  hits,
  measure,
  moved,
  parseLocator,
  snapStroke,
  type MarkupItem,
} from "./model";

const circle = (cx: number, cy: number, r: number, n = 40): [number, number, number][] =>
  Array.from({ length: n + 1 }, (_, i) => {
    const a = (i / n) * Math.PI * 2;
    return [cx + r * Math.cos(a), cy + r * Math.sin(a), 0.5];
  });

describe("snapping rough strokes", () => {
  it("turns a nearly straight stroke into a line", () => {
    const pts: [number, number, number][] = Array.from({ length: 20 }, (_, i) => [
      0.1 + i * 0.02,
      0.5 + (i % 2) * 0.002,
      0.5,
    ]);
    expect(snapStroke(pts, 1)?.tool).toBe("line");
  });

  it("turns a closed round stroke into an ellipse and a boxy one into a rectangle", () => {
    expect(snapStroke(circle(0.5, 0.5, 0.1), 1)?.tool).toBe("ellipse");
    const box: [number, number, number][] = [];
    const side = (x0: number, y0: number, x1: number, y1: number) => {
      for (let i = 0; i < 10; i++)
        box.push([x0 + ((x1 - x0) * i) / 10, y0 + ((y1 - y0) * i) / 10, 0.5]);
    };
    side(0.2, 0.2, 0.6, 0.2);
    side(0.6, 0.2, 0.6, 0.5);
    side(0.6, 0.5, 0.2, 0.5);
    side(0.2, 0.5, 0.2, 0.2);
    box.push([0.2, 0.2, 0.5]);
    const r = snapStroke(box, 1);
    expect(r?.tool).toBe("rect");
    if (r && "box" in r) expect(r.box).toEqual([0.2, 0.2, 0.4, 0.3]);
  });

  it("leaves handwriting alone", () => {
    const squiggle: [number, number, number][] = Array.from({ length: 30 }, (_, i) => [
      0.1 + i * 0.01,
      0.5 + Math.sin(i) * 0.03,
      0.5,
    ]);
    expect(snapStroke(squiggle, 1)).toBeNull();
  });
});

describe("measuring", () => {
  const page: [number, number] = [612, 792];
  it("measures true size on PDFs by default", () => {
    const m: MarkupItem = {
      tool: "measure",
      kind: "distance",
      points: [
        [0, 0],
        [72 / 612, 0],
      ],
      page,
      scale: defaultScale(true, "in"),
      color: "#000",
      width: 0.001,
      opacity: 1,
    };
    expect(measure(m as Extract<MarkupItem, { tool: "measure" }>).label).toBe("1.00 in");
  });

  it("calibrates and converts units", () => {
    const s = calibrate([0, 0], [0.5, 0], page, 3, "m");
    expect(s.perUnit * 306).toBeCloseTo(3);
    const mm = convertScale(defaultScale(true, "in"), "mm");
    expect(mm.perUnit * 72).toBeCloseTo(25.4);
  });

  it("measures areas and angles", () => {
    const base = {
      tool: "measure" as const,
      page: [100, 100] as [number, number],
      scale: { perUnit: 1, unit: "px" as const },
      color: "#000",
      width: 0.001,
      opacity: 1,
    };
    const sq = measure({
      ...base,
      kind: "area",
      points: [
        [0, 0],
        [0.1, 0],
        [0.1, 0.1],
        [0, 0.1],
      ],
    });
    expect(sq.value).toBeCloseTo(100);
    const right = measure({
      ...base,
      kind: "angle",
      points: [
        [0.5, 0],
        [0, 0],
        [0, 0.5],
      ],
    });
    expect(right.label).toBe("90.0°");
  });
});

describe("marks", () => {
  it("hit-tests and moves", () => {
    const line: MarkupItem = {
      tool: "line",
      from: [0.1, 0.1],
      to: [0.5, 0.1],
      color: "#000",
      width: 0.002,
      opacity: 1,
      dash: "solid",
    };
    expect(hits(line, [0.3, 0.101], 1.3)).toBe(true);
    expect(hits(line, [0.3, 0.2], 1.3)).toBe(false);
    const m = moved(line, 0.1, 0.1);
    expect(m.tool === "line" && m.from).toEqual([0.2, 0.2]);
  });

  it("reads stored locators", () => {
    expect(
      parseLocator(
        '{"type":"markup","page":2,"layer":"Markup","item":{"tool":"note","at":[0,0],"color":"#ff0"}}',
      )?.page,
    ).toBe(2);
    expect(parseLocator('{"type":"pdf","page":2}')).toBeNull();
    expect(parseLocator("{")).toBeNull();
  });
});
