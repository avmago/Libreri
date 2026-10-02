import { useEffect, useRef } from "react";
import { ask } from "@tauri-apps/plugin-dialog";
import {
  ChevronRight,
  Folder,
  FolderOpen,
  FolderPlus,
  FolderSearch,
  Pencil,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { ContextMenu, menuContent, menuItem, menuSeparator } from "@/components/ui/menu";
import { commands, unwrap, type FolderDto } from "@/lib/ipc";
import { useTabs } from "@/lib/tabs";
import { closeGoneEverywhere } from "@/lib/tabs/gone";
import { cn } from "@/lib/utils";
import { useCreateFolder, useRenameFolder, useTrashFolder } from "../api";
import { usePermissions } from "@/features/profiles";
import { startDragOnMove, useDrag } from "../drag";
import { useLibraryView } from "../store";

type Editing = { mode: "rename"; path: string } | { mode: "create"; parent: string } | null;

const errorText = (e: unknown) => (e instanceof Error ? e.message : String(e));

function NameInput({
  initial,
  depth,
  onDone,
}: {
  initial: string;
  depth: number;
  onDone: (name: string | null) => void;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const done = useRef(false);
  useEffect(() => {
    ref.current?.focus();
    ref.current?.select();
  }, []);
  const finish = (name: string | null) => {
    if (done.current) return;
    done.current = true;
    onDone(name);
  };
  return (
    <div
      className="flex h-7 items-center gap-1.5 pr-2"
      style={{ paddingLeft: 10 + depth * 14 + 16 }}
    >
      <Folder className="size-4 shrink-0 text-muted-foreground" aria-hidden />
      <input
        ref={ref}
        defaultValue={initial}
        aria-label="Folder name"
        onKeyDown={(e) => {
          if (e.key === "Enter") finish(e.currentTarget.value);
          if (e.key === "Escape") finish(null);
        }}
        onBlur={(e) => finish(e.currentTarget.value)}
        className="h-6 min-w-0 flex-1 rounded border border-ring bg-background px-1.5 text-[13px] outline-none"
      />
    </div>
  );
}

function FolderRow({
  folder,
  depth,
  editing,
  setEditing,
}: {
  folder: FolderDto;
  depth: number;
  editing: Editing;
  setEditing: (e: Editing) => void;
}) {
  const { nav, setNav, expanded, setExpanded, folderMoved } = useLibraryView();
  const { editLibrary } = usePermissions();
  const over = useDrag((s) => s.item !== null && s.target === folder.path);
  const rename = useRenameFolder();
  const create = useCreateFolder();
  const trash = useTrashFolder();
  const active = nav.kind === "folder" && nav.path === folder.path;
  const open = expanded[folder.path] ?? false;
  const hasChildren = folder.children.length > 0;

  const onRename = async (name: string | null) => {
    setEditing(null);
    if (!name || name === folder.name) return;
    try {
      const path = await rename.mutateAsync({ path: folder.path, name });
      // The folder shown may be this one or inside it.
      folderMoved(folder.path, path);
    } catch (e) {
      toast.error("Could not rename the folder", { description: errorText(e) });
    }
  };

  const onCreate = async (name: string | null) => {
    setEditing(null);
    if (!name?.trim()) return;
    try {
      const path = await create.mutateAsync({ parent: folder.path, name });
      setExpanded(folder.path, true);
      setNav({ kind: "folder", path });
    } catch (e) {
      toast.error("Could not create the folder", { description: errorText(e) });
    }
  };

  const onTrash = async () => {
    const n = folder.totalCount;
    const ok = await ask(
      n === 0
        ? "The empty folder goes to the system Trash."
        : `The folder and the ${n === 1 ? "book" : `${n} books`} in it go to the system Trash. Restoring them from there brings their details back.`,
      { title: `Move “${folder.name}” to the Trash?`, kind: "warning", okLabel: "Move to Trash" },
    );
    if (!ok) return;
    try {
      // The books inside, whose open tabs close with the folder.
      const inside = useTabs.getState().tabs.length
        ? await unwrap(commands.listBooks({ folder: folder.path, includeSubfolders: true })).catch(
            () => [],
          )
        : [];
      await trash.mutateAsync(folder.path);
      closeGoneEverywhere(inside.map((b) => b.id));
      if (
        nav.kind === "folder" &&
        (nav.path === folder.path || nav.path.startsWith(`${folder.path}/`))
      ) {
        setNav({ kind: "all" });
      }
    } catch (e) {
      toast.error("Could not move the folder to the Trash", { description: errorText(e) });
    }
  };

  if (editing?.mode === "rename" && editing.path === folder.path) {
    return <NameInput initial={folder.name} depth={depth} onDone={onRename} />;
  }

  return (
    <>
      <ContextMenu.Root>
        <ContextMenu.Trigger asChild>
          <div
            role="treeitem"
            aria-expanded={hasChildren ? open : undefined}
            aria-selected={active}
            data-drop-folder={folder.path}
            onClick={() => {
              if (useDrag.getState().justDropped) return;
              setNav({ kind: "folder", path: folder.path });
              if (hasChildren) setExpanded(folder.path, true);
            }}
            onPointerDown={
              editLibrary
                ? startDragOnMove(() => ({
                    kind: "folder",
                    path: folder.path,
                    label: folder.name,
                  }))
                : undefined
            }
            className={cn(
              "group flex h-7 cursor-default items-center gap-1.5 rounded-md pr-2",
              active ? "bg-muted font-medium" : "hover:bg-muted/60",
              over && "bg-muted ring-2 ring-primary ring-inset",
            )}
            style={{ paddingLeft: 10 + depth * 14 }}
          >
            <button
              type="button"
              aria-label={open ? "Collapse" : "Expand"}
              onClick={(e) => {
                e.stopPropagation();
                setExpanded(folder.path, !open);
              }}
              className={cn("rounded p-0.5 text-muted-foreground", !hasChildren && "invisible")}
            >
              <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
            </button>
            {active ? (
              <FolderOpen className="size-4 shrink-0" aria-hidden />
            ) : (
              <Folder className="size-4 shrink-0 text-muted-foreground" aria-hidden />
            )}
            <span className="flex-1 truncate">{folder.name}</span>
            <span className="text-[11px] text-muted-foreground">{folder.totalCount || ""}</span>
          </div>
        </ContextMenu.Trigger>
        <ContextMenu.Portal>
          <ContextMenu.Content className={menuContent}>
            {editLibrary && (
              <>
                <ContextMenu.Item
                  className={menuItem}
                  onSelect={() => setEditing({ mode: "create", parent: folder.path })}
                >
                  <FolderPlus /> New folder inside
                </ContextMenu.Item>
                <ContextMenu.Item
                  className={menuItem}
                  onSelect={() => setEditing({ mode: "rename", path: folder.path })}
                >
                  <Pencil /> Rename
                </ContextMenu.Item>
              </>
            )}
            <ContextMenu.Item
              className={menuItem}
              onSelect={() =>
                unwrap(commands.revealFolder(folder.path)).catch((e) =>
                  toast.error("Could not open the folder", { description: errorText(e) }),
                )
              }
            >
              <FolderSearch /> Open in file manager
            </ContextMenu.Item>
            {editLibrary && (
              <>
                <ContextMenu.Separator className={menuSeparator} />
                <ContextMenu.Item
                  className={cn(menuItem, "text-destructive [&_svg]:!text-destructive")}
                  onSelect={() => void onTrash()}
                >
                  <Trash2 /> Move to Trash
                </ContextMenu.Item>
              </>
            )}
          </ContextMenu.Content>
        </ContextMenu.Portal>
      </ContextMenu.Root>
      {(open || (editing?.mode === "create" && editing.parent === folder.path)) && (
        <div role="group">
          {editing?.mode === "create" && editing.parent === folder.path && (
            <NameInput initial="New folder" depth={depth + 1} onDone={onCreate} />
          )}
          {open &&
            folder.children.map((c) => (
              <FolderRow
                key={c.path}
                folder={c}
                depth={depth + 1}
                editing={editing}
                setEditing={setEditing}
              />
            ))}
        </div>
      )}
    </>
  );
}

/**
 * The real folders under Books/. Click to browse, drag books or folders onto
 * a folder to move them, right-click for more.
 */
export function FolderTree({
  folders,
  editing,
  setEditing,
}: {
  folders: FolderDto[];
  editing: Editing;
  setEditing: (e: Editing) => void;
}) {
  const create = useCreateFolder();
  const setNav = useLibraryView((s) => s.setNav);

  const onCreateTop = async (name: string | null) => {
    setEditing(null);
    if (!name?.trim()) return;
    try {
      const path = await create.mutateAsync({ parent: "", name });
      setNav({ kind: "folder", path });
    } catch (e) {
      toast.error("Could not create the folder", { description: errorText(e) });
    }
  };

  return (
    <div role="tree" aria-label="Folders" className="flex flex-col gap-px">
      {editing?.mode === "create" && editing.parent === "" && (
        <NameInput initial="New folder" depth={0} onDone={onCreateTop} />
      )}
      {folders.map((f) => (
        <FolderRow key={f.path} folder={f} depth={0} editing={editing} setEditing={setEditing} />
      ))}
      {folders.length === 0 && editing === null && (
        <p className="px-2.5 py-1 text-[12px] text-muted-foreground">
          No folders yet. Folders you create here are real folders inside Books.
        </p>
      )}
    </div>
  );
}

export type { Editing as FolderEditing };
