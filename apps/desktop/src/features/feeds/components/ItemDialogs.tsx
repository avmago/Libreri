import { useEffect, useRef, useState } from "react";
import { BookPlus, ExternalLink, FolderOpen, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { NativeSelect } from "@/components/ui/input";
import { flattenFolders, useFolders } from "@/features/library";
import { bookUrl, commands, unwrap, type FeedItem } from "@/lib/ipc";
import { renderMarkdown, stripFrontMatter } from "@/lib/markdown";
import { createRenderer, type Renderer } from "@/readers";
import "@/readers/reader.css";
import { useAddToLibrary } from "../api";
import { openBook } from "../open";

const fail = (what: string) => (e: unknown) =>
  toast.error(what, { description: e instanceof Error ? e.message : String(e) });

/** Reads a download before deciding what to do with it. */
export function ItemPreview({
  item,
  onClose,
  onAdd,
}: {
  item: FeedItem | null;
  onClose: () => void;
  onAdd?: (item: FeedItem) => void;
}) {
  const file = item?.file ?? null;
  return (
    <Dialog
      open={!!file}
      onOpenChange={(o) => !o && onClose()}
      title={item?.title ?? ""}
      description={item ? `${item.source} · downloaded to Feeds` : undefined}
      className="h-[90vh] max-h-[92vh] w-[min(980px,calc(100vw-48px))]"
    >
      {file &&
        (file.toLowerCase().endsWith(".pdf") ? (
          <PdfPreview file={file} />
        ) : (
          <ArticlePreview file={file} />
        ))}
      {item && file && (
        <div className="flex flex-wrap justify-end gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void unwrap(commands.feedsReveal(file)).catch(fail("Could not show it"))}
          >
            <FolderOpen /> Show in folder
          </Button>
          <Button
            variant="outline"
            size="sm"
            onClick={() =>
              void unwrap(commands.feedOpenFile(file)).catch(fail("Could not open it"))
            }
          >
            <ExternalLink /> Open in another app
          </Button>
          {onAdd && (
            <Button size="sm" onClick={() => onAdd(item)}>
              <BookPlus /> Add to library…
            </Button>
          )}
        </div>
      )}
    </Dialog>
  );
}

function PdfPreview({ file }: { file: string }) {
  const host = useRef<HTMLDivElement>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    const el = host.current;
    if (!el) return;
    let renderer: Renderer | null = null;
    let gone = false;
    void (async () => {
      try {
        const r = await createRenderer(
          "pdf",
          { relocate: () => {}, selection: () => {}, annotationClick: () => {} },
          "",
        );
        if (gone) return;
        renderer = r;
        await r.open(el, bookUrl(file), null);
      } catch (e) {
        if (!gone) setError(e instanceof Error ? e.message : String(e));
      }
    })();
    return () => {
      gone = true;
      renderer?.destroy();
    };
  }, [file]);
  return (
    <div className="relative min-h-0 flex-1 overflow-hidden rounded-md border bg-muted">
      <div ref={host} className="absolute inset-0" />
      {error && (
        <p className="absolute inset-0 flex items-center justify-center p-6 text-center text-destructive">
          {error}
        </p>
      )}
    </div>
  );
}

function ArticlePreview({ file }: { file: string }) {
  const [html, setHtml] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let gone = false;
    fetch(bookUrl(file))
      .then((r) =>
        r.ok ? r.text() : Promise.reject(new Error(`The file could not be read (${r.status}).`)),
      )
      .then((text) => !gone && setHtml(renderMarkdown(stripFrontMatter(text))))
      .catch((e: unknown) => !gone && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      gone = true;
    };
  }, [file]);
  return (
    <div className="min-h-0 flex-1 overflow-auto rounded-md border bg-background">
      {error && <p className="p-6 text-destructive">{error}</p>}
      {!html && !error && (
        <p className="flex items-center gap-2 p-6 text-muted-foreground">
          <Loader2 className="size-4 animate-spin" /> Loading…
        </p>
      )}
      {html && (
        <article
          className="lb-doc lb-doc-md px-8 py-6"
          // Rendered by the app's Markdown renderer, which escapes HTML.
          dangerouslySetInnerHTML={{ __html: html }}
        />
      )}
    </div>
  );
}

/** Chooses a folder of the library for an item, and adds it. */
export function AddToLibraryDialog({
  item,
  onClose,
}: {
  item: FeedItem | null;
  onClose: () => void;
}) {
  const { data: folders = [] } = useFolders();
  const [folder, setFolder] = useState("");
  const add = useAddToLibrary();
  return (
    <Dialog
      open={!!item}
      onOpenChange={(o) => !o && !add.isPending && onClose()}
      title="Add to library"
      description={
        item?.file
          ? "The download moves into the folder you choose and becomes a book of your library, with the feed's details."
          : "It is downloaded, then moved into the folder you choose as a book of your library, with the feed's details."
      }
    >
      {item && (
        <p className="line-clamp-2 font-medium" title={item.title}>
          {item.title}
        </p>
      )}
      <label className="flex flex-col gap-1">
        <span className="text-[11.5px] font-medium text-muted-foreground">Into folder</span>
        <NativeSelect autoFocus value={folder} onChange={(e) => setFolder(e.target.value)}>
          <option value="">Books (top level)</option>
          {flattenFolders(folders).map(({ folder: f, depth }) => (
            <option key={f.path} value={f.path}>
              {"  ".repeat(depth + 1)}
              {f.name}
            </option>
          ))}
        </NativeSelect>
      </label>
      <div className="flex justify-end gap-2">
        <Button variant="outline" onClick={onClose} disabled={add.isPending}>
          Cancel
        </Button>
        <Button
          disabled={!item || add.isPending}
          onClick={() =>
            item &&
            add.mutate(
              { id: item.id, folder },
              {
                onSuccess: (bookId) => {
                  toast.success("Added to your library", {
                    description: folder ? `In ${folder}` : "In Books",
                    action: { label: "Open", onClick: () => void openBook(bookId) },
                  });
                  onClose();
                },
              },
            )
          }
        >
          {add.isPending ? <Loader2 className="animate-spin" /> : <BookPlus />}
          {add.isPending && !item?.file ? "Downloading…" : "Add"}
        </Button>
      </div>
    </Dialog>
  );
}
