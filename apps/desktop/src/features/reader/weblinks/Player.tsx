import { useEffect, useRef, useState } from "react";
import { ExternalLink, Loader2, PictureInPicture2, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { bookUrl, type PlayerDto } from "@/lib/ipc";
import { mediaUrl, openLinkFile, openWeb, playerFor, popOut } from "./api";
import type { LinkInfo } from "./model";

const fail = (e: unknown) => toast.error(String((e as Error).message ?? e));

/**
 * The side panel's mini player: a site's own player (through Libreri's
 * local player page, YouTube in its privacy-enhanced form), or a video or
 * recording on this computer. Starts at the link's start time.
 */
export function Player({ link, onClose }: { link: LinkInfo; onClose: () => void }) {
  const [site, setSite] = useState<PlayerDto | null>(null);
  const [file, setFile] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const media = useRef<HTMLVideoElement & HTMLAudioElement>(null);

  useEffect(() => {
    let live = true;
    const done = (e: unknown) => live && setError(String((e as Error).message ?? e));
    if (link.kind === "file" && link.file)
      mediaUrl(link.file).then((p) => live && setFile(bookUrl(p)), done);
    else
      playerFor(link.url, link.video ?? null, link.embed ?? null, link.start ?? null).then(
        (p) => live && setSite(p),
        done,
      );
    return () => {
      live = false;
    };
  }, [link]);

  const startAt = () => {
    if (media.current && link.start) media.current.currentTime = link.start;
  };

  return (
    <div className="flex flex-col gap-1.5 border-b bg-background p-2">
      {error ? (
        <p className="rounded-md bg-muted p-3 text-[12.5px] text-destructive">{error}</p>
      ) : link.kind === "file" && link.media === "audio" ? (
        file && (
          <audio
            ref={media}
            src={file}
            controls
            autoPlay
            onLoadedMetadata={startAt}
            className="w-full"
            aria-label={link.title}
          />
        )
      ) : (
        <div className="relative aspect-video w-full overflow-hidden rounded-md bg-black">
          {link.kind === "file" && file ? (
            <video
              ref={media}
              src={file}
              controls
              autoPlay
              onLoadedMetadata={startAt}
              className="size-full"
              aria-label={link.title}
            />
          ) : site ? (
            <iframe
              title={link.title || "Video"}
              src={site.page}
              allow="autoplay; encrypted-media; fullscreen; picture-in-picture"
              allowFullScreen
              referrerPolicy="strict-origin-when-cross-origin"
              className="size-full border-0"
            />
          ) : (
            <Loader2 className="absolute top-1/2 left-1/2 size-5 -translate-1/2 animate-spin text-white/70" />
          )}
        </div>
      )}
      <div className="flex items-center gap-0.5">
        <p className="min-w-0 flex-1 truncate px-1 text-[12px] font-medium">{link.title}</p>
        {site && (
          <>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Play in its own window"
              title="Play in its own window"
              onClick={() => popOut(site.page, link.title).catch(fail)}
            >
              <PictureInPicture2 />
            </Button>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Open in browser"
              title="Open in browser"
              onClick={() => openWeb(site.watch).catch(fail)}
            >
              <ExternalLink />
            </Button>
          </>
        )}
        {link.kind === "file" && link.file && (
          <Button
            variant="ghost"
            size="icon"
            aria-label="Open in another app"
            title="Open in another app"
            onClick={() => openLinkFile(link.file!).catch(fail)}
          >
            <ExternalLink />
          </Button>
        )}
        <Button variant="ghost" size="icon" aria-label="Close the player" onClick={onClose}>
          <X />
        </Button>
      </div>
    </div>
  );
}
