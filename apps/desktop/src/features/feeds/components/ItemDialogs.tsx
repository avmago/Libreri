import { useState } from "react";
import { BookPlus, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { NativeSelect } from "@/components/ui/input";
import { flattenFolders, useFolders } from "@/features/library";
import type { FeedItem, Space } from "@/lib/ipc";
import { useAddToLibrary } from "../api";
import { openBook } from "../open";

/** Chooses a folder of the library for an item, and adds it. */
export function AddToLibraryDialog({
  item,
  onClose,
  onAdded,
  space = "feeds",
}: {
  item: Pick<FeedItem, "id" | "title" | "file"> | null;
  onClose: () => void;
  /** Instead of offering to open the new book. */
  onAdded?: (bookId: string) => void;
  space?: Space;
}) {
  const { data: folders = [] } = useFolders();
  const [folder, setFolder] = useState("");
  const add = useAddToLibrary(space);
  return (
    <Dialog
      open={!!item}
      onOpenChange={(o) => !o && !add.isPending && onClose()}
      title="Add to library"
      description={
        item?.file
          ? `The download moves into the folder you choose and becomes ${space === "podcasts" ? "an audiobook" : "a book"} of your library, with the ${space === "podcasts" ? "show" : "feed"}'s details.`
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
                    action: onAdded
                      ? undefined
                      : { label: "Open", onClick: () => void openBook(bookId) },
                  });
                  onClose();
                  onAdded?.(bookId);
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
