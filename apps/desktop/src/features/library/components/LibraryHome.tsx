import { BookOpen, Upload } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { LibrarySummary } from "@/lib/ipc";

/** The library's main view. Phase 0 shows the empty state; Phase 1 adds the grid. */
export function LibraryHome({ library }: { library: LibrarySummary }) {
  return (
    <div className="flex h-full flex-col">
      <header className="flex items-center gap-3 px-6 pt-5 pb-3">
        <h1 className="text-xl font-semibold tracking-tight">All Books</h1>
        <span className="text-muted-foreground">{library.bookCount} items</span>
        <div className="flex-1" />
        <Button disabled title="Importing arrives in the next update">
          <Upload /> Import
        </Button>
      </header>
      <div className="flex flex-1 flex-col items-center justify-center gap-3 border-t text-center">
        <div className="flex size-12 items-center justify-center rounded-xl bg-muted">
          <BookOpen className="size-6 text-muted-foreground" aria-hidden />
        </div>
        <h2 className="text-base font-semibold">No books yet</h2>
        <p className="max-w-sm text-muted-foreground">
          Your library is ready at <span className="font-mono text-xs">{library.path}</span>.
          Importing books arrives in the next update.
        </p>
      </div>
    </div>
  );
}
