import { useEffect } from "react";
import { toast } from "sonner";
import { events, type DetailsFilled } from "@/lib/ipc";
import { report } from "@/lib/windows";
import { useLibraryView } from "@/features/library";
import { useFillMissingDetails } from "../api";

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}

export function summarise(r: DetailsFilled) {
  const title = r.filled.length
    ? `Filled in details of ${plural(r.filled.length, "book")}`
    : r.unsure.length
      ? "Found no sure matches"
      : "No details to fill in";
  const parts: string[] = [];
  if (r.unsure.length) parts.push(`${plural(r.unsure.length, "book")} had no sure match`);
  if (r.unchanged) parts.push(`${plural(r.unchanged, "book")} unchanged`);
  if (r.failed.length) parts.push(`${plural(r.failed.length, "book")} could not be looked up`);
  const details = r.failed.slice(0, 2).map((f) => `${f.file}: ${f.reason}`);
  return { title, description: [parts.join(" · "), ...details].filter(Boolean).join("\n") };
}

/** Reports "Fill in missing details" (by hand or after an import). */
export function useDetailsEvents() {
  const setSelection = useLibraryView((s) => s.setSelection);
  useEffect(() => {
    const off = events.detailsFilled.listen(({ payload }) => {
      const { title, description } = summarise(payload);
      const show = payload.failed.length ? toast.warning : toast.success;
      report(() =>
        show(title, {
          description: description || undefined,
          duration: payload.unsure.length || payload.failed.length ? 10_000 : 4_000,
          action: payload.unsure.length
            ? {
                label: "Select them",
                onClick: () => setSelection(payload.unsure),
              }
            : undefined,
        }),
      );
    });
    return () => void off.then((f) => f());
  }, [setSelection]);
}

/** Starts "Fill in missing details" for some books. */
export function useFillDetails() {
  const fill = useFillMissingDetails();
  return (ids: string[]) => {
    if (!ids.length) return;
    fill.mutate(ids, {
      onSuccess: () =>
        toast(
          ids.length === 1
            ? "Looking up details of 1 book…"
            : `Looking up details of ${ids.length} books…`,
        ),
      onError: (e) => toast.error(e.message),
    });
  };
}
