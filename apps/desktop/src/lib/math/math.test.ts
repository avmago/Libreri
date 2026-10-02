import { describe, expect, it } from "vitest";
import { hasMath, mathAsPlain, mathForAnki, renderMathText, splitMath } from ".";

describe("maths in plain text", () => {
  it("finds inline and display formulas", () => {
    expect(splitMath("Energy $E=mc^2$ and $$\\int_0^1 x\\,dx$$ done")).toEqual([
      { kind: "text", value: "Energy " },
      { kind: "inline", value: "E=mc^2" },
      { kind: "text", value: " and " },
      { kind: "display", value: "\\int_0^1 x\\,dx" },
      { kind: "text", value: " done" },
    ]);
    expect(splitMath("\\(a\\) and \\[b\\]").map((p) => p.kind)).toEqual([
      "inline",
      "text",
      "display",
    ]);
  });

  it("leaves money and lone dollars alone", () => {
    expect(hasMath("It cost $5 and $6 later")).toBe(false);
    expect(hasMath("$ not maths $")).toBe(false);
    expect(hasMath("just text")).toBe(false);
  });

  it("escapes the text around formulas", () => {
    const html = renderMathText("<b>x</b> $x$\nnext");
    expect(html).toContain("&lt;b&gt;");
    expect(html).toContain("katex");
    expect(html).toContain("<br>");
  });

  it("writes MathJax delimiters for Anki and plain source for speech", () => {
    expect(mathForAnki("Area $\\pi r^2$ <ok>")).toBe("Area \\(\\pi r^2\\) &lt;ok&gt;");
    expect(mathForAnki("$$a<b$$")).toBe("\\[a&lt;b\\]");
    expect(mathAsPlain("Area $\\pi r^2$")).toBe("Area \\pi r^2");
  });
});
