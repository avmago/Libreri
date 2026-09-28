import { useState } from "react";
import { FileVideo, Globe, Loader2 } from "lucide-react";
import { open as openFiles } from "@tauri-apps/plugin-dialog";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import type { LinkFileDto, LinkPreviewDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { SpellTextarea } from "@/features/spell";
import { fetchLink, linkFile, saveLink } from "./api";
import { LinkCard } from "./LinkCard";
import { formatTime, parseTime, type LinkInfo } from "./model";

const message = (e: unknown) => String((e as Error).message ?? e);

/**
 * Adds a link to a place in the book: a web page (details fetched once,
 * and an offline copy if wanted), a video with a start time, a video or
 * recording on this computer, or another book (`libreri://` link).
 */
export function AddLinkDialog({
  open,
  where,
  bookId,
  onClose,
  onSave,
}: {
  open: boolean;
  /** "p. 4", or the selected words. */
  where: string;
  bookId: string;
  onClose: () => void;
  onSave: (link: LinkInfo, note: string | null) => void;
}) {
  const [mode, setMode] = useState<"web" | "file">("web");
  const [address, setAddress] = useState("");
  const [preview, setPreview] = useState<LinkPreviewDto | null>(null);
  const [file, setFile] = useState<LinkFileDto | null>(null);
  const [title, setTitle] = useState("");
  const [start, setStart] = useState("");
  const [keepCopy, setKeepCopy] = useState(true);
  const [note, setNote] = useState("");
  const [busy, setBusy] = useState<"fetching" | "saving" | null>(null);
  const [error, setError] = useState<string | null>(null);

  const isBook = /^libreri:\/\/book\//i.test(address.trim());
  const startSecs = parseTime(start);
  const badStart = startSecs !== null && Number.isNaN(startSecs);
  const video =
    (mode === "file" && file !== null) || (mode === "web" && preview?.details.kind === "video");

  const getDetails = async () => {
    if (!address.trim() || isBook) return;
    setBusy("fetching");
    setError(null);
    setPreview(null);
    try {
      const p = await fetchLink(address.trim());
      setPreview(p);
      setTitle(p.details.title ?? "");
      if (p.details.start) setStart(formatTime(p.details.start));
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(null);
    }
  };

  const chooseFile = async () => {
    const picked = await openFiles({
      multiple: false,
      filters: [
        {
          name: "Video and audio",
          extensions: [
            "mp4",
            "m4v",
            "webm",
            "mov",
            "mkv",
            "ogv",
            "mp3",
            "m4a",
            "m4b",
            "aac",
            "ogg",
            "opus",
            "flac",
            "wav",
          ],
        },
      ],
    });
    if (typeof picked !== "string") return;
    setError(null);
    try {
      const f = await linkFile(picked);
      setFile(f);
      setTitle(f.name.replace(/\.[^.]+$/, ""));
    } catch (e) {
      setError(message(e));
    }
  };

  const save = async () => {
    const at = video && startSecs && !badStart ? startSecs : null;
    if (mode === "file") {
      if (!file) return;
      onSave(
        {
          url: "",
          kind: "file",
          title: title.trim() || file.name,
          file: file.file,
          media: file.media === "audio" ? "audio" : "video",
          start: at,
        },
        note.trim() || null,
      );
      return;
    }
    if (isBook) {
      onSave(
        { url: address.trim(), kind: "book", title: title.trim() || "Another book" },
        note.trim() || null,
      );
      return;
    }
    const d = preview?.details;
    if (!d) {
      // Could not fetch (offline?): keep the address as it is.
      onSave(
        {
          url: /^https?:\/\//i.test(address.trim()) ? address.trim() : `https://${address.trim()}`,
          kind: "web",
          title: title.trim() || address.trim(),
        },
        note.trim() || null,
      );
      return;
    }
    setBusy("saving");
    try {
      const kept = await saveLink(
        d.url,
        title.trim() || d.title || d.site,
        keepCopy && preview.canCopy,
      );
      onSave(
        {
          url: d.url,
          kind: d.kind,
          title: title.trim() || d.title || d.site,
          site: d.site,
          author: d.author,
          description: d.description,
          duration: d.duration,
          start: at,
          video: d.video,
          embed: d.embed,
          picture: kept.picture,
          copy: kept.copy,
        },
        note.trim() || null,
      );
    } catch (e) {
      setError(message(e));
    } finally {
      setBusy(null);
    }
  };

  const canSave =
    !busy &&
    !badStart &&
    (mode === "file" ? !!file : isBook || !!preview || (!!error && !!address.trim()));

  const shown: LinkInfo | null =
    mode === "file" && file
      ? {
          url: "",
          kind: "file",
          title: title || file.name,
          file: file.file,
          media: file.media === "audio" ? "audio" : "video",
          start: startSecs || null,
        }
      : preview
        ? {
            url: preview.details.url,
            kind: preview.details.kind,
            title: title || preview.details.title || preview.details.site,
            site: preview.details.site,
            author: preview.details.author,
            duration: preview.details.duration,
            start: startSecs || null,
          }
        : null;

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && !busy && onClose()}
      title="Add a link"
      description={`Linked to ${where}. Details are fetched once, now; after that the link works offline.`}
      className="w-[540px]"
    >
      <div role="tablist" className="flex gap-1 rounded-lg bg-muted p-1">
        {(
          [
            ["web", "Web page or video", Globe],
            ["file", "File on this computer", FileVideo],
          ] as const
        ).map(([id, label, Icon]) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={mode === id}
            onClick={() => {
              setMode(id);
              setError(null);
            }}
            className={cn(
              "flex flex-1 items-center justify-center gap-1.5 rounded-md py-1.5 text-[12.5px] font-medium [&_svg]:size-3.5",
              mode === id
                ? "bg-background shadow-sm"
                : "text-muted-foreground hover:text-foreground",
            )}
          >
            <Icon /> {label}
          </button>
        ))}
      </div>

      {mode === "web" ? (
        <div className="flex gap-2">
          <Input
            autoFocus
            aria-label="Address"
            placeholder="Paste an address: a page, a YouTube or Vimeo video, a libreri:// link"
            value={address}
            onChange={(e) => {
              setAddress(e.target.value);
              setPreview(null);
            }}
            onPaste={(e) => {
              const text = e.clipboardData.getData("text").trim();
              if (text && !address.trim()) {
                e.preventDefault();
                setAddress(text);
                setPreview(null);
                setTimeout(() => document.getElementById("lb-link-fetch")?.click(), 0);
              }
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                e.preventDefault();
                void getDetails();
              }
            }}
          />
          {!isBook && (
            <Button
              id="lb-link-fetch"
              variant="outline"
              disabled={!address.trim() || busy !== null}
              onClick={() => void getDetails()}
            >
              {busy === "fetching" ? <Loader2 className="animate-spin" /> : null} Get details
            </Button>
          )}
        </div>
      ) : (
        <div className="flex items-center gap-2">
          <Button variant="outline" onClick={() => void chooseFile()}>
            <FileVideo /> Choose a video or recording…
          </Button>
          {file && (
            <p
              className="min-w-0 flex-1 truncate text-[12px] text-muted-foreground"
              title={file.file}
            >
              {file.inLibrary
                ? "In the library, so the link travels with it."
                : "Outside the library: the link works on this computer only."}
            </p>
          )}
        </div>
      )}

      {error && (
        <p className="text-[12.5px] text-destructive">
          {error}
          {mode === "web" &&
            address.trim() &&
            !isBook &&
            " You can still save the address with a title of your own."}
        </p>
      )}

      {shown && (
        <LinkCard
          link={shown}
          pictureUrl={mode === "web" ? preview?.picture : null}
          className="rounded-lg border p-2"
        />
      )}

      {(shown || isBook || (error && mode === "web")) && (
        <div className="grid grid-cols-[auto_1fr] items-center gap-x-3 gap-y-2">
          <label htmlFor="lb-link-title" className="text-[12.5px] text-muted-foreground">
            Title
          </label>
          <Input id="lb-link-title" value={title} onChange={(e) => setTitle(e.target.value)} />
          {video && (
            <>
              <label htmlFor="lb-link-start" className="text-[12.5px] text-muted-foreground">
                Start at
              </label>
              <div className="flex items-center gap-2">
                <Input
                  id="lb-link-start"
                  className={cn("w-28 tabular-nums", badStart && "border-destructive")}
                  placeholder="0:00"
                  value={start}
                  onChange={(e) => setStart(e.target.value)}
                />
                <span className="text-[12px] text-muted-foreground">
                  {badStart ? "Write it as 1:30 or 1:02:03" : "minutes:seconds, or h:mm:ss"}
                </span>
              </div>
            </>
          )}
        </div>
      )}

      {preview?.canCopy && (
        <label className="flex items-start gap-2 text-[13px]">
          <input
            type="checkbox"
            className="mt-0.5"
            checked={keepCopy}
            onChange={(e) => setKeepCopy(e.target.checked)}
          />
          <span>
            Keep an offline copy
            <span className="block text-[12px] text-muted-foreground">
              The readable part of the page and its pictures, saved in your notes, so it can be read
              even if the page changes or goes away.
            </span>
          </span>
        </label>
      )}

      {(shown || isBook || error) && (
        <SpellTextarea
          bookId={bookId}
          rows={2}
          aria-label="Note"
          placeholder="Why this link? (optional)"
          value={note}
          onChange={(e) => setNote(e.target.value)}
        />
      )}

      <div className="flex justify-end gap-2">
        <Button variant="ghost" disabled={!!busy} onClick={onClose}>
          Cancel
        </Button>
        <Button disabled={!canSave} onClick={() => void save()}>
          {busy === "saving" && <Loader2 className="animate-spin" />}
          {busy === "saving" && preview?.canCopy && keepCopy ? "Saving a copy…" : "Add link"}
        </Button>
      </div>
    </Dialog>
  );
}
