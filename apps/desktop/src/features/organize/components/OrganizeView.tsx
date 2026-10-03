import { useMemo, useState } from "react";
import {
  ArrowUpRight,
  ChevronRight,
  Combine,
  Pencil,
  Search,
  Shapes,
  Sparkles,
  Tag,
  Trash2,
} from "lucide-react";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useFacets, useLibraryView } from "@/features/library";
import { useShortcut } from "@/lib/shortcuts";
import { cn } from "@/lib/utils";
import {
  useDeleteCategory,
  useDeleteTag,
  useMergeTags,
  useRenameCategory,
  useRenameTag,
  useSimilarTags,
} from "../api";
import { categoryTree, type CategoryNode } from "../model";

type Editing =
  | { kind: "renameTag"; tag: string }
  | { kind: "mergeTags"; tags: string[] }
  | { kind: "moveCategory"; path: string }
  | null;

const done = (n: number, what: string) =>
  toast.success(`${what} on ${n === 1 ? "1 book" : `${n} books`}`);
const failed = (what: string) => (e: unknown) => toast.error(what, { description: String(e) });

/** Tidy tags and categories across the whole library. */
export function OrganizeView() {
  const { data: facets } = useFacets();
  const { data: similar = [] } = useSimilarTags();
  const setNav = useLibraryView((s) => s.setNav);
  const renameTag = useRenameTag();
  const mergeTags = useMergeTags();
  const deleteTag = useDeleteTag();
  const renameCategory = useRenameCategory();
  const deleteCategory = useDeleteCategory();
  const [filter, setFilter] = useState("");
  const [selected, setSelected] = useState<string[]>([]);
  const [focus, setFocus] = useState<{ kind: "tag" | "category"; value: string } | null>(null);
  const [editing, setEditing] = useState<Editing>(null);

  /** Tags renamed, merged or removed leave the selection (focus follows a rename). */
  const tagsGone = (gone: string[], now?: string) => {
    setSelected((s) => s.filter((t) => !gone.includes(t)));
    setFocus((f) =>
      f?.kind === "tag" && gone.includes(f.value) ? (now ? { kind: "tag", value: now } : null) : f,
    );
  };
  /** Same for a category, and the categories inside it. */
  const categoryGone = (path: string, now?: string) =>
    setFocus((f) => {
      if (f?.kind !== "category" || (f.value !== path && !f.value.startsWith(`${path}/`))) return f;
      return now ? { kind: "category", value: now + f.value.slice(path.length) } : null;
    });

  const tags = useMemo(
    () =>
      (facets?.tags ?? []).filter((t) =>
        t.value.toLowerCase().includes(filter.trim().toLowerCase()),
      ),
    [facets, filter],
  );
  const tree = useMemo(() => categoryTree(facets?.categories ?? []), [facets]);

  const removeTag = async (tag: string) => {
    const n = facets?.tags.find((t) => t.value === tag)?.count ?? 0;
    const ok = await ask(
      `“${tag}” is removed from ${n === 1 ? "1 book" : `${n} books`}. The books stay.`,
      {
        title: "Remove this tag?",
        kind: "warning",
        okLabel: "Remove tag",
      },
    );
    if (ok)
      deleteTag.mutate(tag, {
        onSuccess: (n) => {
          tagsGone([tag]);
          done(n, `Removed “${tag}”`);
        },
        onError: failed("Could not remove the tag"),
      });
  };
  const removeCategory = async (path: string) => {
    const ok = await ask(
      `“${path}” and the categories inside it are removed from every book. The books stay.`,
      {
        title: "Remove this category?",
        kind: "warning",
        okLabel: "Remove category",
      },
    );
    if (ok) {
      deleteCategory.mutate(path, {
        onSuccess: (n) => {
          categoryGone(path);
          done(n, `Removed “${path}”`);
        },
        onError: failed("Could not remove the category"),
      });
    }
  };

  useShortcut("organize.rename", () => {
    if (focus?.kind === "tag") setEditing({ kind: "renameTag", tag: focus.value });
    if (focus?.kind === "category") setEditing({ kind: "moveCategory", path: focus.value });
  });
  useShortcut(
    "organize.merge",
    () => selected.length > 1 && setEditing({ kind: "mergeTags", tags: selected }),
  );
  useShortcut("organize.delete", () => {
    if (focus?.kind === "tag") void removeTag(focus.value);
    if (focus?.kind === "category") void removeCategory(focus.value);
  });

  return (
    <div className="flex h-full min-w-0 flex-col">
      <header className="flex items-center gap-3 px-6 pt-4 pb-3">
        <h1 className="text-xl font-semibold tracking-tight">Organize</h1>
        <p className="text-muted-foreground">
          Rename, merge and tidy the tags and categories of the whole library.
        </p>
      </header>
      <div className="grid min-h-0 flex-1 grid-cols-[minmax(0,1.1fr)_minmax(0,1fr)] gap-0 border-t">
        {/* Tags */}
        <section aria-label="Tags" className="flex min-h-0 flex-col border-r">
          <div className="flex items-center gap-2 px-4 py-2.5">
            <Tag className="size-4 text-muted-foreground" aria-hidden />
            <h2 className="font-semibold">Tags</h2>
            <span className="text-[12px] text-muted-foreground">{facets?.tags.length ?? 0}</span>
            <div className="flex-1" />
            {selected.length > 1 && (
              <Button size="sm" onClick={() => setEditing({ kind: "mergeTags", tags: selected })}>
                <Combine /> Merge {selected.length}
              </Button>
            )}
            <div className="relative w-52">
              <Search
                className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
                aria-hidden
              />
              <Input
                value={filter}
                onChange={(e) => setFilter(e.target.value)}
                placeholder="Find a tag"
                aria-label="Find a tag"
                className="pl-8"
              />
            </div>
          </div>

          {similar.length > 0 && (
            <div
              className="mx-4 mb-3 flex flex-col gap-2 rounded-lg border bg-sidebar p-3"
              aria-label="Similar tags"
            >
              <p className="flex items-center gap-2 text-[12.5px] font-medium">
                <Sparkles className="size-3.5 text-muted-foreground" aria-hidden />
                These tags look like the same thing
              </p>
              {similar.map((group) => {
                const into = group[0]!.name;
                return (
                  <div key={group.map((g) => g.name).join()} className="flex items-center gap-2">
                    <div className="flex min-w-0 flex-1 flex-wrap gap-1.5">
                      {group.map((g) => (
                        <span
                          key={g.name}
                          className="rounded-full border bg-background px-2 py-0.5 text-[12px]"
                        >
                          {g.name} <span className="text-muted-foreground">{g.count}</span>
                        </span>
                      ))}
                    </div>
                    <Button
                      size="sm"
                      variant="outline"
                      onClick={() =>
                        mergeTags.mutate(
                          { sources: group.slice(1).map((g) => g.name), into },
                          {
                            onSuccess: (n) => {
                              tagsGone(group.slice(1).map((g) => g.name));
                              done(n, `Merged into “${into}”`);
                            },
                            onError: failed("Could not merge"),
                          },
                        )
                      }
                    >
                      Merge into “{into}”
                    </Button>
                  </div>
                );
              })}
            </div>
          )}

          <ul
            className="min-h-0 flex-1 overflow-y-auto px-2 pb-4"
            role="listbox"
            data-shortcuts
            aria-multiselectable
            aria-label="All tags"
          >
            {tags.map(({ value, count }) => {
              const isSelected = selected.includes(value);
              return (
                <li
                  key={value}
                  role="option"
                  aria-selected={isSelected}
                  tabIndex={0}
                  onFocus={() => setFocus({ kind: "tag", value })}
                  onClick={(e) => {
                    setFocus({ kind: "tag", value });
                    if (e.metaKey || e.ctrlKey || e.shiftKey) {
                      setSelected(
                        isSelected ? selected.filter((s) => s !== value) : [...selected, value],
                      );
                    } else setSelected([value]);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === " ") {
                      e.preventDefault();
                      setSelected(
                        isSelected ? selected.filter((s) => s !== value) : [...selected, value],
                      );
                    }
                  }}
                  className={cn(
                    "group flex h-8 cursor-default items-center gap-2 rounded-md px-2.5 outline-none focus-visible:ring-2 focus-visible:ring-ring",
                    isSelected ? "bg-muted" : "hover:bg-muted/60",
                  )}
                >
                  <input
                    type="checkbox"
                    tabIndex={-1}
                    aria-hidden
                    checked={isSelected}
                    readOnly
                    className="size-3.5 accent-primary"
                  />
                  <span className="flex-1 truncate">{value}</span>
                  <span className="text-[12px] text-muted-foreground tabular-nums">{count}</span>
                  <span className="flex opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
                    <IconButton
                      label="Show books"
                      onClick={() => setNav({ kind: "tag", tag: value })}
                      icon={ArrowUpRight}
                    />
                    <IconButton
                      label="Rename"
                      onClick={() => setEditing({ kind: "renameTag", tag: value })}
                      icon={Pencil}
                    />
                    <IconButton
                      label="Remove"
                      onClick={() => void removeTag(value)}
                      icon={Trash2}
                    />
                  </span>
                </li>
              );
            })}
            {tags.length === 0 && (
              <p className="px-2.5 py-6 text-center text-muted-foreground">
                {facets?.tags.length
                  ? "No tag matches."
                  : "No tags yet. Add them in a book's details or with bulk edit."}
              </p>
            )}
          </ul>
        </section>

        {/* Categories */}
        <section aria-label="Categories" className="flex min-h-0 flex-col">
          <div className="flex items-center gap-2 px-4 py-2.5">
            <Shapes className="size-4 text-muted-foreground" aria-hidden />
            <h2 className="font-semibold">Categories</h2>
            <span className="text-[12px] text-muted-foreground">
              {facets?.categories.length ?? 0}
            </span>
          </div>
          <p className="px-4 pb-2 text-[12px] text-muted-foreground">
            Categories nest with “/”, such as Science/Physics. Moving one moves everything inside
            it.
          </p>
          <div
            role="tree"
            aria-label="Category tree"
            className="min-h-0 flex-1 overflow-y-auto px-2 pb-4"
          >
            {tree.map((n) => (
              <CategoryRow
                key={n.path}
                node={n}
                depth={0}
                focused={focus?.kind === "category" ? focus.value : null}
                onFocus={(path) => setFocus({ kind: "category", value: path })}
                onShow={(path) => setNav({ kind: "category", path })}
                onMove={(path) => setEditing({ kind: "moveCategory", path })}
                onRemove={(path) => void removeCategory(path)}
              />
            ))}
            {tree.length === 0 && (
              <p className="px-2.5 py-6 text-center text-muted-foreground">No categories yet.</p>
            )}
          </div>
        </section>
      </div>

      <NameDialog
        editing={editing}
        onClose={() => setEditing(null)}
        onSubmit={(value) => {
          if (!editing) return;
          if (editing.kind === "renameTag") {
            renameTag.mutate(
              { from: editing.tag, to: value },
              {
                onSuccess: (n) => {
                  tagsGone([editing.tag], value);
                  done(n, `Renamed to “${value}”`);
                },
                onError: failed("Could not rename the tag"),
              },
            );
          } else if (editing.kind === "mergeTags") {
            mergeTags.mutate(
              { sources: editing.tags.filter((t) => t !== value), into: value },
              {
                onSuccess: (n) => {
                  setSelected([]);
                  tagsGone(editing.tags, value);
                  done(n, `Merged into “${value}”`);
                },
                onError: failed("Could not merge the tags"),
              },
            );
          } else {
            renameCategory.mutate(
              { from: editing.path, to: value },
              {
                onSuccess: (n) => {
                  categoryGone(editing.path, value);
                  done(n, `Moved to “${value}”`);
                },
                onError: failed("Could not move the category"),
              },
            );
          }
          setEditing(null);
        }}
      />
    </div>
  );
}

