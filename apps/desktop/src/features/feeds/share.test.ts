import { describe, expect, it } from "vitest";
import { shareBibtex, shareMailto, shareMarkdown, shareText, shareUrl } from "./share";

const paper = {
  title: "Attention Is All You Need",
  link: "https://arxiv.org/abs/1706.03762v7",
  authors: ["Ashish Vaswani", "Noam Shazeer", "Niki Parmar"],
  published: "2017-06-12T00:00:00Z",
  doi: null,
  arxivId: "1706.03762v7",
  source: "cs.CL",
};

describe("sharing feed items", () => {
  it("picks the lasting address", () => {
    expect(shareUrl(paper)).toBe("https://arxiv.org/abs/1706.03762");
    expect(shareUrl({ ...paper, arxivId: null, doi: "10.1/x" })).toBe("https://doi.org/10.1/x");
    expect(shareUrl({ ...paper, arxivId: null, link: "https://a.b/c" })).toBe("https://a.b/c");
    expect(shareUrl({ ...paper, arxivId: null, link: null })).toBeNull();
  });

  it("writes text, Markdown and email", () => {
    expect(shareText(paper)).toBe(
      "Attention Is All You Need\nAshish Vaswani, Noam Shazeer et al., 2017\nhttps://arxiv.org/abs/1706.03762",
    );
    expect(shareMarkdown(paper)).toBe(
      "[Attention Is All You Need](https://arxiv.org/abs/1706.03762)",
    );
    const m = shareMailto(paper);
    expect(m.startsWith("mailto:?subject=Attention%20Is%20All%20You%20Need&body=")).toBe(true);
    expect(decodeURIComponent(m.split("body=")[1]!)).toContain("arxiv.org/abs/1706.03762");
  });

  it("writes a BibTeX entry", () => {
    const b = shareBibtex(paper);
    expect(b.startsWith("@misc{vaswani2017attention,")).toBe(true);
    expect(b).toContain("eprint = {1706.03762v7}");
    expect(b).toContain("author = {Ashish Vaswani and Noam Shazeer and Niki Parmar}");
    expect(shareBibtex({ ...paper, arxivId: null, doi: "10.1/x" })).toMatch(/^@article\{/);
  });
});
