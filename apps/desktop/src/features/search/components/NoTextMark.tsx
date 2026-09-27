import { useMemo } from "react";
import { ScanText } from "lucide-react";
import { useBooksWithoutText } from "../api";

/** Marks a book whose pages are scans without text (it cannot be searched yet). */
export function NoTextMark({ id }: { id: string }) {
  const { data } = useBooksWithoutText();
  const state = useMemo(() => data?.find((b) => b.id === id)?.state, [data, id]);
  if (!state) return null;
  const label =
    state === "noText"
      ? "Scanned pages without text: make it searchable from the book's menu"
      : "Some pages are scans without text";
  return (
    <span title={label} className="shrink-0 text-amber-600 dark:text-amber-400">
      <ScanText className="size-3" aria-label={label} />
    </span>
  );
}