function IconButton({
  label,
  onClick,
  icon: Icon,
}: {
  label: string;
  onClick: () => void;
  icon: typeof Tag;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      onClick={(e) => {
        e.stopPropagation();
        onClick();
      }}
      className="rounded p-1 text-muted-foreground hover:bg-background hover:text-foreground [&_svg]:size-3.5"
    >
      <Icon />
    </button>
  );
}

function CategoryRow({
  node,
  depth,
  focused,
  onFocus,
  onShow,
  onMove,
  onRemove,
}: {
  node: CategoryNode;
  depth: number;
  focused: string | null;
  onFocus: (path: string) => void;
  onShow: (path: string) => void;
  onMove: (path: string) => void;
  onRemove: (path: string) => void;
}) {
  const [open, setOpen] = useState(depth === 0);
  const has = node.children.length > 0;
  return (
    <div role="none">
      <div
        role="treeitem"
        aria-expanded={has ? open : undefined}
        aria-selected={focused === node.path}
        tabIndex={0}
        onFocus={() => onFocus(node.path)}
        onClick={() => onFocus(node.path)}
        onKeyDown={(e) => {
          if (e.key === "ArrowRight") setOpen(true);
          if (e.key === "ArrowLeft") setOpen(false);
          if (e.key === "Enter") onShow(node.path);
        }}
        className={cn(
          "group flex h-8 cursor-default items-center gap-1.5 rounded-md pr-2 outline-none focus-visible:ring-2 focus-visible:ring-ring",
          focused === node.path ? "bg-muted" : "hover:bg-muted/60",
        )}
        style={{ paddingLeft: 8 + depth * 16 }}
      >
        <button
          type="button"
          tabIndex={-1}
          aria-label={open ? "Collapse" : "Expand"}
          onClick={(e) => {
            e.stopPropagation();
            setOpen(!open);
          }}
          className={cn("rounded p-0.5 text-muted-foreground", !has && "invisible")}
        >
          <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
        </button>
        <span className="flex-1 truncate">{node.name}</span>
        <span className="text-[12px] text-muted-foreground tabular-nums">{node.count}</span>
        <span className="flex opacity-0 group-focus-within:opacity-100 group-hover:opacity-100">
          <IconButton label="Show books" onClick={() => onShow(node.path)} icon={ArrowUpRight} />
          <IconButton label="Rename or move" onClick={() => onMove(node.path)} icon={Pencil} />
          <IconButton label="Remove" onClick={() => onRemove(node.path)} icon={Trash2} />
        </span>
      </div>
      {open &&
        node.children.map((c) => (
          <CategoryRow
            key={c.path}
            node={c}
            depth={depth + 1}
            focused={focused}
            onFocus={onFocus}
            onShow={onShow}
            onMove={onMove}
            onRemove={onRemove}
          />
        ))}
    </div>
  );
}

