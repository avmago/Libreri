export interface CategoryNode {
  name: string;
  path: string;
  /** Books in this category or below it. */
  count: number;
  children: CategoryNode[];
}

/** Builds the category tree from "Science/Physics" paths with book counts. */
export function categoryTree(paths: { value: string; count: number }[]): CategoryNode[] {
  const root: CategoryNode = { name: "", path: "", count: 0, children: [] };
  for (const { value, count } of paths) {
    let node = root;
    const parts = value.split("/");
    parts.forEach((part, i) => {
      const path = parts.slice(0, i + 1).join("/");
      let child = node.children.find((c) => c.name.toLowerCase() === part.toLowerCase());
      if (!child) {
        child = { name: part, path, count: 0, children: [] };
        node.children.push(child);
      }
      child.count += count;
      node = child;
    });
  }
  const sort = (n: CategoryNode) => {
    n.children.sort((a, b) => a.name.localeCompare(b.name));
    n.children.forEach(sort);
  };
  sort(root);
  return root.children;
}
