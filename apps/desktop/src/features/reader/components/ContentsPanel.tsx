import { ResizablePanel } from "@/components/ResizablePanel";
import type * as React from "react";
import { useState } from "react";
import {
  Bookmark,
  Camera,
  ChevronRight,
  FileText,
  Highlighter,
  MessageSquare,
  Mic,
  Trash2,
} from "lucide-react";
import { VoicePlayer, voiceOf } from "@/features/speech";
import { captureOf } from "../capture/model";
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
  onOpenCapture,
}: {
  a: Annotation;
  onGo: (a: Annotation) => void;
  onDelete: (a: Annotation) => void;
  onOpenCapture?: (path: string, title: string) => void;
}) {
  const isBookmark = a.kind === "bookmark";
  const capture = captureOf(a);
  const voice = a.kind === "voice" ? voiceOf(a.locator) : null;
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
          ) : voice ? (
            <Mic className="size-3" aria-hidden />
          ) : capture ? (
            <Camera className="size-3" aria-hidden />
          ) : (
            <span
              className="size-2.5 rounded-full border border-black/10"
              style={{ background: highlightFill(a.color, false) }}
              aria-hidden
            />
          )}
          {a.label || (isBookmark ? "Bookmark" : voice ? "Voice note" : "Highlight")}
          {capture && ` · ${capture.title}`}
        </span>
        {a.quote?.exact && <span className="line-clamp-3 leading-snug">{a.quote.exact}</span>}
        {a.note && (
          <span className="flex gap-1.5 text-[12px] text-muted-foreground">
            {capture ? (
              <span className="line-clamp-3">{a.note}</span>
            ) : voice ? (
              <span className="line-clamp-4 italic">“{a.note}”</span>
            ) : (
              <>
                <MessageSquare className="mt-0.5 size-3 shrink-0" aria-hidden />
                <span className="line-clamp-3">{a.note}</span>
              </>
            )}
          </span>
        )}
      </button>
      {voice && <VoicePlayer path={voice} className="mb-1.5 px-1.5" />}
      {capture && onOpenCapture && (
        <button
          type="button"
          onClick={() => onOpenCapture(capture.path, capture.title)}
          className="mx-2.5 mb-2 flex items-center gap-1.5 rounded-md border px-2 py-1 text-[12px] hover:bg-muted"
        >
          <FileText className="size-3.5" aria-hidden /> Show {capture.pages}{" "}
          {capture.pages === 1 ? "page" : "pages"}
        </button>
      )}
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

export type LeftPanel = "contents" | "marks" | "markup" | "links";

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
  onOpenCapture,
  markup,
  links,
  linkCount = 0,
}: {
  panel: LeftPanel;
  setPanel: (p: LeftPanel) => void;
  toc: TocItem[];
  section?: string;
  annotations: Annotation[];
  onGo: (target: string) => void;
  onShow: (a: Annotation) => void;
  onDelete: (a: Annotation) => void;
  onOpenCapture?: (path: string, title: string) => void;
  /** Fixed-page books: the markup tab's content. */
  markup?: React.ReactNode;
  /** The links tab's content (the player and the book's links). */
  links?: React.ReactNode;
  linkCount?: number;
}) {
  const marks = annotations.filter((a) => a.kind !== "markup" && a.kind !== "link");
  const markupCount = annotations.filter((a) => a.kind === "markup").length;
  const bookmarks = annotations.filter((a) => a.kind === "bookmark");
  const highlights = annotations.filter((a) => a.kind === "highlight");
  const voices = annotations.filter((a) => a.kind === "voice");
  const captures = annotations.filter((a) => a.kind === "capture");
  return (
    <ResizablePanel
      key={panel === "links" ? "links" : "side"}
      id={panel === "links" ? "reader.links" : "reader.contents"}
      initial={panel === "links" ? 320 : 256}
      min={200}
      max={640}
      side="left"
      label="contents and marks"
    >
      <aside
        aria-label="Contents and marks"
        className="flex min-w-0 flex-1 flex-col border-r bg-sidebar"
      >
        <div role="tablist" className="flex h-10 shrink-0 items-end gap-1 border-b px-3">
          {(
            [
              ["contents", "Contents"],
              ["marks", `Marks${marks.length ? ` (${marks.length})` : ""}`],
              ...(markup
                ? ([["markup", `Markup${markupCount ? ` (${markupCount})` : ""}`]] as const)
                : []),
              ...(links
                ? ([["links", `Links${linkCount ? ` (${linkCount})` : ""}`]] as const)
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
        ) : panel === "links" && links ? (
          links
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
                {captures.length > 0 && (
                  <section>
                    <h3 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                      PAPER NOTES
                    </h3>
                    <ul>
                      {captures.map((a) => (
                        <MarkRow
                          key={a.id}
                          a={a}
                          onGo={onShow}
                          onDelete={onDelete}
                          onOpenCapture={onOpenCapture}
                        />
                      ))}
                    </ul>
                  </section>
                )}
                {voices.length > 0 && (
                  <section>
                    <h3 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                      VOICE NOTES
                    </h3>
                    <ul>
                      {voices.map((a) => (
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
                <p>
                  Select text to highlight it, or add a bookmark or a voice note from the toolbar.
                </p>
              </div>
            )}
          </div>
        )}
      </aside>
    </ResizablePanel>
  );
}