function NameDialog({
  editing,
  onClose,
  onSubmit,
}: {
  editing: Editing;
  onClose: () => void;
  onSubmit: (value: string) => void;
}) {
  const title =
    editing?.kind === "renameTag"
      ? `Rename “${editing.tag}”`
      : editing?.kind === "mergeTags"
        ? `Merge ${editing.tags.length} tags`
        : editing?.kind === "moveCategory"
          ? `Rename or move “${editing.path}”`
          : "";
  const description =
    editing?.kind === "renameTag"
      ? "Every book with this tag gets the new name. If the name is already a tag, the two merge."
      : editing?.kind === "mergeTags"
        ? `Books tagged ${editing.tags.map((t) => `“${t}”`).join(", ")} get one tag instead.`
        : "Write the full path. Use “/” to put it inside another category, such as Science/Physics.";
  const initial =
    editing?.kind === "renameTag"
      ? editing.tag
      : editing?.kind === "mergeTags"
        ? (editing.tags[0] ?? "")
        : (editing?.path ?? "");
  return (
    <Dialog
      open={editing !== null}
      onOpenChange={(o) => !o && onClose()}
      title={title}
      description={description}
    >
      {editing && (
        <NameForm
          key={title}
          initial={initial}
          options={editing.kind === "mergeTags" ? editing.tags : []}
          onClose={onClose}
          onSubmit={onSubmit}
        />
      )}
    </Dialog>
  );
}

function NameForm({
  initial,
  options,
  onClose,
  onSubmit,
}: {
  initial: string;
  options: string[];
  onClose: () => void;
  onSubmit: (v: string) => void;
}) {
  const [value, setValue] = useState(initial);
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        if (value.trim()) onSubmit(value.trim());
      }}
    >
      {options.length > 0 && (
        <div className="flex flex-wrap gap-1.5">
          {options.map((o) => (
            <button
              key={o}
              type="button"
              onClick={() => setValue(o)}
              aria-pressed={value === o}
              className={cn(
                "rounded-full border px-2.5 py-0.5 text-[12px]",
                value === o && "border-primary bg-muted",
              )}
            >
              {o}
            </button>
          ))}
        </div>
      )}
      <Input
        value={value}
        onChange={(e) => setValue(e.target.value)}
        autoFocus
        aria-label="New name"
      />
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button type="submit" disabled={!value.trim()}>
          Save
        </Button>
      </div>
    </form>
  );
}
