import { ExternalLink, Globe } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { bookUrl } from "@/lib/ipc";
import { openLinkFile, openWeb } from "./api";

const fail = (e: unknown) => toast.error(String((e as Error).message ?? e));

/** A web page's offline copy, in a frame where nothing can run. */
export function CopyViewer({
  copy,
  url,
  title,
  onClose,
}: {
  copy: string | null;
  url: string;
  title: string;
  onClose: () => void;
}) {
  return (
    <Dialog
      open={!!copy}
      onOpenChange={(o) => !o && onClose()}
      title={title || "Offline copy"}
      description={`Offline copy of ${url}`}
      className="h-[88vh] w-[min(900px,calc(100vw-48px))] max-h-[92vh]"
    >
      {copy && (
        <iframe
          title={`Offline copy of ${title}`}
          src={bookUrl(copy)}
          sandbox=""
          className="min-h-0 w-full flex-1 rounded-md border bg-white"
        />
      )}
      <div className="flex justify-end gap-2">
        <Button variant="outline" size="sm" onClick={() => copy && openLinkFile(copy).catch(fail)}>
          <ExternalLink /> Open the copy in a browser
        </Button>
        <Button size="sm" onClick={() => openWeb(url).catch(fail)}>
          <Globe /> Open the page online
        </Button>
      </div>
    </Dialog>
  );
}
