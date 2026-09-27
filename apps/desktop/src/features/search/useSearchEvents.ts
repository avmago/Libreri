import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { events } from "@/lib/ipc";
import { searchKey } from "./api";
import { useIndexing } from "./store";

/** Keeps indexing progress current and reports finished OCR runs. */
export function useSearchEvents() {
  const qc = useQueryClient();
  useEffect(() => {
    let finishedAt = 0;
    const offs = [
      events.searchIndexProgress.listen(({ payload: p }) => {
        useIndexing.getState().set(p);
        // Refresh results when a pass ends, and now and then during a long one.
        const now = Date.now();
        if (!p.running || now - finishedAt > 5_000) {
          finishedAt = now;
          void qc.invalidateQueries({ queryKey: searchKey });
        }
      }),
      events.ocrFinished.listen(({ payload: r }) => {
        void qc.invalidateQueries({ queryKey: searchKey });
        const pages = r.pagesRead === 1 ? "1 page" : `${r.pagesRead} pages`;
        const books = r.bookIds.length === 1 ? "The book" : `${r.books} books`;
        if (r.errors.length && r.pagesRead === 0) {
          toast.error("OCR could not read the pages", {
            description: r.errors.slice(0, 3).join("\n"),
            duration: 12_000,
          });
        } else if (r.errors.length || r.pagesFailed) {
          toast.warning(`${books} can now be searched (${pages} read)`, {
            description: [
              r.pagesFailed ? `${r.pagesFailed} pages could not be read.` : "",
              ...r.errors.slice(0, 3),
            ]
              .filter(Boolean)
              .join("\n"),
            duration: 12_000,
          });
        } else if (r.pagesRead) {
          toast.success(`${books} can now be searched`, { description: `${pages} read with OCR.` });
        } else {
          toast.success("Nothing to read", {
            description: "Every page already has text.",
          });
        }
      }),
    ];
    return () => {
      for (const p of offs) void p.then((off) => off());
    };
  }, [qc]);
}
