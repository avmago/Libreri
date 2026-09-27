import type { ReactNode } from "react";
import { AlertTriangle, CheckCircle2, Wrench } from "lucide-react";
import { toast } from "sonner";
import { ask } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { commands, type BookRef, type HealthReportDto } from "@/lib/ipc";
import { useHealthCheck, useRepairHealth } from "../api";
import { formatBytes, formatWhen } from "../model";
import { usePortability } from "../store";

/** Library health check: what is broken, and what Libreri can fix. */
export function HealthDialog() {
  const open = usePortability((s) => s.health);
  const close = usePortability((s) => s.close);
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && close()}
      title="Library health"
      description="Checks that every book has its file and its backup, and that every link in your notes still opens a book."
      className="w-[620px]"
    >
      {open && <Body onDone={close} />}
    </Dialog>
  );
}

function Section({
  title,
  count,
  children,
  action,
}: {
  title: string;
  count: number;
  children?: ReactNode;
  action?: ReactNode;
}) {
  if (count === 0) return null;
  return (
    <section className="flex flex-col gap-1.5 rounded-lg border px-3 py-2.5">
      <div className="flex items-center gap-2">
        <AlertTriangle className="size-4 text-amber-600" aria-hidden />
        <h3 className="flex-1 font-medium">
          {title} <span className="text-muted-foreground tabular-nums">({count})</span>
        </h3>
        {action}
      </div>
      {children}
    </section>
  );
}

function Books({ books }: { books: BookRef[] }) {
  return (
    <ul className="flex max-h-32 flex-col overflow-auto pl-6 text-[12.5px]">
      {books.slice(0, 50).map((b) => (
        <li key={b.id} className="truncate" title={b.relPath}>
          {b.title}
        </li>
      ))}
      {books.length > 50 && (
        <li className="text-muted-foreground">…and {books.length - 50} more</li>
      )}
    </ul>
  );
}

function allClear(r: HealthReportDto) {
  return (
    !r.missingFiles.length &&
    !r.brokenLinks.length &&
    !r.duplicates.length &&
    !r.unreadableBackups.length &&
    !r.database.length &&
    !r.fixable
  );
}

function Body({ onDone }: { onDone: () => void }) {
  const { data: r, error, isLoading, refetch, isFetching } = useHealthCheck(true);
  const repair = useRepairHealth();
  const showMissing = usePortability((s) => s.showMissing);

  if (isLoading) return <p className="text-muted-foreground">Checking the library…</p>;
  if (error || !r) return <p className="text-destructive">{error?.message}</p>;

  const fix = () =>
    repair.mutate(undefined, {
      onSuccess: (n) => {
        toast.success(n === 1 ? "Fixed 1 thing" : `Fixed ${n} things`);
        void refetch();
      },
      onError: (e) => toast.error(e.message),
    });
  const rebuild = async () => {
    const ok = await ask(
      "Libreri rebuilds its catalogue from the book files and the JSON backups. Your books and notes are not touched; the old catalogue is kept as library.db.bak.",
      { title: "Rebuild the library index?", okLabel: "Rebuild" },
    );
    if (ok) {
      const res = await commands.rebuildLibraryIndex();
      if (res.status === "error") toast.error(res.error.message);
      else {
        toast("Rebuilding the library index…");
        onDone();
      }
    }
  };

  return (
    <div className="flex flex-col gap-3">
      <p className="text-[12.5px] text-muted-foreground">
        {r.books} books · checked {formatWhen(r.checkedAt)}
      </p>
      {allClear(r) && (
        <p className="flex items-center gap-2 rounded-lg border px-3 py-3">
          <CheckCircle2 className="size-4 text-green-600" aria-hidden />
          Everything looks right.
        </p>
      )}
      <Section
        title="Books whose file is missing"
        count={r.missingFiles.length}
        action={
          <Button size="sm" variant="outline" onClick={showMissing}>
            Show them
          </Button>
        }
      >
        <p className="pl-6 text-[12.5px] text-muted-foreground">
          Their notes are kept. Select one and choose Locate file…, or add the file again.
        </p>
        <Books books={r.missingFiles} />
      </Section>
      <Section title="Links in your notes that open no book" count={r.brokenLinks.length}>
        <ul className="flex max-h-32 flex-col overflow-auto pl-6 font-mono text-[11.5px]">
          {r.brokenLinks.slice(0, 50).map((l) => (
            <li key={`${l.note}:${l.line}:${l.link}`} className="truncate" title={l.link}>
              {l.note}:{l.line} · {l.link}
            </li>
          ))}
        </ul>
      </Section>
      <Section title="Books that share an ISBN or DOI" count={r.duplicates.length}>
        <ul className="flex max-h-32 flex-col gap-1 overflow-auto pl-6 text-[12.5px]">
          {r.duplicates.map((d) => (
            <li key={d.key}>
              <span className="text-muted-foreground">{d.key}:</span>{" "}
              {d.books.map((b) => b.title).join(" · ")}
            </li>
          ))}
        </ul>
      </Section>
      {r.otherFileNotes.length > 0 && (
        <section className="flex flex-col gap-1 rounded-lg border px-3 py-2.5">
          <h3 className="font-medium">Notes made in another copy of the book</h3>
          <p className="text-[12.5px] text-muted-foreground">
            Not a problem: these notes find their place by the words they quote.
          </p>
          <ul className="flex max-h-24 flex-col overflow-auto text-[12.5px]">
            {r.otherFileNotes.map((b) => (
              <li key={b.book.id}>
                {b.book.title} <span className="text-muted-foreground">({b.notes} notes)</span>
              </li>
            ))}
          </ul>
        </section>
      )}
      <Section
        title="Things Libreri can fix by itself"
        count={r.missingSidecars + r.staleNotebooks + r.unusedCovers}
        action={
          <Button size="sm" onClick={fix} disabled={repair.isPending}>
            <Wrench /> Fix
          </Button>
        }
      >
        <ul className="pl-6 text-[12.5px] text-muted-foreground">
          {r.missingSidecars > 0 && <li>{r.missingSidecars} books without a JSON backup</li>}
          {r.staleNotebooks > 0 && <li>{r.staleNotebooks} notebooks that were deleted</li>}
          {r.unusedCovers > 0 && (
            <li>
              {r.unusedCovers} covers of removed books ({formatBytes(r.unusedCoverBytes)})
            </li>
          )}
        </ul>
      </Section>
      <Section title="Backups of notes that cannot be read" count={r.unreadableBackups.length}>
        <ul className="pl-6 font-mono text-[11.5px]">
          {r.unreadableBackups.slice(0, 20).map((f) => (
            <li key={f}>{f}</li>
          ))}
        </ul>
      </Section>
      <Section
        title="Problems in the catalogue"
        count={r.database.length}
        action={
          <Button size="sm" variant="outline" onClick={() => void rebuild()}>
            Rebuild…
          </Button>
        }
      >
        <ul className="pl-6 text-[12.5px]">
          {r.database.map((d) => (
            <li key={d}>{d}</li>
          ))}
        </ul>
      </Section>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={() => void refetch()} disabled={isFetching}>
          Check again
        </Button>
        <Button onClick={onDone}>Done</Button>
      </div>
    </div>
  );
}
