import { useEffect, useState } from "react";
import { ExternalLink, Loader2, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  useDownloadOcrModel,
  useOcrEngines,
  useRemoveOcrModel,
  useSetOcrEngine,
} from "@/features/search";
import { commands, events, type OcrModelInfo } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { Group } from "../parts";

function gb(mb: number): string {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`;
}

/**
 * Settings › Helper programs: what reads scanned pages. Tesseract, or a
 * model downloaded here and run on this computer (ADR 0029). Each model
 * has its own Delete button.
 */
export function OcrEnginesGroup() {
  const { data } = useOcrEngines();
  const choose = useSetOcrEngine();
  const download = useDownloadOcrModel();
  const remove = useRemoveOcrModel();
  const [progress, setProgress] = useState<{ id: string; done: number; total: number } | null>(
    null,
  );

  useEffect(() => {
    const off = events.ocrModelDownload.listen(({ payload: p }) => {
      if (p.finished) setProgress(null);
      else setProgress({ id: p.id, done: p.done ?? 0, total: p.total ?? 0 });
    });
    return () => void off.then((f) => f());
  }, []);

  if (!data) return null;
  const pick = (id: string) =>
    choose.mutate(id, {
      onError: (e) => toast.error("Could not choose it", { description: e.message }),
    });

  return (
    <Group
      title="Reading scanned pages"
      scope="computer"
      description="Choose what reads the pages of scanned books when you make them searchable. Tesseract is quick and small. A model reads more like a person: headings, tables and formulas in reading order. It is a large download and takes seconds per page; it runs on this computer and nothing is sent anywhere."
    >
      <div role="radiogroup" aria-label="OCR engine" className="flex flex-col divide-y">
        <EngineRow
          name="Tesseract"
          description={
            data.tesseract
              ? "The helper program above. Fast, in many languages (see below)."
              : "Install it above to use it."
          }
          selected={data.selected === "tesseract"}
          selectable
          onSelect={() => pick("tesseract")}
        />
        {data.models.map((m) => (
          <ModelRow
            key={m.id}
            model={m}
            selected={data.selected === m.id}
            downloading={data.downloading === m.id}
            otherDownloading={data.downloading !== null && data.downloading !== m.id}
            progress={progress?.id === m.id ? progress : null}
            onSelect={() => pick(m.id)}
            onDownload={() =>
              download.mutate(m.id, {
                onSuccess: () =>
                  toast.success(`${m.name} is ready`, {
                    description: "Choose it here to read scanned pages with it.",
                  }),
                onError: (e) =>
                  e.message !== "cancelled" &&
                  toast.error(`${m.name} could not be downloaded`, { description: e.message }),
              })
            }
            onDelete={() =>
              remove.mutate(m.id, {
                onSuccess: () => toast(`${m.name} was removed`),
                onError: (e) => toast.error("Could not remove it", { description: e.message }),
              })
            }
            removing={remove.isPending && remove.variables === m.id}
          />
        ))}
      </div>
    </Group>
  );
}

function EngineRow({
  name,
  description,
  selected,
  selectable,
  onSelect,
  children,
  note,
}: {
  name: string;
  description: string;
  selected: boolean;
  selectable: boolean;
  onSelect: () => void;
  children?: React.ReactNode;
  note?: React.ReactNode;
}) {
  return (
    <div className="flex items-start gap-3 px-4 py-3">
      <input
        type="radio"
        name="ocr-engine"
        aria-label={`Use ${name}`}
        className="mt-1"
        checked={selected}
        disabled={!selectable}
        onChange={onSelect}
      />
      <div className={cn("flex min-w-0 flex-1 flex-col gap-0.5", !selectable && "opacity-70")}>
        <span className="font-medium">{name}</span>
        <p className="text-[12.5px] leading-snug text-muted-foreground">{description}</p>
        {note}
      </div>
      {children && <div className="flex shrink-0 items-center gap-2">{children}</div>}
    </div>
  );
}

function ModelRow({
  model: m,
  selected,
  downloading,
  otherDownloading,
  progress,
  onSelect,
  onDownload,
  onDelete,
  removing,
}: {
  model: OcrModelInfo;
  selected: boolean;
  downloading: boolean;
  otherDownloading: boolean;
  progress: { done: number; total: number } | null;
  onSelect: () => void;
  onDownload: () => void;
  onDelete: () => void;
  removing: boolean;
}) {
  const pct =
    progress && progress.total ? Math.round((progress.done / progress.total) * 100) : null;
  return (
    <EngineRow
      name={m.name}
      description={m.description}
      selected={selected}
      selectable={m.downloaded}
      onSelect={onSelect}
      note={
        <p className="flex flex-wrap items-center gap-x-2 text-[12px] text-muted-foreground">
          <span>
            {gb(m.sizeMb)} · {m.licence}
          </span>
          <button
            type="button"
            className="inline-flex items-center gap-1 underline-offset-2 hover:underline"
            onClick={() => void commands.openExternalUrl(m.homepage)}
          >
            About the model <ExternalLink className="size-3" />
          </button>
          {m.note && <span className="w-full">{m.note}</span>}
          {downloading && (
            <span className="flex w-full items-center gap-2 pt-1">
              <span className="h-1.5 max-w-64 flex-1 overflow-hidden rounded-full bg-muted">
                <span
                  className="block h-full bg-foreground/70 transition-[width]"
                  style={{ width: `${pct ?? 0}%` }}
                />
              </span>
              <span className="tabular-nums">{pct !== null ? `${pct}%` : "Starting…"}</span>
            </span>
          )}
        </p>
      }
    >
      {!m.available ? (
        <span className="rounded-md border px-2 py-1 text-[12px] text-muted-foreground">
          Coming later
        </span>
      ) : downloading ? (
        <Button variant="outline" size="sm" onClick={() => void commands.cancelOcrModelDownload()}>
          <X /> Stop
        </Button>
      ) : m.downloaded ? (
        <Button
          variant="outline"
          size="sm"
          aria-label={`Delete ${m.name}`}
          title={`Delete ${m.name} from this computer (${gb(m.sizeMb)})`}
          disabled={removing}
          onClick={onDelete}
        >
          {removing ? <Loader2 className="animate-spin" /> : <Trash2 />} Delete
        </Button>
      ) : (
        <Button size="sm" disabled={otherDownloading} onClick={onDownload}>
          Download
        </Button>
      )}
    </EngineRow>
  );
}
