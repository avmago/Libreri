import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { ExternalLink, Loader2, NotebookPen } from "lucide-react";
import { Button } from "@/components/ui/button";
import { commands, unwrap } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { Floating } from "./Popovers";

/**
 * Look up: the selected word's meanings (Wiktionary) and, for
 * names and things, a short Wikipedia summary, in the book's language.
 */
export function LookupPopover({
  query,
  lang,
  rect,
  onNotebook,
  onClose,
}: {
  query: string;
  lang?: string | null;
  rect: DOMRect;
  onNotebook: (markdown: string) => void;
  onClose: () => void;
}) {
  const q = useQuery({
    queryKey: ["look-up", query, lang ?? null],
    queryFn: () => unwrap(commands.lookUp(query, lang ?? null)),
    staleTime: Infinity,
    retry: false,
  });
  const [picked, setTab] = useState<"dictionary" | "wikipedia" | null>(null);
  const d = q.data;
  const hasDict = !!d?.meanings.length;
  const hasWiki = !!d?.wikipedia;
  const tab = picked ?? (hasDict || !hasWiki ? "dictionary" : "wikipedia");
  const open = (url: string) => void commands.openExternalUrl(url);

  const toNotebook = () => {
    if (!d) return;
    if (tab === "wikipedia" && d.wikipedia) {
      onNotebook(
        `**${d.wikipedia.title}** — ${d.wikipedia.extract}\n\n([Wikipedia](${d.wikipedia.url}))`,
      );
      return;
    }
    const lines = [`**${d.query}**`];
    for (const m of d.meanings.slice(0, 3)) {
      lines.push(`*${m.partOfSpeech.toLowerCase()}*`);
      m.definitions.slice(0, 3).forEach((x, i) => lines.push(`${i + 1}. ${x.text}`));
    }
    lines.push(`([Wiktionary](${d.wiktionaryUrl}))`);
    onNotebook(lines.join("\n"));
  };

  return (
    <Floating rect={rect} onClose={onClose}>
      <div className="flex w-[22rem] max-w-[90vw] flex-col gap-2 p-1 text-[13px]">
        <div className="flex items-center gap-2">
          <span className="min-w-0 flex-1 truncate text-[15px] font-semibold">{query}</span>
          {(hasDict || hasWiki) && (
            <div role="tablist" aria-label="Source" className="flex rounded-md bg-muted p-0.5">
              {(
                [
                  ["dictionary", "Dictionary", hasDict],
                  ["wikipedia", "Wikipedia", hasWiki],
                ] as const
              ).map(([k, name, has]) => (
                <button
                  key={k}
                  type="button"
                  role="tab"
                  aria-selected={tab === k}
                  disabled={!has}
                  onClick={() => setTab(k)}
                  className={cn(
                    "rounded px-2 py-0.5 text-[11.5px] font-medium disabled:opacity-40",
                    tab === k ? "bg-background shadow-sm" : "text-muted-foreground",
                  )}
                >
                  {name}
                </button>
              ))}
            </div>
          )}
        </div>

        <div className="max-h-72 overflow-auto">
          {q.isLoading ? (
            <p className="flex items-center gap-2 py-3 text-muted-foreground">
              <Loader2 className="size-4 animate-spin" /> Looking it up…
            </p>
          ) : q.isError ? (
            <p className="py-2 text-muted-foreground">
              Could not look it up: {String((q.error as Error).message ?? q.error)}
            </p>
          ) : !hasDict && !hasWiki ? (
            <p className="py-2 text-muted-foreground">
              Nothing found
              {d?.problems.length ? ` (${d.problems.join("; ")})` : ""}. Try a single word, or the
              word without its ending.
            </p>
          ) : tab === "dictionary" ? (
            <div className="flex flex-col gap-2.5">
              {d!.meanings.map((m, i) => (
                <div key={i} className="flex flex-col gap-1">
                  <span className="text-[11.5px] font-medium tracking-wide text-muted-foreground uppercase">
                    {m.partOfSpeech}
                  </span>
                  <ol className="flex list-decimal flex-col gap-1 pl-5">
                    {m.definitions.map((x, j) => (
                      <li key={j} className="leading-snug">
                        {x.text}
                        {x.examples.map((e, k) => (
                          <span key={k} className="block text-[12px] text-muted-foreground italic">
                            {e}
                          </span>
                        ))}
                      </li>
                    ))}
                  </ol>
                </div>
              ))}
            </div>
          ) : (
            d!.wikipedia && (
              <div className="flex flex-col gap-2">
                <div className="flex gap-3">
                  {d!.wikipedia.image && (
                    <img
                      src={d!.wikipedia.image}
                      alt=""
                      className="h-20 w-20 shrink-0 rounded-md object-cover"
                    />
                  )}
                  <div className="min-w-0">
                    <p className="font-semibold">{d!.wikipedia.title}</p>
                    {d!.wikipedia.description && (
                      <p className="text-[12px] text-muted-foreground">
                        {d!.wikipedia.description}
                      </p>
                    )}
                  </div>
                </div>
                <p className="leading-relaxed">{d!.wikipedia.extract}</p>
                {d!.wikipedia.disambiguation && (
                  <p className="text-[12px] text-muted-foreground">
                    This name has several meanings; open Wikipedia to choose.
                  </p>
                )}
              </div>
            )
          )}
        </div>

        {(hasDict || hasWiki) && (
          <div className="flex items-center justify-between border-t pt-1.5">
            <button
              type="button"
              className="flex items-center gap-1 text-[11.5px] text-muted-foreground hover:text-foreground"
              onClick={() =>
                open(tab === "wikipedia" && d?.wikipedia ? d.wikipedia.url : d!.wiktionaryUrl)
              }
            >
              <ExternalLink className="size-3" />
              {tab === "wikipedia" ? "Wikipedia" : "Wiktionary"}
            </button>
            <Button size="sm" variant="ghost" onClick={toNotebook}>
              <NotebookPen /> Add to notebook
            </Button>
          </div>
        )}
      </div>
    </Floating>
  );
}
