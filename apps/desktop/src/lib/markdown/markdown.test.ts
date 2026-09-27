import { describe, expect, it } from "vitest";
import { renderMarkdown, stripFrontMatter } from ".";

describe("renderMarkdown", () => {
  it("renders inline and display maths with KaTeX", () => {
    const html = renderMarkdown("Euler: $e^{i\\pi}+1=0$\n\n$$\n\\int_0^1 x\\,dx\n$$\n");
    expect(html).toContain('class="katex"');
    expect(html).toContain("math-block");
    expect(html).toContain("katex-display");
  });

  it("leaves money alone", () => {
    const html = renderMarkdown("It cost $5 and then $6 more.");
    expect(html).not.toContain("katex");
  });

  it("never renders raw HTML", () => {
    const html = renderMarkdown('<img src=x onerror="alert(1)"> <script>alert(1)</script>');
    expect(html).not.toContain("<img");
    expect(html).not.toContain("<script");
  });

  it("gives headings stable ids and hides front matter", () => {
    const html = renderMarkdown("---\ntitle: T\n---\n# Linear maps\n## Linear maps\n");
    expect(html).toContain('id="linear-maps"');
    expect(html).toContain('id="linear-maps-1"');
    expect(html).not.toContain("title: T");
    expect(stripFrontMatter("no front matter")).toBe("no front matter");
  });

  it("marks Mermaid blocks for later drawing", () => {
    expect(renderMarkdown("```mermaid\ngraph TD; A-->B\n```")).toContain('<pre class="mermaid">');
  });
});
