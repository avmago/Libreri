import { Skeleton } from "@/components/ui/skeleton";
import { cn } from "@/lib/utils";

/** Says "loading" to screen readers; the skeletons are only for the eye. */
function Loading({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: React.ReactNode;
}) {
  return (
    <div role="status" aria-label={label} aria-busy className={className}>
      <span className="sr-only">{label}</span>
      <div aria-hidden className="contents">
        {children}
      </div>
    </div>
  );
}

/** Rows of a list (feeds, episodes, results): an optional picture and lines. */
export function RowsSkeleton({
  rows = 6,
  picture,
  label = "Loading…",
  className,
}: {
  rows?: number;
  /** A square on the left: "round" (a play button), "square" (artwork). */
  picture?: "round" | "square";
  label?: string;
  className?: string;
}) {
  return (
    <Loading label={label} className={cn("flex flex-col", className)}>
      {Array.from({ length: rows }, (_, i) => (
        <div key={i} className="flex gap-3 border-b px-5 py-3.5 last:border-b-0">
          {picture && (
            <Skeleton
              className={cn("size-9 shrink-0", picture === "round" ? "rounded-full" : "rounded-md")}
            />
          )}
          <div className="flex flex-1 flex-col gap-2">
            <Skeleton className="h-4" style={{ width: `${55 + ((i * 17) % 35)}%` }} />
            <Skeleton className="h-3 w-40" />
            <Skeleton className="h-3" style={{ width: `${70 + ((i * 11) % 25)}%` }} />
          </div>
        </div>
      ))}
    </Loading>
  );
}

/** Book covers in a grid, as the library shows them. */
export function CoversSkeleton({
  count = 12,
  label = "Loading books…",
}: {
  count?: number;
  label?: string;
}) {
  return (
    <Loading
      label={label}
      className="grid grid-cols-[repeat(auto-fill,minmax(132px,1fr))] gap-x-5 gap-y-6 px-6 pt-4 pb-10"
    >
      {Array.from({ length: count }, (_, i) => (
        <div key={i} className="flex flex-col gap-2">
          <Skeleton className="aspect-[2/3] w-full" />
          <Skeleton className="h-3.5 w-4/5" />
          <Skeleton className="h-3 w-1/2" />
        </div>
      ))}
    </Loading>
  );
}

/** A page being opened (a book, a PDF, a comparison). */
export function PageSkeleton({
  label = "Opening…",
  className,
}: {
  label?: string;
  className?: string;
}) {
  return (
    <Loading
      label={label}
      className={cn("flex h-full justify-center overflow-hidden p-6", className)}
    >
      <div className="flex aspect-[1/1.35] h-full max-h-[880px] max-w-full flex-col gap-3 rounded-md border bg-background p-[8%] shadow-sm">
        <Skeleton className="mb-4 h-6 w-2/5" />
        {Array.from({ length: 14 }, (_, i) => (
          <Skeleton key={i} className="h-3" style={{ width: i % 5 === 4 ? "60%" : "100%" }} />
        ))}
      </div>
    </Loading>
  );
}

/** Small page thumbnails in a row (page editors, captures). */
export function ThumbsSkeleton({
  count = 8,
  label = "Opening the pages…",
}: {
  count?: number;
  label?: string;
}) {
  return (
    <Loading label={label} className="flex flex-wrap gap-4 p-6">
      {Array.from({ length: count }, (_, i) => (
        <Skeleton key={i} className="aspect-[1/1.35] w-28" />
      ))}
    </Loading>
  );
}

/** A thin bar for something on its way (a download). */
export function BarSkeleton({ label, className }: { label: string; className?: string }) {
  return (
    <Loading label={label} className={cn("flex items-center gap-2", className)}>
      <Skeleton className="h-1.5 flex-1 rounded-full" />
      <span className="text-[11.5px] text-muted-foreground">{label}</span>
    </Loading>
  );
}
