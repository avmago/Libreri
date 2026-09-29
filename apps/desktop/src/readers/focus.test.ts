import { describe, expect, it } from "vitest";
import * as CFI from "foliate-js/epubcfi.js";
import {
  applyBionic,
  bionicFilter,
  bionicPieces,
  boldLength,
  liftBoundary,
  removeBionic,
} from "./focus";

describe("bionic reading", () => {
  it("makes about half of each word bold", () => {
    expect(boldLength(1, 0.5)).toBe(1);
    expect(boldLength(3, 0.5)).toBe(1);
    expect(boldLength(4, 0.5)).toBe(2);
    expect(boldLength(10, 0.5)).toBe(5);
    expect(boldLength(10, 0.3)).toBe(3);
  });

  it("splits words and keeps the text", () => {
    const text = "Hello, wörld — naïve café 42 日本語!";
    const pieces = bionicPieces(text, 0.5);
    expect(pieces.map((p) => p.text).join("")).toBe(text);
    expect(pieces.filter((p) => p.kind === "bold").map((p) => p.text)).toEqual([
      "Hel",
      "wör",
      "naï",
      "ca",
    ]);
    // Always ends with plain text.
    expect(pieces.at(-1)?.kind).toBe("text");
  });

  it("keeps accents with their letter", () => {
    const decomposed = "cafés";
    const pieces = bionicPieces(decomposed, 0.8);
    expect(pieces.map((p) => p.text).join("")).toBe(decomposed);
    const bold = pieces.find((p) => p.kind === "bold")!.text;
    expect(bold).toBe("café");
  });

  it("goes on and comes off without changing the text", () => {
    document.body.innerHTML =
      "<article><p>The quick <em>brown</em> fox.</p><pre>code stays</pre></article>";
    const root = document.querySelector("article")!;
    const before = root.textContent;
    const html = root.innerHTML;
    applyBionic(root, { fixation: 0.5, fade: 0.7 });
    expect(root.textContent).toBe(before);
    expect(root.querySelectorAll("lb-b").length).toBe(4);
    expect(root.querySelector("pre")!.innerHTML).toBe("code stays");
    removeBionic(root);
    expect(root.innerHTML).toBe(html);
  });

  it("gives EPUB positions as if it were off", () => {
    document.body.innerHTML =
      "<div><p>Call me Ishmael. Some <i>years</i> ago, never mind how long.</p></div>";
    const p = document.querySelector("p")!;
    const cfiOf = (start: Node, so: number, end: Node, eo: number) => {
      const s = liftBoundary(start, so);
      const e = liftBoundary(end, eo);
      return CFI.fromRange(
        {
          startContainer: s.node,
          startOffset: s.offset,
          endContainer: e.node,
          endOffset: e.offset,
          collapsed: false,
        },
        bionicFilter,
      );
    };
    // "Ishmael" and "long" before.
    const first = p.firstChild!;
    const last = p.lastChild!;
    const plain = [
      cfiOf(first, 8, first, 15),
      cfiOf(
        last,
        (last as Text).data.indexOf("long"),
        last,
        (last as Text).data.indexOf("long") + 4,
      ),
    ];

    applyBionic(p.parentElement!, { fixation: 0.5, fade: 1 });
    // Find the same words in the wrapped text.
    const at = (word: string) => {
      const walker = document.createTreeWalker(p, NodeFilter.SHOW_TEXT);
      const nodes: Text[] = [];
      for (let n = walker.nextNode(); n; n = walker.nextNode()) nodes.push(n as Text);
      const i = nodes.findIndex(
        (n, k) =>
          n.data + (nodes[k + 1]?.data ?? "") === word && n.parentElement?.localName === "lb-b",
      );
      return { start: nodes[i]!, end: nodes[i + 1]! };
    };
    const ish = at("Ishmael");
    const long = at("long");
    const wrapped = [
      cfiOf(ish.start, 0, ish.end, ish.end.data.length),
      cfiOf(long.start, 0, long.end, long.end.data.length),
    ];
    expect(wrapped).toEqual(plain);

    // And the positions find the same words.
    for (const [i, word] of ["Ishmael", "long"].entries()) {
      const range = CFI.toRange(document, CFI.parse(plain[i]!), bionicFilter);
      expect(range.toString()).toBe(word);
    }
  });
});
