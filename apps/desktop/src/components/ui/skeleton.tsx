import { cn } from "@/lib/utils";

/** shadcn/ui's Skeleton: a placeholder shown while something loads (on
 * `muted-foreground`, as Libreri's accent is its strong colour). */
function Skeleton({ className, ...props }: React.ComponentProps<"div">) {
  return (
    <div
      data-slot="skeleton"
      className={cn("animate-pulse rounded-md bg-muted-foreground/15", className)}
      {...props}
    />
  );
}

export { Skeleton };
