import { useEffect, useRef, useState } from "react";
import { BookPlus, ExternalLink, FolderOpen, Minus, Plus } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { usePermissions } from "@/features/profiles";
import { bookUrl, commands, unwrap } from "@/lib/ipc";
import { useTabs, type BookTab, type FeedDoc } from "@/lib/tabs";
import { createRenderer, type Renderer } from "@/readers";
import "@/readers/reader.css";
import { openBook } from "../open";
import { AddToLibraryDialog } from "./ItemDialogs";

const fail = (what: string) => (e: unknown) =>
  toast.error(what, { description: e instanceof Error ? e.message : String(e) });

/**
 * A download from Feeds, read in a tab with the library's own reader before
 * it is added to the library (highlights and notes come once it is a book).
 */
export function FeedDocView({ tab, feed }: { tab: BookTab; feed: FeedDoc }) {
  const host = useRef<HTMLDivElement>(null);
  const renderer = useRef<Renderer | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [zoom, setZoom] = useState(1);
  const [adding, setAdding] = useState(false);
  const { editLibrary } = usePermissions();

  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let r: Renderer | null = null;
    let gone = false;
    void (async () => {
      try {
        r = await createRenderer(
          tab.fileType,
          { relocate: () => {}, selection: () => {}, annotationClick: () => {} },
          tab.bookId,
        );
        if (gone) return r.destroy();
        await r.open(el, bookUrl(feed.file), null);
        if (gone) return r.destroy();
        renderer.current = r;
      } catch (e) {
        if (!gone)
          setError(
            e instanceof Error
              ? e.message
              : "The file could not be opened. It may have been deleted.",
          );
      }
    })();
    return () => {
      gone = true;
      renderer.current = null;
      r?.destroy();
    };
  }, [feed.file, tab.fileType, tab.bookId]);

  const zoomBy = (step: number) => {
    const z = Math.min(3, Math.max(0.5, Math.round((zoom + step) * 10) / 10));
    setZoom(z);
    renderer.current?.setZoom(z);
  };

  return (
    <div className="flex h-full min-h-0 flex-col">
      <div className="flex h-12 shrink-0 items-center gap-2 border-b px-4">
        <div className="flex min-w-0 flex-1 flex-col leading-tight">
          <span className="truncate font-medium" title={tab.title}>
            {tab.title}
          </span>
          <span className="truncate text-[11.5px] text-muted-foreground">
            {feed.source} · from Feeds, not in your library yet
          </span>
        </div>
        <div className="flex items-center rounded-lg border">
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label="Smaller"
            onClick={() => zoomBy(-0.1)}
          >
            <Minus />
          </Button>
          <span className="w-11 text-center text-[12px] tabular-nums">
            {Math.round(zoom * 100)}%
          </span>
          <Button
            variant="ghost"
            size="icon"
            className="size-7"
            aria-label="Larger"
            onClick={() => zoomBy(0.1)}
          >
            <Plus />
          </Button>
        </div>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Show in folder"
          title="Show in folder"
          onClick={() =>
            void unwrap(commands.feedsReveal(feed.file)).catch(fail("Could not show it"))
          }
        >
          <FolderOpen />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Open in another app"
          title="Open in another app"
          onClick={() =>
            void unwrap(commands.feedOpenFile(feed.file)).catch(fail("Could not open it"))
          }
        >
          <ExternalLink />
        </Button>
        {editLibrary && (
          <Button
            size="sm"
            onClick={() => setAdding(true)}
            title="Add it to your library, to highlight and take notes"
          >
            <BookPlus /> Add to library…
          </Button>
        )}
      </div>
      <div className="relative min-h-0 flex-1 overflow-hidden bg-muted/40">
        <div ref={host} className="absolute inset-0" />
        {error && (
          <p className="absolute inset-0 flex items-center justify-center p-6 text-center text-destructive">
            {error}
          </p>
        )}
      </div>
      <AddToLibraryDialog
        space={feed.space}
        item={adding ? { id: feed.id, title: tab.title, file: feed.file } : null}
        onClose={() => setAdding(false)}
        onAdded={(bookId) => {
          // The book takes the download's place.
          useTabs.getState().detach(tab.bookId);
          void openBook(bookId);
        }}
      />
    </div>
  );
}
