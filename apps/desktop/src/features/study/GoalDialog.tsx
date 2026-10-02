import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useBooks } from "@/features/library";
import { cn } from "@/lib/utils";
import { dayKey, newId, type Goal } from "./model";
import { useReading } from "./store";

/** Sets or changes a reading goal: finish a book by a date, or something
 * to have read by a day. */
export function GoalDialog({
  goal,
  defaultDue,
  onClose,
  onSave,
}: {
  goal: Goal | null;
  defaultDue: string;
  onClose: () => void;
  onSave: (g: Goal) => void;
}) {
  const reading = useReading((s) => s.now);
  const [kind, setKind] = useState<Goal["kind"]>(goal?.kind ?? "finish");
  const [book, setBook] = useState<{ id: string; title: string; pages: number | null } | null>(
    goal?.bookId ? { id: goal.bookId, title: goal.title, pages: goal.pages ?? null } : null,
  );
  const [title, setTitle] = useState(goal?.kind === "due" ? goal.title : "");
  const [search, setSearch] = useState("");
  const [due, setDue] = useState(goal?.due ?? defaultDue);
  const [pages, setPages] = useState<string>(goal?.pages ? String(goal.pages) : "");
  const { data: found = [] } = useBooks(
    { search: search.trim() || null, sort: "lastOpened", descending: true, audio: false },
    !book && search.trim().length > 1,
  );

  const pick = (b: { id: string; title: string; pages: number | null }) => {
    setBook(b);
    if (!pages && b.pages) setPages(String(b.pages));
  };

  const ok =
    !!due &&
    (kind === "finish"
      ? !!book && Number(pages) > 0
      : (title.trim() || book?.title || "").length > 0);

  const submit = () => {
    if (!ok) return;
    const startPage =
      goal?.startPage ?? (reading && book && reading.bookId === book.id ? reading.page : null) ?? 0;
    onSave({
      id: goal?.id ?? newId(),
      kind,
      title: kind === "due" ? title.trim() || book?.title || "" : book!.title,
      bookId: book?.id ?? null,
      due,
      pages: kind === "finish" ? Number(pages) : null,
      startPage: kind === "finish" ? startPage : null,
      reached: goal?.reached ?? (kind === "finish" ? startPage : null),
      done: goal?.done ?? false,
      created: goal?.created ?? new Date().toISOString(),
    });
    onClose();
  };

  return (
    <Dialog
      open
      onOpenChange={(o) => !o && onClose()}
      title={goal ? "Change the goal" : "New reading goal"}
      description="Goals show on the calendar, with the pages a day to finish on time."
    >
      <div role="radiogroup" aria-label="Kind" className="flex rounded-lg bg-muted p-0.5">
        {(
          [
            ["finish", "Finish a book"],
            ["due", "Something due"],
          ] as const
        ).map(([k, name]) => (
          <button
            key={k}
            type="button"
            role="radio"
            aria-checked={kind === k}
            onClick={() => setKind(k)}
            className={cn(
              "flex-1 rounded-md py-1.5 text-[13px] font-medium",
              kind === k ? "bg-background shadow-sm" : "text-muted-foreground",
            )}
          >
            {name}
          </button>
        ))}
      </div>

      {kind === "due" && (
        <label className="flex flex-col gap-1">
          <span className="font-medium">What is due</span>
          <Input
            autoFocus
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder="Chapter 4, the seminar paper…"
          />
        </label>
      )}

      <div className="flex flex-col gap-1">
        <span className="font-medium">{kind === "finish" ? "Book" : "Book (optional)"}</span>
        {book ? (
          <div className="flex items-center gap-2 rounded-md border px-3 py-2">
            <span className="flex-1 truncate">{book.title}</span>
            <Button variant="ghost" size="sm" onClick={() => setBook(null)}>
              Change
            </Button>
          </div>
        ) : (
          <>
            {reading && !reading.bookId.startsWith("feed:") && (
              <Button
                variant="outline"
                size="sm"
                className="self-start"
                onClick={() =>
                  pick({ id: reading.bookId, title: reading.title, pages: reading.pages })
                }
              >
                The book open now: {reading.title}
              </Button>
            )}
            <Input
              autoFocus={kind === "finish"}
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Find a book by title or author"
              aria-label="Find a book"
            />
            {found.length > 0 && (
              <div className="flex max-h-44 flex-col overflow-auto rounded-md border">
                {found.slice(0, 8).map((b) => (
                  <button
                    key={b.id}
                    type="button"
                    className="truncate px-3 py-1.5 text-left hover:bg-muted"
                    onClick={() =>
                      pick({
                        id: b.id,
                        title: b.metadata.title || "Book",
                        pages: b.metadata.pages ?? null,
                      })
                    }
                  >
                    {b.metadata.title || "Book"}
                    {b.metadata.authors?.length ? (
                      <span className="text-muted-foreground">
                        {" "}
                        · {b.metadata.authors.join(", ")}
                      </span>
                    ) : null}
                  </button>
                ))}
              </div>
            )}
          </>
        )}
      </div>

      <div className="flex gap-3">
        <label className="flex flex-1 flex-col gap-1">
          <span className="font-medium">By</span>
          <Input
            type="date"
            value={due}
            min={dayKey(new Date())}
            onChange={(e) => setDue(e.target.value)}
          />
        </label>
        {kind === "finish" && (
          <label className="flex w-36 flex-col gap-1">
            <span className="font-medium">Up to page</span>
            <Input
              type="number"
              min={1}
              value={pages}
              onChange={(e) => setPages(e.target.value)}
              placeholder="300"
            />
          </label>
        )}
      </div>

      <div className="flex justify-end gap-2">
        <Button variant="outline" onClick={onClose}>
          Cancel
        </Button>
        <Button disabled={!ok} onClick={submit}>
          {goal ? "Save" : "Add goal"}
        </Button>
      </div>
    </Dialog>
  );
}
