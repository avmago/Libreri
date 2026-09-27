import { describe, expect, it } from "vitest";
import { cleanQuery, defaultPicks, FIELDS, full, matchLabel, merge, show } from "./model";

const current = full({
  title: "Linear algebra",
  authors: ["John Smith"],
  tags: ["maths"],
  contentType: "book",
});

const found = full({
  title: "Linear Algebra Done Right",
  authors: ["John Smith"],
  publisher: "Acme Press",
  year: 2015,
  tags: ["Maths", "Vector spaces"],
  categories: ["Mathematics/Algebra"],
  contentType: "textbook",
});

describe("merging found details", () => {
  it("takes empty fields and new tags by default, not replacements", () => {
    const picks = defaultPicks(current, found, 0);
    expect(Object.keys(picks).sort()).toEqual(
      ["categories", "contentType", "publisher", "tags", "year"].sort(),
    );
    const out = merge(current, [found], picks);
    expect(out.title).toBe("Linear algebra");
    expect(out.publisher).toBe("Acme Press");
    expect(out.tags).toEqual(["maths", "Vector spaces"]);
    expect(out.contentType).toBe("textbook");
  });

  it("takes each field from the candidate it was picked from", () => {
    const other = full({ title: "Other", publisher: "Other Press", year: 1999 });
    const out = merge(current, [found, other], { title: 0, publisher: 1, year: 0 });
    expect(out.title).toBe("Linear Algebra Done Right");
    expect(out.publisher).toBe("Other Press");
    expect(out.year).toBe(2015);
  });

  it("keeps a specific type", () => {
    const paper = { ...current, contentType: "researchPaper" as const };
    expect(defaultPicks(paper, found, 0).contentType).toBeUndefined();
  });

  it("shows values as text", () => {
    const f = (k: string) => FIELDS.find((x) => x.key === k)!;
    expect(show(f("contentType"), "textbook")).toBe("Textbook");
    expect(show(f("categories"), ["A", "B/C"])).toBe("A · B/C");
    expect(show(f("year"), null)).toBe("");
  });

  it("labels matches and tidies queries", () => {
    const c = { source: "openLibrary" as const, sourceId: "x", link: null, coverUrl: null };
    expect(matchLabel({ ...c, metadata: found, score: 1 })).toBe("Same identifier");
    expect(matchLabel({ ...c, metadata: found, score: 0.9 })).toBe("Close match");
    expect(matchLabel({ ...c, metadata: found, score: 0.2 })).toBe("Weak match");
    expect(cleanQuery({ isbn: "  ", title: " Dune " })).toEqual({
      isbn: null,
      doi: null,
      arxivId: null,
      title: "Dune",
      author: null,
      contentType: null,
    });
  });
});
