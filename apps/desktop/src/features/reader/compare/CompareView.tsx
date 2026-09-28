import { useEffect, useMemo, useRef, useState } from "react";
import { toast } from "sonner";
import { save as saveDialog } from "@tauri-apps/plugin-dialog";
import {
  AlertTriangle,
  ArrowLeftRight,
  ChevronDown,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  FileDown,
  Loader2,
  X,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import {
  bookUrl,
  commands,
  events,
  unwrap,
  type Change,
  type CompareStarted,
  type ComparisonDto,
} from "@/lib/ipc";
import type { CompareRequest } from "@/lib/tabs";
import { cn } from "@/lib/utils";
import {
  byPair,
  counts,
  KIND_COLOUR,
  KIND_LABEL,
  pairLabel,
  rectsOf,
  step,
  type Box,
} from "./model";

type View = "side" | "overlay" | "text";

const pageUrl = (id: string, side: "a" | "b", page: number, width: number) =>
  `${bookUrl(`.compare/${id}/${side}/${page}`)}?w=${width}`;

/**
 * Two documents compared (board 4e): side by side, one over the other, or
 * as a list of changes. The first document is on the left.
 */
export function CompareView({
  request,
  title,
  onSwap,
  onClose,
}: {
  request: CompareRequest;
  title: string;
  onSwap: () => void;
  onClose: () => void;
}) {
  const [started, setStarted] = useState<CompareStarted | null>(null);
  const [progress, setProgress] = useState<{ done: number; total: number; message: string }>({
    done: 0,
    total: 0,
    message: "Opening…",
  });
  const [result, setResult] = useState<ComparisonDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [view, setView] = useState<View>("side");
  const [onlyChanged, setOnlyChanged] = useState(true);
  const [current, setCurrent] = useState<number | null>(null);
  const [overlayPair, setOverlayPair] = useState<number | null>(null);
  const [exporting, setExporting] = useState(false);

  // Start comparing; the result comes as an event.
  useEffect(() => {
    let live = true;
    let id: string | null = null;
    let job: string | null = null;
    let finished = false;
    const fetchResult = async () => {
      if (!id) return;
      const r = await unwrap(commands.getComparison(id)).catch((e: Error) => {
        setError(e.message);
        return null;
      });
      if (!live || !r?.done) return;
      finished = true;
      if (r.error) setError(r.error);
      else setResult(r);
    };
    const offDone = events.compareFinished.listen(({ payload }) => {
      if (payload.id === id) void fetchResult();
    });
    const offJob = events.jobEventPayload.listen(({ payload: e }) => {
      if (e.id !== job || e.kind !== "progress") return;
      setProgress({ done: e.done ?? 0, total: e.total ?? 0, message: e.message ?? "" });
    });
    unwrap(commands.startCompare(request.a, request.b))
      .then((s) => {
        if (!live) {
          void commands.closeCompare(s.id);
          return;
        }
        id = s.id;
        job = s.jobId;
        setStarted(s);
        // It may have finished before the listener knew its id.
        void fetchResult();
      })
      .catch((e: Error) => live && setError(e.message));
    return () => {
      live = false;
      void offDone.then((f) => f());
      void offJob.then((f) => f());
      if (job && !finished) void commands.cancelJob(job);
      if (id) void commands.closeCompare(id);
    };
  }, [request]);

  const changes = useMemo(() => result?.changes ?? [], [result]);
  const pairs = useMemo(() => result?.pairs ?? [], [result]);
  const perPair = useMemo(() => byPair(changes), [changes]);
  const shownPairs = useMemo(
    () =>
      pairs
        .map((p, i) => ({ p, i }))
        .filter(({ i }) => !onlyChanged || !changes.length || perPair.has(i)),
    [pairs, onlyChanged, changes.length, perPair],
  );
  const c = counts(changes);
  const currentChange = current !== null ? changes[current] : undefined;

  // Go to a change: its row in the side-by-side view, its pair in the overlay.
  const scroller = useRef<HTMLDivElement>(null);
  const goTo = (i: number | null) => {
    setCurrent(i);
    if (i === null) return;
    const ch = changes[i]!;
    setOverlayPair(ch.pair);
    if (view === "text") setView("side");
    requestAnimationFrame(() => {
      const row = scroller.current?.querySelector<HTMLElement>(`[data-pair="${ch.pair}"]`);
      const mark = row?.querySelector<HTMLElement>(`[data-change="${i}"]`);
      (mark ?? row)?.scrollIntoView({ block: "center", behavior: "smooth" });
    });
  };

  const exportReport = async () => {
    if (!started) return;
    const clean = title.replace(/[\\/:*?"<>|]+/g, " ").trim();
    const dest = await saveDialog({
      defaultPath: `${clean} (comparison).pdf`,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!dest) return;
    setExporting(true);
    try {
      await unwrap(commands.exportCompareReport(started.id, dest));
      toast.success("Comparison report saved", {
        description: dest,
        action: { label: "Show", onClick: () => void commands.revealPath(dest) },
      });
    } catch (e) {
      toast.error("The report could not be saved", {
        description: e instanceof Error ? e.message : String(e),
      });
    } finally {
      setExporting(false);
    }
  };

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "n" || e.key === "j") goTo(step(changes.length, current, 1));
    else if (e.key === "p" || e.key === "k") goTo(step(changes.length, current, -1));
  };

  const overlayIndex =
    overlayPair ?? currentChange?.pair ?? shownPairs.find(({ p }) => p.a && p.b)?.i ?? 0;

  return (
    <div className="flex min-h-0 flex-1 flex-col" onKeyDown={onKey} tabIndex={-1}>
      {/* Header */}
      <div className="flex min-h-11 shrink-0 flex-wrap items-center gap-2 border-b px-3 py-1.5">
        <div className="flex min-w-0 flex-1 items-center gap-2 text-[12.5px]">
          <span className="truncate" title={started?.aLabel}>
            <span className="text-muted-foreground">First:</span> {started?.aLabel ?? "…"}
          </span>
          <Button
            variant="ghost"
            size="icon"
            aria-label="Swap the two sides"
            title="Swap the two sides"
            onClick={onSwap}
          >
            <ArrowLeftRight />
          </Button>
          <span className="truncate" title={started?.bLabel}>
            <span className="text-muted-foreground">Second:</span> {started?.bLabel ?? "…"}
          </span>
        </div>
        <div className="flex rounded-md border p-0.5" role="radiogroup" aria-label="View">
          {(
            [
              ["side", "Side by side"],
              ["overlay", "Overlay"],
              ["text", "Changes"],
            ] as const
          ).map(([id, label]) => (
            <button
              key={id}
              type="button"
              role="radio"
              aria-checked={view === id}
              onClick={() => setView(id)}
              className={cn(
                "h-6 rounded px-2.5 text-[12.5px]",
                view === id
                  ? "bg-muted font-medium text-foreground"
                  : "text-muted-foreground hover:text-foreground",
              )}
            >
              {label}
            </button>
          ))}
        </div>
        {result && changes.length > 0 && (
          <div className="flex items-center gap-0.5">
            <Button
              variant="ghost"
              size="icon"
              aria-label="Previous change"
              title="Previous change (P)"
              onClick={() => goTo(step(changes.length, current, -1))}
            >
              <ChevronUp />
            </Button>
            <span className="min-w-16 text-center text-[12px] text-muted-foreground tabular-nums">
              {current !== null
                ? `${current + 1} of ${changes.length}`
                : `${changes.length} changes`}
            </span>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Next change"
              title="Next change (N)"
              onClick={() => goTo(step(changes.length, current, 1))}
            >
              <ChevronDown />
            </Button>
          </div>
        )}
        <Button
          variant="outline"
          size="sm"
          disabled={!result || exporting}
          onClick={() => void exportReport()}
        >
          {exporting ? <Loader2 className="animate-spin" /> : <FileDown />} Report…
        </Button>
        <Button variant="ghost" size="icon" aria-label="Close the comparison" onClick={onClose}>
          <X />
        </Button>
      </div>

      {result && (
        <div className="flex h-8 shrink-0 items-center gap-3 border-b bg-muted/30 px-3 text-[12px]">
          {changes.length === 0 ? (
            <span>No differences: the same text, and the pages look the same.</span>
          ) : (
            <>
              {(
                [
                  ["removed", c.removed],
                  ["added", c.added],
                  ["changed", c.changed],
                  ["look", c.look],
                ] as const
              ).map(([k, n]) =>
                n ? (
                  <span key={k} className="flex items-center gap-1.5">
                    <span className="size-2.5 rounded-sm" style={{ background: KIND_COLOUR[k] }} />
                    {n} {KIND_LABEL[k].toLowerCase()}
                  </span>
                ) : null,
              )}
              {c.pages > 0 && <span>{c.pages} pages added or removed</span>}
              <span className="flex-1" />
              {view === "side" && (
                <label className="flex items-center gap-1.5">
                  <input
                    type="checkbox"
                    checked={onlyChanged}
                    onChange={(e) => setOnlyChanged(e.target.checked)}
                  />
                  Only pages with changes
                </label>
              )}
            </>
          )}
        </div>
      )}

      {/* Body */}
      {error ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 p-6 text-center">
          <AlertTriangle className="size-6 text-destructive" aria-hidden />
          <p>These two could not be compared.</p>
          <p className="max-w-md text-[12.5px] text-muted-foreground">{error}</p>
        </div>
      ) : !result ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-3 text-muted-foreground">
          <Loader2 className="size-5 animate-spin" />
          <p>{progress.message || "Comparing…"}</p>
          {progress.total > 0 && (
            <div className="h-1.5 w-64 overflow-hidden rounded-full bg-muted">
              <div
                className="h-full bg-primary/70 transition-[width]"
                style={{ width: `${Math.round((progress.done / progress.total) * 100)}%` }}
              />
            </div>
          )}
        </div>
      ) : view === "text" ? (
        <ChangeList changes={changes} pairs={result.pairs} current={current} onPick={goTo} />
      ) : view === "overlay" ? (
        <Overlay
          id={started!.id}
          result={result}
          index={overlayIndex}
          changes={perPair.get(overlayIndex) ?? []}
          onIndex={setOverlayPair}
          changedPairs={[...perPair.keys()].sort((a, b) => a - b)}
        />
      ) : (
        <div ref={scroller} className="min-h-0 flex-1 overflow-auto bg-muted/40 px-4 py-4">
          <div className="mx-auto flex max-w-[1500px] flex-col gap-6">
            {shownPairs.map(({ p, i }) => (
              <div key={i} data-pair={i} className="flex flex-col gap-1.5">
                <div className="flex items-center gap-2 text-[12px] text-muted-foreground">
                  <span className="font-medium text-foreground">{pairLabel(p)}</span>
                  {(perPair.get(i) ?? []).length > 0 && (
                    <span>
                      · {(perPair.get(i) ?? []).length}{" "}
                      {(perPair.get(i) ?? []).length === 1 ? "change" : "changes"}
                    </span>
                  )}
                </div>
                <div className="grid grid-cols-2 gap-4">
                  {(["a", "b"] as const).map((side) => {
                    const page = side === "a" ? p.a : p.b;
                    return page ? (
                      <PageWithMarks
                        key={side}
                        src={pageUrl(started!.id, side, page, 900)}
                        label={`${side === "a" ? "First" : "Second"}, page ${page}`}
                        changes={perPair.get(i) ?? []}
                        all={changes}
                        side={side}
                        current={currentChange}
                      />
                    ) : (
                      <div
                        key={side}
                        className="flex aspect-[1/1.3] items-center justify-center rounded border border-dashed text-[12.5px] text-muted-foreground"
                      >
                        {side === "a" ? "Not in the first" : "Not in the second"}
                      </div>
                    );
                  })}
                </div>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

/** A page picture with the changes drawn over it. */
function PageWithMarks({
  src,
  label,
  changes,
  all,
  side,
  current,
}: {
  src: string;
  label: string;
  changes: Change[];
  all: Change[];
  side: "a" | "b";
  current: Change | undefined;
}) {
  const [loaded, setLoaded] = useState(false);
  const [failed, setFailed] = useState(false);
  return (
    <div
      className={cn(
        "relative overflow-hidden rounded-sm bg-white shadow-sm ring-1 ring-border",
        !loaded && "aspect-[1/1.3]",
      )}
    >
      <img
        src={src}
        alt={label}
        loading="lazy"
        className="block w-full"
        onLoad={() => setLoaded(true)}
        onError={() => setFailed(true)}
      />
      {failed && (
        <div className="absolute inset-0 flex items-center justify-center p-4 text-center text-[12px] text-muted-foreground">
          This page could not be drawn.
        </div>
      )}
      {loaded &&
        changes.map((ch) =>
          rectsOf(ch, side).map(([x, y, w, h], k) => (
            <span
              key={`${all.indexOf(ch)}-${k}`}
              data-change={all.indexOf(ch)}
              title={`${KIND_LABEL[ch.kind]}${ch.aText || ch.bText ? `: ${side === "a" ? ch.aText : ch.bText}` : ""}`}
              className={cn(
                "absolute rounded-[2px]",
                ch === current && "outline outline-2 outline-offset-2 outline-foreground",
              )}
              style={{
                left: `${x * 100}%`,
                top: `${y * 100}%`,
                width: `${w * 100}%`,
                height: `${h * 100}%`,
                background: `${KIND_COLOUR[ch.kind]}33`,
                border: `1.5px solid ${KIND_COLOUR[ch.kind]}`,
              }}
            />
          )),
        )}
    </div>
  );
}

/** One page pair, the second on top of the first. */
function Overlay({
  id,
  result,
  index,
  changes,
  changedPairs,
  onIndex,
}: {
  id: string;
  result: ComparisonDto;
  index: number;
  changes: Change[];
  changedPairs: number[];
  onIndex: (i: number) => void;
}) {
  const [amount, setAmount] = useState(50);
  const [difference, setDifference] = useState(false);
  const pair = result.pairs[index];
  const list = changedPairs.length ? changedPairs : result.pairs.map((_, i) => i);
  const at = list.indexOf(index);
  const go = (dir: 1 | -1) => {
    const next = list[at < 0 ? 0 : (at + dir + list.length) % list.length];
    if (next !== undefined) onIndex(next);
  };
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-10 shrink-0 items-center gap-3 border-b px-3 text-[12.5px]">
        <Button variant="ghost" size="icon" aria-label="Previous page" onClick={() => go(-1)}>
          <ChevronLeft />
        </Button>
        <span className="min-w-24 text-center font-medium">{pairLabel(pair)}</span>
        <Button variant="ghost" size="icon" aria-label="Next page" onClick={() => go(1)}>
          <ChevronRight />
        </Button>
        <span className="flex-1" />
        <label className="flex items-center gap-2">
          <span className="text-muted-foreground">First</span>
          <input
            type="range"
            min={0}
            max={100}
            value={amount}
            disabled={difference}
            onChange={(e) => setAmount(Number(e.target.value))}
            aria-label="How much of the second page shows"
          />
          <span className="text-muted-foreground">Second</span>
        </label>
        <label className="flex items-center gap-1.5">
          <input
            type="checkbox"
            checked={difference}
            onChange={(e) => setDifference(e.target.checked)}
          />
          Show only differences
        </label>
      </div>
      <div className="min-h-0 flex-1 overflow-auto bg-muted/40 p-4">
        {pair?.a && pair?.b ? (
          <div
            className="relative mx-auto max-w-[900px] bg-white shadow-sm ring-1 ring-border"
            // What is the same cancels out to white; what differs stays dark.
            style={difference ? { filter: "invert(1)", isolation: "isolate" } : undefined}
          >
            <img
              src={pageUrl(id, "a", pair.a, 1400)}
              alt={`First, page ${pair.a}`}
              className="block w-full"
            />
            <img
              src={pageUrl(id, "b", pair.b, 1400)}
              alt={`Second, page ${pair.b}`}
              className="absolute inset-0 size-full"
              style={
                difference ? { mixBlendMode: "difference", opacity: 1 } : { opacity: amount / 100 }
              }
            />
            {!difference &&
              changes.map((ch, i) =>
                (amount >= 50 ? ch.bRects : ch.aRects).map((r, k) => {
                  const [x, y, w, h] = r as Box;
                  return (
                    <span
                      key={`${i}-${k}`}
                      className="absolute rounded-[2px]"
                      style={{
                        left: `${x * 100}%`,
                        top: `${y * 100}%`,
                        width: `${w * 100}%`,
                        height: `${h * 100}%`,
                        border: `1.5px solid ${KIND_COLOUR[ch.kind]}`,
                      }}
                    />
                  );
                }),
              )}
          </div>
        ) : (
          <p className="py-16 text-center text-muted-foreground">
            {pair?.a
              ? "This page was removed: there is nothing to lay it over."
              : "This page was added: there is nothing to lay it over."}
          </p>
        )}
      </div>
    </div>
  );
}

/** Every change as text, like tracked changes. */
function ChangeList({
  changes,
  pairs,
  current,
  onPick,
}: {
  changes: Change[];
  pairs: ComparisonDto["pairs"];
  current: number | null;
  onPick: (i: number) => void;
}) {
  if (!changes.length)
    return (
      <p className="py-16 text-center text-muted-foreground">
        No differences: the same text, and the pages look the same.
      </p>
    );
  return (
    <div className="min-h-0 flex-1 overflow-auto">
      <ul className="mx-auto flex max-w-3xl flex-col divide-y py-2">
        {changes.map((c, i) => (
          <li key={i}>
            <button
              type="button"
              onClick={() => onPick(i)}
              className={cn(
                "flex w-full items-start gap-3 px-4 py-2.5 text-left hover:bg-muted",
                current === i && "bg-muted",
              )}
            >
              <span className="w-20 shrink-0 text-[12px] text-muted-foreground tabular-nums">
                {pairLabel(pairs[c.pair])}
              </span>
              <span
                className="w-28 shrink-0 text-[12px] font-medium"
                style={{ color: KIND_COLOUR[c.kind] }}
              >
                {KIND_LABEL[c.kind]}
              </span>
              <span className="min-w-0 flex-1 text-[13px] leading-relaxed">
                {c.kind === "look" ? (
                  <span className="text-muted-foreground">
                    Drawings or pictures on this page look different.
                  </span>
                ) : (
                  <>
                    {c.aText && (
                      <del className="rounded-sm bg-red-500/15 px-0.5 text-red-700 decoration-red-500/70 dark:text-red-300">
                        {c.aText}
                      </del>
                    )}
                    {c.aText && c.bText && " "}
                    {c.bText && (
                      <ins className="rounded-sm bg-green-500/15 px-0.5 text-green-800 no-underline dark:text-green-300">
                        {c.bText}
                      </ins>
                    )}
                  </>
                )}
              </span>
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
