import { describe, expect, it } from "vitest";
import { findQuote, firstPagePart, makeQuote } from "./quote";

describe("text quotes", () => {
  const text = "the cat sat on the mat. later the cat sat on the hat.";

  it("records context around a selection", () => {
    const q = makeQuote(text, 4, 7);
    expect(q.exact).toBe("cat");
    expect(q.prefix).toBe("the ");
    expect(q.suffix?.startsWith(" sat on the mat")).toBe(true);
  });

  it("finds the right occurrence again using the context", () => {
    const second = text.lastIndexOf("cat");
    const q = makeQuote(text, second, second + 3);
    const edited = `A new first line. ${text}`;
    expect(findQuote(edited, q)).toEqual([second + 18, second + 21]);
    expect(findQuote(edited, { exact: "dog", prefix: "", suffix: "" })).toBeNull();
  });
});

describe("selections across pages", () => {
  it("keeps only the part on the first page", () => {
    document.body.innerHTML =
      '<div id="p1"><div class="t"><span>end of one</span></div></div>' +
      '<div id="p2"><div class="t"><span>start of two</span></div></div>';
    const [one, two] = Array.from(document.querySelectorAll("span"));
    const range = document.createRange();
    range.setStart(one!.firstChild!, 4);
    range.setEnd(two!.firstChild!, 5);
    const page = document.getElementById("p1")!;
    const part = firstPagePart(range, page, page.querySelector<HTMLElement>(".t")!);
    expect(part.toString()).toBe("of one");
    expect(firstPagePart(part, page, page).toString()).toBe("of one");
  });
});
