import type * as React from "react";
import { useState } from "react";
import { Bookmark, ChevronRight, Highlighter, MessageSquare, Trash2 } from "lucide-react";
import type { Annotation } from "@/lib/ipc";
import { highlightFill, type TocItem } from "@/readers";
import { cn } from "@/lib/utils";

function TocTree({
  items,
  depth,
  current,
  onGo,
}: {
  items: TocItem[];
  depth: number;
  current?: string;
  onGo: (target: string) => void;
}) {
  return (
    <ul role={depth === 0 ? "tree" : "group"} className="flex flex-col">
      {items.map((item, i) => (
        <TocRow
          key={`${item.target}-${i}`}
          item={item}
          depth={depth}
          current={current}
          onGo={onGo}
        />
      ))}
    </ul>
  );
}

function TocRow({
  item,
  depth,
  current,
  onGo,
}: {
  item: TocItem;
  depth: number;
  current?: string;
  onGo: (target: string) => void;
}) {
  const [open, setOpen] = useState(depth === 0);
  const active = current !== undefined && item.label.trim() === current.trim();
  return (
    <li
      role="treeitem"
      aria-expanded={item.children.length ? open : undefined}
      aria-selected={active}
    >
      <div
        className={cn(
          "flex min-h-7 items-start gap-1 rounded-md py-1 pr-2 hover:bg-muted/70",
          active && "bg-muted font-medium",
        )}
        style={{ paddingLeft: 4 + depth * 12 }}
      >
        <button
          type="button"
          aria-label={open ? "Collapse" : "Expand"}
          onClick={() => setOpen(!open)}
          className={cn(
            "mt-0.5 rounded p-0.5 text-muted-foreground",
            !item.children.length && "invisible",
          )}
        >
          <ChevronRight className={cn("size-3 transition-transform", open && "rotate-90")} />
        </button>
        <button
          type="button"
          onClick={() => onGo(item.target)}
          className="flex-1 text-left leading-snug"
        >
          {item.label || "Untitled"}
        </button>
      </div>
      {open && item.children.length > 0 && (
        <TocTree items={item.children} depth={depth + 1} current={current} onGo={onGo} />
      )}
    </li>
  );
}

function MarkRow({
  a,
  onGo,
  onDelete,
}: {
  a: Annotation;
  onGo: (a: Annotation) => void;
  onDelete: (a: Annotation) => void;
}) {
  const isBookmark = a.kind === "bookmark";
  return (
    <li className="group relative">
      <button
        type="button"
        onClick={() => onGo(a)}
        className="flex w-full flex-col gap-1 rounded-md px-2.5 py-2 text-left hover:bg-muted/70"
      >
        <span className="flex items-center gap-1.5 text-[11px] text-muted-foreground">
          {isBookmark ? (
            <Bookmark className="size-3" aria-hidden />
          ) : (
            <span
              className="size-2.5 rounded-full border border-black/10"
              style={{ background: highlightFill(a.color, false) }}
              aria-hidden
            />
          )}
          {a.label || (isBookmark ? "Bookmark" : "Highlight")}
        </span>
        {a.quote?.exact && <span className="line-clamp-3 leading-snug">{a.quote.exact}</span>}
        {a.note && (
          <span className="flex gap-1.5 text-[12px] text-muted-foreground">
            <MessageSquare className="mt-0.5 size-3 shrink-0" aria-hidden />
            <span className="line-clamp-3">{a.note}</span>
          </span>
        )}
      </button>
      <button
        type="button"
        aria-label="Delete"
        onClick={() => onDelete(a)}
        className="absolute top-1.5 right-1.5 hidden rounded p-1 text-muted-foreground hover:bg-background hover:text-destructive group-hover:block focus-visible:block"
      >
        <Trash2 className="size-3.5" />
      </button>
    </li>
  );
}

export type LeftPanel = "contents" | "marks" | "markup";

/** Left side of the reader: table of contents, and bookmarks + highlights. */
export function ContentsPanel({
  panel,
  setPanel,
  toc,
  section,
  annotations,
  onGo,
  onShow,
  onDelete,
  markup,
}: {
  panel: LeftPanel;
  setPanel: (p: LeftPanel) => void;
  toc: TocItem[];
  section?: string;
  annotations: Annotation[];
  onGo: (target: string) => void;
  onShow: (a: Annotation) => void;
  onDelete: (a: Annotation) => void;
  /** Fixed-page books: the markup tab's content. */
  markup?: React.ReactNode;
}) {
  const marks = annotations.filter((a) => a.kind !== "markup");
  const markupCount = annotations.length - marks.length;
  const bookmarks = annotations.filter((a) => a.kind === "bookmark");
  const highlights = annotations.filter((a) => a.kind === "highlight");
  return (
    <aside
      aria-label="Contents and marks"
      className="flex w-64 shrink-0 flex-col border-r bg-sidebar"
    >
      <div role="tablist" className="flex h-10 shrink-0 items-end gap-1 border-b px-3">
        {(
          [
            ["contents", "Contents"],
            ["marks", `Marks${marks.length ? ` (${marks.length})` : ""}`],
            ...(markup
              ? ([["markup", `Markup${markupCount ? ` (${markupCount})` : ""}`]] as const)
              : []),
          ] as const
        ).map(([id, label]) => (
          <button
            key={id}
            role="tab"
            type="button"
            aria-selected={panel === id}
            onClick={() => setPanel(id)}
            className={cn(
              "-mb-px border-b-2 px-2 pb-2 text-[12.5px] font-medium",
              panel === id
                ? "border-foreground"
                : "border-transparent text-muted-foreground hover:text-foreground",
            )}
          >
            {label}
          </button>
        ))}
      </div>
      {panel === "markup" && markup ? (
        markup
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-1.5 py-2">
          {panel === "contents" ? (
            toc.length ? (
              <TocTree items={toc} depth={0} current={section} onGo={onGo} />
            ) : (
              <p className="px-3 py-6 text-center text-muted-foreground">
                This book has no table of contents.
              </p>
            )
          ) : marks.length ? (
            <div className="flex flex-col gap-3">
              {bookmarks.length > 0 && (
                <section>
                  <h3 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                    BOOKMARKS
                  </h3>
                  <ul>
                    {bookmarks.map((a) => (
                      <MarkRow key={a.id} a={a} onGo={onShow} onDelete={onDelete} />
                    ))}
                  </ul>
                </section>
              )}
              {highlights.length > 0 && (
                <section>
                  <h3 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                    HIGHLIGHTS
                  </h3>
                  <ul>
                    {highlights.map((a) => (
                      <MarkRow key={a.id} a={a} onGo={onShow} onDelete={onDelete} />
                    ))}
                  </ul>
                </section>
              )}
            </div>
          ) : (
            <div className="flex flex-col items-center gap-2 px-4 py-8 text-center text-muted-foreground">
              <Highlighter className="size-5" aria-hidden />
              <p>Select text to highlight it, or add a bookmark from the toolbar.</p>
            </div>
          )}
        </div>
      )}
    </aside>
  );
}
