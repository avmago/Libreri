import { useState } from "react";
import { AlertTriangle } from "lucide-react";
import { bookUrl } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { authorsText, coverTone, FILE_TYPE_LABEL, type BookView } from "../model";

/**
 * A book's cover: the stored thumbnail, or a generated cover with the title
 * and author. Generated covers are the one place a serif face appears in
 * the interface (docs/code-structure.md, typography rules).
 */
export function BookCover({
  book,
  large = false,
  className,
}: {
  book: BookView;
  large?: boolean;
  className?: string;
}) {
  const [failed, setFailed] = useState(false);
  const image = large ? book.cover : book.thumbnail;
  const [bg, fg] = coverTone(book.id);

  return (
    <div
      className={cn(
        "relative aspect-[2/3] w-full overflow-hidden rounded-[3px] bg-muted shadow-[0_1px_2px_rgba(0,0,0,0.12),0_4px_12px_-4px_rgba(0,0,0,0.18)]",
        book.missing && "opacity-50 grayscale",
        className,
      )}
    >
      {image && !failed ? (
        <img
          src={`${bookUrl(image)}?v=${encodeURIComponent(book.modifiedAt)}`}
          alt=""
          loading="lazy"
          decoding="async"
          draggable={false}
          onError={() => setFailed(true)}
          className="size-full object-cover"
        />
      ) : (
        <div
          className="flex size-full flex-col justify-between p-[9%] text-left"
          style={{ background: bg, color: fg }}
          aria-hidden
        >
          <div className="flex flex-col gap-[6%]">
            <div className="h-px w-1/3 opacity-40" style={{ background: fg }} />
            <p
              className={cn(
                "line-clamp-5 font-serif leading-[1.15] font-medium break-words",
                large ? "text-2xl" : "text-[clamp(10px,1.1vw,15px)]",
              )}
            >
              {book.metadata.title}
            </p>
          </div>
          <div className="flex items-end justify-between gap-2">
            <p className={cn("line-clamp-2 opacity-75", large ? "text-sm" : "text-[10px]")}>
              {authorsText(book.metadata.authors, 1)}
            </p>
            <span className="shrink-0 rounded-sm border border-current/30 px-1 font-mono text-[9px] opacity-70">
              {FILE_TYPE_LABEL[book.fileType]}
            </span>
          </div>
        </div>
      )}
      {book.missing && (
        <span className="absolute top-1.5 right-1.5 rounded-full bg-background/90 p-1 text-destructive">
          <AlertTriangle className="size-3.5" aria-label="File missing" />
        </span>
      )}
    </div>
  );
}
