import { ScanText } from "lucide-react";
import { usePermissions } from "@/features/profiles";
import { cn } from "@/lib/utils";
import { useTextStatus } from "../api";
import { describeText } from "../model";
import { useOcrDialog } from "../store";

const TONE = {
  ok: "text-foreground",
  warn: "text-amber-700 dark:text-amber-400",
  muted: "text-muted-foreground",
  error: "text-destructive",
};

/** "Search: Searchable / Scanned pages without text — Make searchable…" */
export function TextStatusRow({ bookId }: { bookId: string }) {
  const { data: s } = useTextStatus(bookId);
  const { editLibrary } = usePermissions();
  const open = useOcrDialog((x) => x.open);
  if (!s) return null;
  const d = describeText(s.state, s);
  const offer = s.canOcr && editLibrary && (s.state === "noText" || s.state === "partial");
  return (
    <div className="grid grid-cols-[92px_1fr] gap-2 py-1">
      <dt className="text-muted-foreground">Search</dt>
      <dd className="flex min-w-0 flex-col items-start gap-1">
        <span className={cn(TONE[d.tone])} title={s.message ?? undefined}>
          {d.label}
          {s.state === "failed" && s.message ? `: ${s.message}` : ""}
        </span>
        {offer && (
          <button
            type="button"
            onClick={() => open([bookId])}
            className="flex items-center gap-1 text-[12px] font-medium underline-offset-2 hover:underline"
          >
            <ScanText className="size-3.5" aria-hidden /> Make searchable…
          </button>
        )}
      </dd>
    </div>
  );
}
