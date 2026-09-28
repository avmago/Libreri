import { ExternalLink, FileText, Link2, LocateFixed, Play, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import type { Annotation } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { LinkCard } from "./LinkCard";
import { Player } from "./Player";
import { linkOf, playable, type LinkInfo } from "./model";

export interface LinkActions {
  onPlay: (a: Annotation) => void;
  /** Web pages open in the browser; book links in Libreri. */
  onOpen: (a: Annotation, link: LinkInfo) => void;
  onCopy: (a: Annotation, link: LinkInfo) => void;
  onShow: (a: Annotation) => void;
  onDelete: (a: Annotation) => void;
}

function Row({
  a,
  link,
  playing,
  actions,
}: {
  a: Annotation;
  link: LinkInfo;
  playing: boolean;
  actions: LinkActions;
}) {
  return (
    <li
      className={cn(
        "group rounded-md px-2 py-1.5 hover:bg-sidebar-accent",
        playing && "bg-sidebar-accent",
      )}
    >
      <button
        type="button"
        className="w-full text-left"
        onClick={() => (playable(link) ? actions.onPlay(a) : actions.onOpen(a, link))}
        title={playable(link) ? "Play" : link.url}
      >
        <LinkCard link={link} compact />
      </button>
      {(a.note || a.quote?.exact) && (
        <p className="mt-1 line-clamp-2 text-[11.5px] text-muted-foreground">
          {a.note ?? `“${a.quote?.exact}”`}
        </p>
      )}
      <div className="mt-1 flex items-center gap-0.5 text-muted-foreground [&_button]:size-7 [&_svg]:size-3.5">
        <span className="flex-1 truncate text-[11px]">{a.label}</span>
        {playable(link) ? (
          <Button variant="ghost" size="icon" aria-label="Play" onClick={() => actions.onPlay(a)}>
            <Play />
          </Button>
        ) : (
          <Button
            variant="ghost"
            size="icon"
            aria-label={link.kind === "book" ? "Open the book" : "Open in browser"}
            onClick={() => actions.onOpen(a, link)}
          >
            <ExternalLink />
          </Button>
        )}
        {link.copy && (
          <Button
            variant="ghost"
            size="icon"
            aria-label="Read the offline copy"
            title="Read the offline copy"
            onClick={() => actions.onCopy(a, link)}
          >
            <FileText />
          </Button>
        )}
        <Button
          variant="ghost"
          size="icon"
          aria-label="Show in the book"
          title="Show in the book"
          onClick={() => actions.onShow(a)}
        >
          <LocateFixed />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Delete link"
          className="hover:text-destructive"
          onClick={() => actions.onDelete(a)}
        >
          <Trash2 />
        </Button>
      </div>
    </li>
  );
}

/**
 * The reader's Links tab: the mini player, then this page's links and the
 * rest of the book's.
 */
export function LinksPanel({
  links,
  page,
  playing,
  onStop,
  onAdd,
  ...actions
}: LinkActions & {
  links: Annotation[];
  /** The page shown (page-based books), to list its links first. */
  page: number | null;
  playing: Annotation | null;
  onStop: () => void;
  onAdd: () => void;
}) {
  const withLinks = links
    .map((a) => ({ a, link: linkOf(a.locator) }))
    .filter((x): x is { a: Annotation; link: LinkInfo } => !!x.link);
  const pageOf = (a: Annotation) => {
    try {
      return (JSON.parse(a.locator) as { page?: number }).page ?? null;
    } catch {
      return null;
    }
  };
  const here = page === null ? [] : withLinks.filter((x) => pageOf(x.a) === page);
  const rest = withLinks.filter((x) => !here.includes(x));
  const playingLink = playing ? linkOf(playing.locator) : null;
  const list = (items: typeof withLinks) => (
    <ul className="flex flex-col gap-0.5">
      {items.map(({ a, link }) => (
        <Row key={a.id} a={a} link={link} playing={playing?.id === a.id} actions={actions} />
      ))}
    </ul>
  );
  return (
    <div className="flex min-h-0 flex-1 flex-col">
      {playing && playingLink && (
        <Player key={playing.id + (playingLink.start ?? "")} link={playingLink} onClose={onStop} />
      )}
      <div className="min-h-0 flex-1 overflow-y-auto px-1.5 py-2">
        <Button
          variant="outline"
          size="sm"
          className="mx-1 mb-2 w-[calc(100%-8px)]"
          onClick={onAdd}
        >
          <Plus /> Add a link here
        </Button>
        {withLinks.length === 0 ? (
          <div className="flex flex-col items-center gap-2 px-4 py-6 text-center text-muted-foreground">
            <Link2 className="size-5" aria-hidden />
            <p>
              Link web pages, videos (with a start time) and recordings to a passage or a page.
              Select text and choose the link button, or add one to the page.
            </p>
          </div>
        ) : (
          <div className="flex flex-col gap-3">
            {here.length > 0 && (
              <section>
                <h3 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                  ON THIS PAGE
                </h3>
                {list(here)}
              </section>
            )}
            {rest.length > 0 && (
              <section>
                <h3 className="px-2.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground">
                  {here.length ? "ELSEWHERE IN THE BOOK" : "IN THIS BOOK"}
                </h3>
                {list(rest)}
              </section>
            )}
          </div>
        )}
      </div>
    </div>
  );
}
