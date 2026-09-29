import { useState } from "react";
import { Headphones, Link2, Loader2, Unlink } from "lucide-react";
import { toast } from "sonner";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { commands, unwrap, type BookDto } from "@/lib/ipc";
import type { BookView } from "../model";

/** Audio formats (an audiobook is a book in one of these). */
const AUDIO = ["mp3", "m4b", "m4a", "aac", "ogg", "opus", "flac"];
const isAudioBook = (b: { fileType: string }) => AUDIO.includes(b.fileType);

/** A title to search with: without "(Audiobook)", "Unabridged" and the
 * like, which the other edition's title does not have. */
function plainTitle(title: string): string {
  return title
    .replace(/[([][^)\]]*[)\]]/g, " ")
    .replace(/\b(unabridged|abridged|audiobook|audio book|narrated by.*)$/gi, " ")
    .replace(/\s+/g, " ")
    .trim();
}

/** The reader's queries for audiobook links, refreshed after a change. */
const readerKey = ["lib", "reader"];

/**
 * Links between an audiobook and the book it reads (Edit details). An
 * audiobook reads one book; a book can have several audiobooks. Changes
 * are saved at once; sync points are set in the audiobook player.
 */
export function AudioLinks({ book, editable }: { book: BookView; editable: boolean }) {
  const qc = useQueryClient();
  const audio = isAudioBook(book);
  const [picking, setPicking] = useState(false);

  // An audiobook: the book it reads.
  const { data: link, isPending: linkPending } = useQuery({
    queryKey: [...readerKey, book.id, "audio-link"],
    queryFn: () => unwrap(commands.getAudioLink(book.id)),
    enabled: audio,
  });
  // A book: the audiobooks that read it.
  const { data: audiobooks = [], isPending: listPending } = useQuery({
    queryKey: [...readerKey, book.id, "audiobooks"],
    queryFn: () => unwrap(commands.audiobooksFor(book.id)),
    enabled: !audio,
  });
  const setLink = useMutation({
    mutationFn: ({ audioId, textId }: { audioId: string; textId: string | null }) =>
      unwrap(commands.setAudioLink(audioId, textId)),
    onSuccess: () => void qc.invalidateQueries({ queryKey: readerKey }),
    onError: (e) => toast.error("Could not change the link", { description: String(e) }),
  });

  if (audio ? linkPending : listPending) return null;
  const linked: BookDto[] = audio ? (link?.text ? [link.text] : []) : audiobooks;
  if (!editable && !linked.length) return null;

  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="flex items-center gap-1.5 text-[11.5px] font-medium text-muted-foreground">
        <Headphones className="size-3.5" aria-hidden />
        {audio ? "Reads the book" : linked.length > 1 ? "Audiobooks" : "Audiobook"}
      </h3>
      {linked.map((b) => (
        <div key={b.id} className="flex items-center gap-2 rounded-md border px-2.5 py-1.5">
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="truncate font-medium">{b.metadata.title}</span>
            <span className="truncate text-[12px] text-muted-foreground">
              {[(b.metadata.authors ?? []).join(", "), b.fileType.toUpperCase()]
                .filter(Boolean)
                .join(" · ")}
            </span>
          </span>
          {editable && (
            <Button
              variant="ghost"
              size="icon"
              aria-label={`Unlink ${b.metadata.title}`}
              title="Unlink"
              disabled={setLink.isPending}
              onClick={() =>
                setLink.mutate(
                  { audioId: audio ? book.id : b.id, textId: null },
                  { onSuccess: () => toast("Unlinked") },
                )
              }
            >
              <Unlink />
            </Button>
          )}
        </div>
      ))}
      {editable && (
        <>
          {!linked.length && (
            <p className="text-[12px] text-muted-foreground">
              {audio
                ? "Link the ebook or PDF this audiobook reads: the book can then follow the audio, and “Listen from here” opens the audiobook at the place you are reading."
                : "Link an audiobook of this book: it can follow your reading, and the book follows the audio."}
            </p>
          )}
          {(!audio || !linked.length) && (
            <Button
              variant="outline"
              size="sm"
              className="self-start"
              disabled={setLink.isPending}
              onClick={() => setPicking(true)}
            >
              {setLink.isPending ? <Loader2 className="animate-spin" /> : <Link2 />}
              {audio ? "Link to a book…" : "Link an audiobook…"}
            </Button>
          )}
          {audio && linked.length > 0 && (
            <Button
              variant="ghost"
              size="sm"
              className="self-start"
              onClick={() => setPicking(true)}
            >
              <Link2 /> Link to another book…
            </Button>
          )}
          <BookPickDialog
            key={picking ? "open" : "closed"}
            open={picking}
            audio={!audio}
            initial={plainTitle(book.metadata.title)}
            exclude={linked.map((b) => b.id)}
            onClose={() => setPicking(false)}
            onPick={(id) => {
              setPicking(false);
              setLink.mutate(
                { audioId: audio ? book.id : id, textId: audio ? id : book.id },
                {
                  onSuccess: () =>
                    toast.success("Linked", {
                      description: "Set sync points in the audiobook player to line them up.",
                    }),
                },
              );
            }}
          />
        </>
      )}
    </section>
  );
}

/** Finds a book to link: audiobooks, or books with text. */
function BookPickDialog({
  open,
  audio,
  initial,
  exclude,
  onClose,
  onPick,
}: {
  open: boolean;
  /** List audiobooks (otherwise books with text). */
  audio: boolean;
  initial: string;
  exclude: string[];
  onClose: () => void;
  onPick: (id: string) => void;
}) {
  const [search, setSearch] = useState(initial);
  const { data: books = [], isFetching } = useQuery({
    queryKey: ["lib", "link-books", audio, search],
    queryFn: () => unwrap(commands.listBooks({ search: search || null, sort: "title" })),
    enabled: open,
  });
  const shown = books.filter((b) => isAudioBook(b) === audio && !exclude.includes(b.id));
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={audio ? "Link an audiobook" : "Link to a book"}
      description={
        audio
          ? "An audiobook already linked to another book moves to this one."
          : "The ebook or PDF with the text this audiobook reads."
      }
      className="w-[520px]"
    >
      <input
        autoFocus
        aria-label="Find a book"
        value={search}
        onChange={(e) => setSearch(e.target.value)}
        placeholder={audio ? "Find an audiobook" : "Find a book"}
        className="h-8 rounded-md border border-input bg-background px-2 outline-none focus-visible:border-ring"
      />
      <ul className="flex max-h-72 flex-col overflow-auto rounded-md border">
        {shown.map((b) => (
          <li key={b.id}>
            <button
              type="button"
              onClick={() => onPick(b.id)}
              className="flex w-full flex-col items-start px-3 py-1.5 text-left hover:bg-muted"
            >
              <span className="font-medium">{b.metadata.title}</span>
              <span className="text-[12px] text-muted-foreground">
                {[(b.metadata.authors ?? []).join(", "), b.fileType.toUpperCase()]
                  .filter(Boolean)
                  .join(" · ")}
              </span>
            </button>
          </li>
        ))}
        {!shown.length && (
          <li className="flex items-center gap-2 px-3 py-2 text-muted-foreground">
            {isFetching && <Loader2 className="size-3.5 animate-spin" />}
            {isFetching
              ? "Looking…"
              : search
                ? "Nothing found. Try fewer words."
                : audio
                  ? "No audiobooks in the library."
                  : "No books found."}
          </li>
        )}
      </ul>
    </Dialog>
  );
}
