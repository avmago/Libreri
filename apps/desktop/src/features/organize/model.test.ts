import { describe, expect, it } from "vitest";
import { categoryTree } from "./model";

describe("categoryTree", () => {
  it("nests paths and adds up counts", () => {
    const tree = categoryTree([
      { value: "Science/Physics", count: 3 },
      { value: "Science", count: 1 },
      { value: "Science/Biology/Genetics", count: 2 },
      { value: "Art", count: 4 },
    ]);
    expect(tree.map((n) => [n.name, n.count])).toEqual([
      ["Art", 4],
      ["Science", 6],
    ]);
    const science = tree[1]!;
    expect(science.children.map((c) => c.path)).toEqual(["Science/Biology", "Science/Physics"]);
    expect(science.children[0]!.children[0]!.path).toBe("Science/Biology/Genetics");
  });
});
