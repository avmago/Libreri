import { describe, expect, it } from "vitest";
import { layoutToLatex, type Glyph } from "./layout";

/** Lays characters out like a PDF: main size 20 with the line's centre at
 * y = 100. `dy` moves the centre (negative is up); `size` is the height. */
function lay(items: [text: string, x: number, dy?: number, size?: number][]): Glyph[] {
  const out: Glyph[] = [];
  for (const [text, x0, dy = 0, size = 20] of items) {
    let x = x0;
    for (const ch of text) {
      const w = size * 0.55;
      out.push({ ch, x, y: 100 + dy - size / 2, w, h: size });
      x += w;
    }
  }
  return out;
}

describe("layoutToLatex", () => {
  it("reads superscripts and subscripts of several characters", () => {
    // x_{2n}^{k+1}
    expect(
      layoutToLatex(
        lay([
          ["x", 0],
          ["2n", 11.5, 6, 14],
          ["k+1", 11.5, -8, 14],
        ]),
      ),
    ).toBe("x_{2n}^{k+1}");
    // e^{-x^2}
    expect(
      layoutToLatex(
        lay([
          ["e", 0],
          ["−x", 11.5, -8, 14],
          ["2", 27, -14, 10],
        ]),
      ),
    ).toBe("e^{-x^2}");
  });

  it("reads a sum with limits above and below", () => {
    // ∑_{n=1}^{∞} a_n, limits centred under and over the ∑.
    const g = lay([
      ["∑", 10, 0, 28],
      ["n=1", 7.5, 22, 14],
      ["∞", 20, -22, 14],
      ["a", 34],
      ["n", 45, 6, 14],
    ]);
    expect(layoutToLatex(g)).toBe("\\sum_{n=1}^{\\infty}a_n");
  });

  it("reads a sum with limits beside it (text style)", () => {
    const g = lay([
      ["∑", 0],
      ["i=0", 11, 6, 14],
      ["N", 11, -8, 14],
      ["i", 40],
    ]);
    expect(layoutToLatex(g)).toBe("\\sum_{i=0}^Ni");
  });

  it("reads a fraction after an equals sign", () => {
    const g = lay([
      ["y", 0],
      ["=", 15],
      ["a+b", 30, -12],
      ["c", 38, 12],
    ]);
    expect(layoutToLatex(g)).toBe("y=\\frac{a+b}{c}");
  });

  it("reads a fraction on its own", () => {
    expect(
      layoutToLatex(
        lay([
          ["a+b+c", 0, -12],
          ["d", 12, 12],
        ]),
      ),
    ).toBe("\\frac{a+b+c}{d}");
  });

  it("keeps function names and limits under lim", () => {
    const g = lay([
      ["lim", 0],
      ["x→0", 2, 20, 14],
      ["sin", 40],
      ["x", 80],
    ]);
    expect(layoutToLatex(g)).toBe("\\lim_{x\\to 0}\\sin x");
  });

  it("leaves plain text as it is", () => {
    expect(
      layoutToLatex(
        lay([
          ["E=mc", 0],
          ["2", 44, -8, 14],
        ]),
      ),
    ).toBe("E=mc^2");
  });

  it("reads a root up to the next relation sign", () => {
    const g = lay([
      ["√", 0, -8],
      ["x", 12],
      ["2", 23, -8, 14],
      ["+y", 32],
      ["=", 60],
      ["r", 76],
    ]);
    expect(layoutToLatex(g)).toBe("\\sqrt{x^2+y}=r");
  });

  it("reads a binomial between tall brackets", () => {
    const g = [
      ...lay([
        ["n", 20, -12],
        ["k", 20, 12],
        ["=", 45],
      ]),
      { ch: "(", x: 10, y: 70, w: 8, h: 20, sized: true },
      { ch: ")", x: 32, y: 70, w: 8, h: 20, sized: true },
    ];
    expect(layoutToLatex(g)).toBe("\\binom{n}{k}=");
  });

  it("reads maths italic letters, Greek too", () => {
    expect(
      layoutToLatex(
        lay([
          ["𝑒", 0],
          ["𝑖𝜋", 11.5, -8, 14],
        ]),
      ),
    ).toBe("e^{i\\pi}");
  });
});
