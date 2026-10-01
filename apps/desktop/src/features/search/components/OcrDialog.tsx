import { useEffect, useMemo, useState } from "react";
import { Download, Loader2, ScanText } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useHelperDialog } from "@/features/helpers";
import { events } from "@/lib/ipc";
import {
  useDownloadOcrLanguage,
  useMakeSearchable,
  useOcrEngines,
  useOcrLanguages,
  useTextStatus,
} from "../api";
import { useOcrDialog } from "../store";

/**
 * "Make searchable": reads the scanned pages of PDF and DjVu books with
 * Tesseract (OCR) and saves the text in the library. Pages that already
 * have text are left as they are.
 */
export function OcrDialog() {
  const ids = useOcrDialog((s) => s.ids);
  const close = useOcrDialog((s) => s.close);
  return (
    <Dialog
      open={ids !== null}
      onOpenChange={(o) => !o && close()}
      title={ids && ids.length > 1 ? `Make ${ids.length} books searchable` : "Make searchable"}
      description="Libreri reads the scanned pages with OCR, so their words can be found, selected and highlighted. The text is saved in the library."
    >
      {ids && <Body ids={ids} onDone={close} />}
    </Dialog>
  );
}

function Body({ ids, onDone }: { ids: string[]; onDone: () => void }) {
  const { data: langs, refetch } = useOcrLanguages();
  const { data: engines } = useOcrEngines();
  // A downloaded model chosen in Settings reads instead of Tesseract.
  const model = engines?.models.find((m) => m.id === engines.selected && m.downloaded);
  const { data: first } = useTextStatus(ids[0] ?? null);
  const make = useMakeSearchable();
  const download = useDownloadOcrLanguage();
  const openHelper = useHelperDialog((s) => s.open);
  const [chosen, setChosen] = useState<string[] | null>(null);
  const [redo, setRedo] = useState(false);
  const [progress, setProgress] = useState<Record<string, number>>({});

  useEffect(() => {
    const off = events.ocrLanguageDownload.listen(({ payload: p }) => {
      if (p.total) setProgress((s) => ({ ...s, [p.code]: (p.done ?? 0) / p.total! }));
    });
    return () => void off.then((f) => f());
  }, []);

  const available = useMemo(
    () =>
      new Set((langs?.languages ?? []).filter((l) => l.builtIn || l.downloaded).map((l) => l.code)),
    [langs],
  );
  // Start from the book's own language (or the usual ones).
  const selected = chosen ?? first?.suggestedLanguages ?? [];
  const missing = selected.filter((c) => !available.has(c));
  const name = (code: string) => langs?.languages.find((l) => l.code === code)?.name ?? code;
  const offered = (langs?.languages ?? []).filter(
    (l) => l.builtIn || l.downloaded || selected.includes(l.code),
  );

  if (langs && !langs.tesseract && !model)
    return (
      <div className="flex flex-col gap-3 text-[13px]">
        <p>
          Reading scanned pages needs <strong>Tesseract</strong>, a free OCR program. Libreri can
          install it for you.
        </p>
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onDone}>
            Close
          </Button>
          <Button onClick={() => openHelper("tesseract", () => void refetch())}>
            Install Tesseract…
          </Button>
        </div>
      </div>
    );

  const start = () =>
    make.mutate(
      { ids, languages: selected, redo },
      {
        onSuccess: () => {
          toast("Reading pages in the background", {
            description: "You can keep reading while it works.",
          });
          onDone();
        },
        onError: (e) => toast.error(e.message),
      },
    );

  const already = first?.ocrPages ?? 0;
  return (
    <div className="flex flex-col gap-4 text-[13px]">
      {ids.length === 1 && first && (
        <p className="text-muted-foreground">
          {first.state === "text" && !first.emptyPages
            ? "Every page of this book already has text."
            : first.pages
              ? `${first.emptyPages} of ${first.pages} pages have no text yet.`
              : "Pages without text will be read."}
        </p>
      )}

      {model && (
        <p className="rounded-md border bg-muted/40 p-3">
          Pages are read with <strong>{model.name}</strong>, on this computer, one page at a time
          (seconds per page). Change it in Settings › Helper programs.
        </p>
      )}

      {!model && (
        <fieldset className="flex flex-col gap-2">
          <legend className="mb-1 font-medium">Language of the pages</legend>
          <div className="flex max-h-48 flex-col gap-1 overflow-auto rounded-md border p-2">
            {offered.map((l) => (
              <label key={l.code} className="flex items-center gap-2">
                <input
                  type="checkbox"
                  checked={selected.includes(l.code)}
                  onChange={(e) =>
                    setChosen(
                      e.target.checked
                        ? [...selected, l.code]
                        : selected.filter((c) => c !== l.code),
                    )
                  }
                />
                <span className="flex-1">{l.name}</span>
                {!available.has(l.code) && (
                  <span className="text-[11.5px] text-amber-700 dark:text-amber-400">
                    needs a download
                  </span>
                )}
              </label>
            ))}
            <label className="mt-1 flex items-center gap-2 text-muted-foreground">
              <span>More:</span>
              <select
                className="h-7 flex-1 rounded border bg-background px-1"
                value=""
                aria-label="Add a language"
                onChange={(e) => e.target.value && setChosen([...selected, e.target.value])}
              >
                <option value="">Add a language…</option>
                {(langs?.languages ?? [])
                  .filter((l) => !offered.includes(l))
                  .map((l) => (
                    <option key={l.code} value={l.code}>
                      {l.name}
                    </option>
                  ))}
              </select>
            </label>
          </div>
          <p className="text-[12px] text-muted-foreground">
            Pick every language on the pages; fewer languages read faster.
          </p>
        </fieldset>
      )}

      {!model && missing.length > 0 && (
        <div className="flex flex-col gap-2 rounded-md border border-amber-300/60 bg-amber-50 p-3 dark:border-amber-500/30 dark:bg-amber-950/30">
          <p>
            {missing.map(name).join(", ")} {missing.length > 1 ? "need" : "needs"} a download (a few
            MB each, from the Tesseract project).
          </p>
          {missing.map((code) => (
            <div key={code} className="flex items-center gap-2">
              <span className="flex-1">{name(code)}</span>
              {download.isPending && download.variables === code ? (
                <span className="flex items-center gap-1 text-muted-foreground">
                  <Loader2 className="size-3.5 animate-spin" aria-hidden />
                  {Math.round((progress[code] ?? 0) * 100)}%
                </span>
              ) : (
                <Button
                  size="sm"
                  variant="outline"
                  disabled={download.isPending}
                  onClick={() => download.mutate(code, { onError: (e) => toast.error(e.message) })}
                >
                  <Download /> Download
                </Button>
              )}
            </div>
          ))}
        </div>
      )}

      {already > 0 && (
        <label className="flex items-center gap-2">
          <input type="checkbox" checked={redo} onChange={(e) => setRedo(e.target.checked)} />
          Read the {already} pages read before again (for example in another language)
        </label>
      )}

      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button
          onClick={start}
          disabled={make.isPending || (!model && (!selected.length || missing.length > 0))}
        >
          <ScanText /> Make searchable
        </Button>
      </div>
    </div>
  );
}
