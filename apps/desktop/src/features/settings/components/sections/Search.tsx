import { useEffect, useState } from "react";
import { Download, Loader2, RefreshCw, Trash2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  useDownloadOcrLanguage,
  useIndexStatus,
  useIndexing,
  useOcrLanguages,
  useRebuildIndex,
  useRemoveOcrLanguage,
  useSetOcrLanguages,
} from "@/features/search";
import { events } from "@/lib/ipc";
import { Group, Row } from "../parts";

function size(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  const mb = bytes / 1024 / 1024;
  return mb < 1024
    ? `${mb < 10 ? mb.toFixed(1) : Math.round(mb)} MB`
    : `${(mb / 1024).toFixed(1)} GB`;
}

/** Settings › Library & storage: the search index on this computer. */
export function SearchIndexGroup() {
  const { data } = useIndexStatus();
  const { running, done, total } = useIndexing();
  const rebuild = useRebuildIndex();
  const c = data?.counts;
  return (
    <Group
      title="Search inside books"
      scope="computer"
      description="Libreri reads the words of every book into a search index on this computer. It fills itself in the background and is made again when needed, so it is not part of backups."
    >
      <Row
        label="Search index"
        help={
          running && total
            ? `Reading books: ${done} of ${total}`
            : c
              ? `${c.withText + c.partial === 1 ? "1 book" : `${c.withText + c.partial} books`} searchable${c.noText ? ` · ${c.noText} scanned without text` : ""}${c.failed ? ` · ${c.failed} could not be read` : ""}`
              : undefined
        }
      >
        <span className="tabular-nums">{data ? size(data.size ?? 0) : "…"}</span>
        <Button
          variant="outline"
          size="sm"
          disabled={rebuild.isPending}
          onClick={() =>
            rebuild.mutate(undefined, {
              onSuccess: () => toast("Rebuilding the search index in the background"),
              onError: (e) => toast.error(e.message),
            })
          }
        >
          <RefreshCw /> Rebuild
        </Button>
      </Row>
    </Group>
  );
}

/** Settings › Helper programs: languages OCR can read. */
export function OcrLanguagesGroup() {
  const { data } = useOcrLanguages();
  const setDefaults = useSetOcrLanguages();
  const download = useDownloadOcrLanguage();
  const remove = useRemoveOcrLanguage();
  const [progress, setProgress] = useState<Record<string, number>>({});
  const [showAll, setShowAll] = useState(false);

  useEffect(() => {
    const off = events.ocrLanguageDownload.listen(({ payload: p }) => {
      if (p.total) setProgress((s) => ({ ...s, [p.code]: (p.done ?? 0) / p.total! }));
    });
    return () => void off.then((f) => f());
  }, []);

  if (!data) return null;
  const defaults = data.defaults;
  const ready = data.languages.filter((l) => l.builtIn || l.downloaded);
  const others = data.languages.filter((l) => !l.builtIn && !l.downloaded);
  const toggleDefault = (code: string, on: boolean) => {
    const next = on ? [...defaults, code] : defaults.filter((c) => c !== code);
    setDefaults.mutate(next.length ? next : ["eng"]);
  };

  return (
    <Group
      title="Languages for reading scanned pages"
      scope="computer"
      description="OCR needs data for each language on the page. Books that say their language use it; the ticked languages are used for the rest."
    >
      {!data.tesseract && (
        <Row label="Tesseract is not installed" help="Install it above to read scanned pages.">
          <span />
        </Row>
      )}
      {ready.map((l) => (
        <Row
          key={l.code}
          label={l.name}
          help={l.builtIn ? "Came with Tesseract" : "Downloaded by Libreri"}
        >
          <label className="flex items-center gap-1.5 text-[12.5px]">
            <input
              type="checkbox"
              checked={defaults.includes(l.code)}
              onChange={(e) => toggleDefault(l.code, e.target.checked)}
            />
            Use by default
          </label>
          {l.downloaded && !l.builtIn && (
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Remove ${l.name}`}
              onClick={() => remove.mutate(l.code, { onError: (e) => toast.error(e.message) })}
            >
              <Trash2 />
            </Button>
          )}
        </Row>
      ))}
      <div className="flex flex-col gap-2 px-4 py-3">
        <button
          type="button"
          onClick={() => setShowAll((v) => !v)}
          className="self-start text-[12.5px] font-medium text-muted-foreground hover:text-foreground"
        >
          {showAll ? "Hide other languages" : `Get another language (${others.length} available)…`}
        </button>
        {showAll && (
          <div className="grid grid-cols-2 gap-x-4 gap-y-1">
            {others.map((l) => (
              <div key={l.code} className="flex items-center gap-2">
                <span className="flex-1 text-[12.5px]">{l.name}</span>
                {download.isPending && download.variables === l.code ? (
                  <span className="flex items-center gap-1 text-[12px] text-muted-foreground">
                    <Loader2 className="size-3 animate-spin" aria-hidden />
                    {Math.round((progress[l.code] ?? 0) * 100)}%
                  </span>
                ) : (
                  <Button
                    variant="ghost"
                    size="sm"
                    disabled={download.isPending}
                    onClick={() =>
                      download.mutate(l.code, {
                        onSuccess: () => toast.success(`${l.name} can now be read`),
                        onError: (e) => toast.error(e.message),
                      })
                    }
                  >
                    <Download /> Get
                  </Button>
                )}
              </div>
            ))}
          </div>
        )}
      </div>
    </Group>
  );
}
