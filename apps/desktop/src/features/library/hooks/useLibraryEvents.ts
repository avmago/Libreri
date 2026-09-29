import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { toast } from "sonner";
import { create } from "zustand";
import { events, type ImportFinished } from "@/lib/ipc";
import { report } from "@/lib/windows";
import { libKey } from "../api";

export interface RunningJob {
  id: string;
  label: string;
  done: number;
  total: number;
  message: string | null;
}

interface JobsState {
  jobs: Record<string, RunningJob>;
  update: (id: string, change: Partial<RunningJob> | null) => void;
}

/** Background jobs (imports, scans) currently running, for the progress strip. */
export const useJobs = create<JobsState>((set) => ({
  jobs: {},
  update: (id, change) =>
    set((s) => {
      const jobs = { ...s.jobs };
      if (change === null) delete jobs[id];
      else
        jobs[id] = {
          ...(jobs[id] ?? { id, label: "Working…", done: 0, total: 0, message: null }),
          ...change,
        };
      return { jobs };
    }),
}));

function plural(n: number, one: string, many = `${one}s`) {
  return `${n} ${n === 1 ? one : many}`;
}

function summarise(r: ImportFinished) {
  const parts: string[] = [];
  if (r.relinked) parts.push(`${plural(r.relinked, "missing file")} found again`);
  if (r.duplicates.length) parts.push(`${plural(r.duplicates.length, "duplicate")} skipped`);
  if (r.unsupported) parts.push(`${plural(r.unsupported, "unsupported file")} ignored`);
  if (r.failed.length) parts.push(`${plural(r.failed.length, "file")} could not be imported`);
  const title = r.added
    ? `Imported ${plural(r.added, "book")}`
    : r.relinked
      ? "Import finished"
      : "Nothing new to import";
  const details = [
    ...r.duplicates
      .slice(0, 3)
      .map((d) => `${d.file} is already in the library as “${d.existingTitle}”`),
    ...r.failed.slice(0, 3).map((f) => `${f.file}: ${f.reason}`),
  ];
  return { title, description: [parts.join(" · "), ...details].filter(Boolean).join("\n") };
}

/**
 * Listens to Rust events while a library is open: refreshes lists when the
 * library changes, tracks job progress and reports finished imports.
 */
export function useLibraryEvents() {
  const qc = useQueryClient();
  const update = useJobs((s) => s.update);

  useEffect(() => {
    // An import changes the library many times a second: refresh at once,
    // then at most every half second while changes keep coming.
    let timer: ReturnType<typeof setTimeout> | undefined;
    let again = false;
    const refresh = () => {
      if (timer) {
        again = true;
        return;
      }
      void qc.invalidateQueries({ queryKey: libKey });
      timer = setTimeout(() => {
        timer = undefined;
        if (again) {
          again = false;
          refresh();
        }
      }, 500);
    };
    const unlisten = [
      events.libraryChanged.listen(refresh),
      events.jobEventPayload.listen(({ payload: e }) => {
        switch (e.kind) {
          case "started":
            update(e.id, { label: e.label ?? "Working…" });
            break;
          case "progress":
            update(e.id, { done: e.done ?? 0, total: e.total ?? 0, message: e.message });
            break;
          case "failed":
            update(e.id, null);
            report(() =>
              toast.error("Something went wrong", { description: e.message ?? undefined }),
            );
            break;
          default:
            update(e.id, null);
        }
      }),
      events.importFinished.listen(({ payload }) => {
        const { title, description } = summarise(payload);
        const hasProblems = payload.failed.length > 0;
        report(() =>
          (hasProblems ? toast.warning : toast.success)(title, {
            description: description || undefined,
            duration: hasProblems || payload.duplicates.length ? 10_000 : 4_000,
          }),
        );
      }),
    ];
    return () => {
      clearTimeout(timer);
      for (const p of unlisten) void p.then((off) => off());
    };
  }, [qc, update]);
}
