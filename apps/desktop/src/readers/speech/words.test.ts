import { describe, expect, it } from "vitest";
import { pagePieces } from "./words";
import { sentences } from "./sentences";
import { mathmlToSpeech, texToSpeech } from "./math";

describe("read aloud text", () => {
  it("splits sentences and keeps abbreviations", () => {
    const t = "Dr. Harte kept the light. It never failed! Did it? Yes.";
    const s = sentences(t).map((x) => t.slice(x.start, x.end));
    expect(s).toEqual(["Dr. Harte kept the light.", "It never failed!", "Did it?", "Yes."]);
  });

  it("starts a new sentence at a heading on a page", () => {
    const words = [
      { text: "Storms", rect: [0.1, 0.1, 0.1, 0.03] as [number, number, number, number] },
      { text: "The", rect: [0.1, 0.16, 0.05, 0.015] as [number, number, number, number] },
      { text: "keeper", rect: [0.16, 0.16, 0.08, 0.015] as [number, number, number, number] },
      { text: "wrote.", rect: [0.25, 0.16, 0.08, 0.015] as [number, number, number, number] },
    ];
    const p = pagePieces(3, words);
    expect(p.map((x) => x.text)).toEqual(["Storms", "The keeper wrote."]);
    expect(p[1]!.rects).toHaveLength(1);
    expect(p[1]!.page).toBe(3);
  });

  it("reads MathML", () => {
    const div = document.createElement("div");
    div.innerHTML =
      "<math><mfrac><mi>a</mi><mi>b</mi></mfrac><mo>+</mo><msup><mi>x</mi><mn>2</mn></msup><mo>=</mo><msqrt><mi>y</mi></msqrt></math>";
    expect(mathmlToSpeech(div.firstElementChild!)).toBe(
      "a over b plus x squared equals the square root of y",
    );
  });

  it("reads maths", () => {
    expect(texToSpeech("x^2 + \\frac{a}{b} = \\alpha")).toBe(
      "x squared plus a over b equals alpha",
    );
  });
});
