import { describe, expect, it } from "vitest";
import { mathmlToLatex, textToLatex } from "./latex";

function math(xml: string): Element {
  return new DOMParser().parseFromString(
    `<math xmlns="http://www.w3.org/1998/Math/MathML">${xml}</math>`,
    "application/xml",
  ).documentElement;
}

describe("maths as LaTeX", () => {
  it("uses the TeX a book keeps", () => {
    const m = math(
      `<semantics><mrow><mi>x</mi></mrow><annotation encoding="application/x-tex">\\frac{a}{b}</annotation></semantics>`,
    );
    expect(mathmlToLatex(m)).toBe("\\frac{a}{b}");
  });

  it("turns MathML into LaTeX", () => {
    expect(
      mathmlToLatex(
        math(
          "<mrow><mfrac><mrow><mo>-</mo><mi>b</mi><mo>±</mo><msqrt><msup><mi>b</mi><mn>2</mn></msup><mo>-</mo><mn>4</mn><mi>a</mi><mi>c</mi></msqrt></mrow><mrow><mn>2</mn><mi>a</mi></mrow></mfrac></mrow>",
        ),
      ),
    ).toBe("\\frac{-b\\pm \\sqrt{b^2-4ac}}{2a}");
    expect(
      mathmlToLatex(
        math(
          "<munderover><mo>∑</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mi>n</mi></munderover><msub><mi>x</mi><mi>i</mi></msub>",
        ),
      ),
    ).toBe("\\sum_{i=1}^n x_i");
    expect(mathmlToLatex(math("<mover><mi>v</mi><mo>→</mo></mover>"))).toBe("\\vec{v}");
    expect(mathmlToLatex(math("<mi>sin</mi><mi>θ</mi>"))).toBe("\\sin \\theta");
  });

  it("rebuilds symbols from a PDF's text", () => {
    expect(textToLatex("E = mc²")).toBe("E = mc^2");
    expect(textToLatex("α ≤ β₁₂ → ∞")).toBe("\\alpha \\leq \\beta_{12} \\to \\infty");
  });

  it("writes function names as commands", () => {
    expect(textToLatex("sin x + cos y")).toBe("\\sin x + \\cos y");
    expect(textToLatex("sinx")).toBe("\\sin x");
    expect(textToLatex("log(x) ln 2")).toBe("\\log (x) \\ln 2");
    expect(textToLatex("\\sin x")).toBe("\\sin x");
    expect(textToLatex("sinxy maxima min")).toBe("sinxy maxima \\min");
  });
});
