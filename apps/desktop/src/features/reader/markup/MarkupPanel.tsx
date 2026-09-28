import { useMemo, useState } from "react";
import { Eye, EyeOff, Plus, Trash2 } from "lucide-react";
import { cn } from "@/lib/utils";
import { markupModel, type Mark } from "@/readers";
import type { MarkupState } from "./useMarkup";

const { describe, DEFAULT_LAYER } = markupModel;

/** Board 4b: every mark by page, layers to show or hide, and where new marks go. */
export function MarkupPanel({ m, onShow }: { m: MarkupState; onShow: (mark: Mark) => void }) {
  const [newLayer, setNewLayer] = useState("");
  const layers = useMemo(() => {
    const set = new Set<string>([DEFAULT_LAYER, m.layer, ...m.marks.map((x) => x.layer)]);
    return [...set];
  }, [m.marks, m.layer]);
  const byPage = useMemo(() => {
    const map = new Map<number, Mark[]>();
    for (const x of [...m.marks].sort((a, b) => a.page - b.page)) {
      const list = map.get(x.page) ?? [];
      list.push(x);
      map.set(x.page, list);
    }
    return [...map.entries()];
  }, [m.marks]);
  const toggleLayer = (name: string) =>
    m.setVisibility(
      m.hideAll,
      m.hiddenLayers.includes(name)
        ? m.hiddenLayers.filter((l) => l !== name)
        : [...m.hiddenLayers, name],
    );

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex flex-col gap-1 border-b px-3 py-2">
        <label className="flex items-center gap-2 text-[12.5px]">
          <input
            type="checkbox"
            checked={m.hideAll}
            onChange={(e) => m.setVisibility(e.target.checked, m.hiddenLayers)}
          />
          Hide all markup
        </label>
        <p className="pt-1 text-[11px] font-semibold tracking-wide text-muted-foreground">LAYERS</p>
        {layers.map((name) => {
          const hidden = m.hiddenLayers.includes(name);
          return (
            <div key={name} className="flex items-center gap-1.5">
              <button
                type="button"
                aria-label={hidden ? `Show ${name}` : `Hide ${name}`}
                title={hidden ? "Show" : "Hide"}
                onClick={() => toggleLayer(name)}
                className="rounded p-1 text-muted-foreground hover:bg-muted [&_svg]:size-3.5"
              >
                {hidden ? <EyeOff /> : <Eye />}
              </button>
              <button
                type="button"
                onClick={() => m.setLayer(name)}
                className={cn(
                  "flex-1 truncate rounded px-1 py-0.5 text-left text-[12.5px] hover:bg-muted",
                  m.layer === name && "font-medium",
                )}
                title="Draw new marks on this layer"
              >
                {name}
                {m.layer === name && (
                  <span className="ml-1 text-[11px] text-muted-foreground">(drawing)</span>
                )}
              </button>
            </div>
          );
        })}
        <form
          className="flex items-center gap-1 pt-1"
          onSubmit={(e) => {
            e.preventDefault();
            const n = newLayer.trim().slice(0, 40);
            if (n) {
              m.setLayer(n);
              setNewLayer("");
            }
          }}
        >
          <input
            value={newLayer}
            onChange={(e) => setNewLayer(e.target.value)}
            placeholder="New layer"
            aria-label="New layer"
            className="h-7 min-w-0 flex-1 rounded-md border border-input bg-background px-2 text-[12.5px] outline-none focus-visible:border-ring"
          />
          <button
            type="submit"
            aria-label="Add layer"
            className="rounded p-1 text-muted-foreground hover:bg-muted [&_svg]:size-4"
          >
            <Plus />
          </button>
        </form>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-2 py-2">
        {byPage.length === 0 && (
          <p className="px-2 py-6 text-center text-[12.5px] text-muted-foreground">
            No markup yet. Choose a tool above and draw on the page.
          </p>
        )}
        {byPage.map(([page, list]) => (
          <div key={page} className="pb-2">
            <p className="px-2 pb-0.5 text-[11px] font-semibold tracking-wide text-muted-foreground">
              PAGE {page}
            </p>
            {list.map((x) => (
              <div
                key={x.id}
                className={cn(
                  "group flex items-center gap-1 rounded-md px-2 py-1 text-[12.5px] hover:bg-muted",
                  m.selected?.id === x.id && "bg-muted",
                  (m.hideAll || m.hiddenLayers.includes(x.layer)) && "opacity-50",
                )}
              >
                <button
                  type="button"
                  className="min-w-0 flex-1 truncate text-left"
                  onClick={() => onShow(x)}
                >
                  {describe(x)}
                </button>
                <button
                  type="button"
                  aria-label="Delete"
                  onClick={() => m.deleteMark(x.id)}
                  className="rounded p-0.5 text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-destructive focus-visible:opacity-100 [&_svg]:size-3.5"
                >
                  <Trash2 />
                </button>
              </div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
