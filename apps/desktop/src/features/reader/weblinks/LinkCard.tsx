import { BookOpen, FileAudio, FileVideo, Globe, Play, Video } from "lucide-react";
import { bookUrl } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { formatTime, linkByline, type LinkInfo } from "./model";

function Icon({ link }: { link: LinkInfo }) {
  const Glyph =
    link.kind === "video"
      ? Video
      : link.kind === "book"
        ? BookOpen
        : link.kind === "file"
          ? link.media === "audio"
            ? FileAudio
            : FileVideo
          : Globe;
  return <Glyph className="size-5 text-muted-foreground" aria-hidden />;
}

/** A link's picture (or a plain icon: no site logos), title and details. */
export function LinkCard({
  link,
  compact = false,
  className,
  pictureUrl,
}: {
  link: LinkInfo;
  compact?: boolean;
  className?: string;
  /** A picture not yet saved (a data URL, while adding the link). */
  pictureUrl?: string | null;
}) {
  const src = pictureUrl ?? (link.picture ? bookUrl(link.picture) : null);
  const w = compact ? "w-16" : "w-24";
  return (
    <div className={cn("flex min-w-0 items-start gap-2.5", className)}>
      <div
        className={cn(
          "relative flex aspect-video shrink-0 items-center justify-center overflow-hidden rounded-md border bg-muted",
          w,
        )}
      >
        {src ? (
          <img src={src} alt="" loading="lazy" className="size-full object-cover" />
        ) : (
          <Icon link={link} />
        )}
        {link.start ? (
          <span className="absolute right-0.5 bottom-0.5 flex items-center gap-0.5 rounded bg-black/75 px-1 text-[10px] leading-4 text-white tabular-nums">
            <Play className="size-2.5 fill-current" aria-hidden />
            {formatTime(link.start)}
          </span>
        ) : null}
      </div>
      <div className="min-w-0 flex-1">
        <p className={cn("line-clamp-2 font-medium", compact ? "text-[12.5px]" : "text-[13px]")}>
          {link.title || link.url || link.file}
        </p>
        <p className="truncate text-[11.5px] text-muted-foreground">{linkByline(link)}</p>
      </div>
    </div>
  );
}
