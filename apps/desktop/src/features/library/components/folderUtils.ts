import type { FolderDto } from "@/lib/ipc";

/** Every folder in the tree, depth-first, with its depth. */
export function flattenFolders(
  tree: FolderDto[],
  depth = 0,
): { folder: FolderDto; depth: number }[] {
  return tree.flatMap((folder) => [
    { folder, depth },
    ...flattenFolders(folder.children, depth + 1),
  ]);
}

export function findFolder(tree: FolderDto[], path: string): FolderDto | undefined {
  for (const f of tree) {
    if (f.path === path) return f;
    if (path.startsWith(`${f.path}/`)) return findFolder(f.children, path);
  }
  return undefined;
}

export function folderLabel(path: string): string {
  return path === "" ? "Books" : path.slice(path.lastIndexOf("/") + 1);
}
