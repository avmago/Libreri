import { useMemo } from "react";
import { BookOpen, Layers } from "lucide-react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useBooks } from "../api";
import { useBookActions } from "../hooks/useBookActions";
import { authorsText } from "../model";
import { allSeries, type Series } from "../series";
import { useLibraryView } from "../store";
import { BookCover } from "./BookCover";

const ALL = { sort: "title" as const, descending: false };

/** Every series in the library, with how far you are in each. */
export function SeriesView() {
  const { data, isPending } = useBooks(ALL);
  const series = useMemo(() => allSeries(data ?? []), [data]);
  return (
    <div className="flex h-full min-h-0 flex-col overflow-auto p-6">
      <div className="mb-4 flex items-baseline gap-3">
        <h1 className="text-xl font-semibold tracking-tight">Series</h1>
        {!isPending && (
          <span className="text-muted-foreground">
            {series.length} {series.length === 1 ? "series" : "series"}
          </span>
        )}
      </div>
      {!isPending && !series.length ? (
        <div className="flex flex-1 flex-col items-center justify-center gap-2 text-center text-muted-foreground">
          <Layers className="size-8" aria-hidden />
          <p className="font-medium text-foreground">No series yet</p>
          <p className="max-w-sm text-[13px]">
            Books show here when their details name a series. Select several books and use Edit
            together to put them in a series and number them.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-[repeat(auto-fill,minmax(250px,1fr))] gap-4">
          {series.map((s) => (
            <SeriesCard key={s.name} series={s} />
          ))}
        </div>
      )}
    </div>
  );
}

function SeriesCard({ series: s }: { series: Series }) {
  const setNav = useLibraryView((v) => v.setNav);
  const { open } = useBookActions();
  const total = s.books.length;
  const pct = Math.round((s.read / total) * 100);
  return (
    <div className="flex flex-col gap-3 rounded-xl border p-3.5 hover:bg-muted/30">
      <button
        type="button"
        className="flex gap-3 text-left"
        onClick={() => setNav({ kind: "series", name: s.name })}
      >
        {/* The first books, fanned. */}
        <div className="relative h-24 w-[86px] shrink-0">
          {s.books.slice(0, 3).map((b, i) => (
            <div
              key={b.id}
              className="absolute top-0 w-16 shadow-md"
              style={{
                left: i * 11,
                top: i * 3,
                zIndex: 3 - i,
                transform: `rotate(${(i - 1) * 3}deg)`,
              }}
            >
              <BookCover book={b} className="rounded-[3px]" />
            </div>
          ))}
        </div>
        <div className="flex min-w-0 flex-col gap-0.5">
          <span className="line-clamp-2 font-semibold leading-snug">{s.name}</span>
          {s.authors.length > 0 && (
            <span className="truncate text-[12px] text-muted-foreground">
              {authorsText(s.authors, 2)}
            </span>
          )}
          <span className="text-[12px] text-muted-foreground">
            {total} books · {s.read} read
            {s.reading ? ` · ${s.reading} reading` : ""}
          </span>
        </div>
      </button>
      <div className="h-1.5 overflow-hidden rounded-full bg-muted" title={`${pct}% read`}>
        <div className={cn("h-full bg-primary")} style={{ width: `${pct}%` }} />
      </div>
      {s.next ? (
        <Button
          size="sm"
          variant="outline"
          className="justify-start"
          onClick={() => s.next && void open(s.next)}
        >
          <BookOpen />
          <span className="truncate">
            {s.next.user.status === "reading" ? "Continue" : "Next"}:{" "}
            {s.next.metadata.seriesNumber != null ? `${s.next.metadata.seriesNumber}. ` : ""}
            {s.next.metadata.title}
          </span>
        </Button>
      ) : (
        <p className="text-[12px] text-muted-foreground">All read</p>
      )}
    </div>
  );
}
