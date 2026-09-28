import { useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { ask, open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Camera, CheckCircle2, History, Loader2, RotateCcw, Save, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import {
  bookUrl,
  commands,
  events,
  unwrap,
  type BookDto,
  type PhonePairingDto,
  type Quality,
  type VersionDto,
} from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { parseRanges, type OtherFile } from "./model";
import { PdfPages } from "./pdfPages";

const inputClass =
  "h-8 min-w-0 rounded-md border border-input bg-background px-2 outline-none focus-visible:border-ring";

function formatSize(bytes: number): string {
  if (bytes < 1024 * 1024) return `${Math.max(1, Math.round(bytes / 1024))} KB`;
  return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
}

// ---------- pages from another PDF ----------

/** Chooses another PDF (a file or a book in the library) and which pages. */
export function PdfPagesDialog({
  open,
  source,
  onClose,
  onPick,
}: {
  open: boolean;
  /** "file": a PDF on this computer; "book": one in the library. */
  source: "file" | "book";
  onClose: () => void;
  onPick: (file: OtherFile, pages: number[]) => void;
}) {
  const [file, setFile] = useState<OtherFile | null>(null);
  const [ranges, setRanges] = useState("");
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  const { data: books = [] } = useQuery({
    queryKey: ["lib", "edit-pdf-books", search],
    queryFn: () =>
      unwrap(commands.listBooks({ search: search || null, fileTypes: ["pdf"], sort: "title" })),
    enabled: open && source === "book",
  });

  const chooseFile = async () => {
    const path = await openDialog({
      multiple: false,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (typeof path !== "string") return;
    setBusy(true);
    try {
      const pages = await unwrap(commands.pdfPageCount(path));
      setFile({ ref: path, name: path.split(/[\\/]/).pop() ?? path, pages });
    } catch (e) {
      toast.error("That PDF could not be read", { description: String(e) });
    } finally {
      setBusy(false);
    }
  };
  const chooseBook = async (b: BookDto) => {
    setBusy(true);
    try {
      const url = bookUrl(b.relPath ?? "");
      const doc = await PdfPages.open(url);
      const pages = doc.count;
      doc.close();
      setFile({ ref: `book:${b.id}`, name: b.metadata.title ?? "Book", pages, url });
    } catch (e) {
      toast.error("That PDF could not be read", { description: String(e) });
    } finally {
      setBusy(false);
    }
  };

  const pages = file ? (ranges.trim() ? parseRanges(ranges, file.pages || 9999) : null) : null;
  const all = file && !ranges.trim();

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={source === "file" ? "Add pages from a PDF" : "Add pages from a book"}
      description="The pages are added after the selected page."
      className="w-[520px]"
    >
      {!file && source === "file" && (
        <Button onClick={() => void chooseFile()} disabled={busy} className="self-start">
          {busy && <Loader2 className="animate-spin" />} Choose a PDF…
        </Button>
      )}
      {!file && source === "book" && (
        <div className="flex flex-col gap-2">
          <input
            autoFocus
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="Find a PDF in the library"
            className={inputClass}
          />
          <ul className="flex max-h-72 flex-col overflow-auto rounded-md border">
            {books.map((b) => (
              <li key={b.id}>
                <button
                  type="button"
                  className="flex w-full flex-col items-start px-3 py-1.5 text-left hover:bg-muted"
                  onClick={() => void chooseBook(b)}
                >
                  <span className="font-medium">{b.metadata.title}</span>
                  <span className="text-[12px] text-muted-foreground">
                    {(b.metadata.authors ?? []).join(", ")}
                  </span>
                </button>
              </li>
            ))}
            {!books.length && <li className="px-3 py-2 text-muted-foreground">No PDFs found.</li>}
          </ul>
        </div>
      )}
      {file && (
        <div className="flex flex-col gap-2">
          <p>
            <span className="font-medium">{file.name}</span>
            {file.pages ? (
              <span className="text-muted-foreground"> · {file.pages} pages</span>
            ) : null}
          </p>
          <label className="flex flex-col gap-1">
            <span>Pages (leave empty for all)</span>
            <input
              autoFocus
              value={ranges}
              onChange={(e) => setRanges(e.target.value)}
              placeholder="e.g. 1-3, 7"
              className={inputClass}
            />
          </label>
          {pages && !pages.length && (
            <p className="text-[12.5px] text-destructive">Write pages like 1-3, 7.</p>
          )}
        </div>
      )}
      <div className="flex justify-end gap-2">
        {file && (
          <Button variant="ghost" onClick={() => setFile(null)}>
            Back
          </Button>
        )}
        <Button variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button
          disabled={!file || (!all && !pages?.length) || Boolean(all && !file.pages)}
          onClick={() => {
            if (!file) return;
            const list = all ? Array.from({ length: file.pages }, (_, i) => i + 1) : pages!;
            onPick(file, list);
          }}
        >
          Add pages
        </Button>
      </div>
    </Dialog>
  );
}

// ---------- camera ----------

/** Photographs paper pages with the computer's camera. */
export function CameraPagesDialog({
  open,
  onClose,
  onPhoto,
}: {
  open: boolean;
  onClose: () => void;
  onPhoto: (src: string) => void;
}) {
  const video = useRef<HTMLVideoElement>(null);
  const [state, setState] = useState<"starting" | "live" | "failed">("starting");
  const [taken, setTaken] = useState(0);
  const [flash, setFlash] = useState(false);

  useEffect(() => {
    if (!open) return;
    let stream: MediaStream | null = null;
    let stopped = false;
    void (async () => {
      try {
        if (!navigator.mediaDevices?.getUserMedia) throw new Error("unsupported");
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "environment", width: { ideal: 2560 }, height: { ideal: 1440 } },
          audio: false,
        });
        if (stopped) return stream.getTracks().forEach((t) => t.stop());
        if (video.current) {
          video.current.srcObject = stream;
          await video.current.play().catch(() => {});
        }
        setState("live");
      } catch {
        setState("failed");
      }
    })();
    return () => {
      stopped = true;
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, [open]);

  const take = () => {
    const v = video.current;
    if (!v || !v.videoWidth) return;
    const canvas = document.createElement("canvas");
    canvas.width = v.videoWidth;
    canvas.height = v.videoHeight;
    canvas.getContext("2d")?.drawImage(v, 0, 0);
    onPhoto(canvas.toDataURL("image/jpeg", 0.88));
    setTaken((n) => n + 1);
    setFlash(true);
    setTimeout(() => setFlash(false), 150);
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Add pages with the camera"
      description="Hold each page flat under the camera, in good light, and take a photo. Every photo becomes a page."
      className="w-[640px]"
    >
      <div className="relative aspect-video overflow-hidden rounded-lg bg-black">
        <video ref={video} muted playsInline className="size-full object-contain" />
        {flash && <div className="absolute inset-0 bg-white/70" />}
        {state !== "live" && (
          <div className="absolute inset-0 flex items-center justify-center p-6 text-center text-white/90">
            {state === "starting" ? (
              <>
                <Loader2 className="mr-2 size-4 animate-spin" /> Starting the camera…
              </>
            ) : (
              "No camera is available, or Libreri may not use it. Use your phone or a picture instead."
            )}
          </div>
        )}
      </div>
      <div className="flex items-center justify-between">
        <span className="text-muted-foreground">
          {taken
            ? `${taken} ${taken === 1 ? "page" : "pages"} added`
            : "The photos stay on this computer."}
        </span>
        <div className="flex gap-2">
          <Button variant="ghost" onClick={onClose}>
            Done
          </Button>
          <Button onClick={take} disabled={state !== "live"}>
            <Camera /> Take photo
          </Button>
        </div>
      </div>
    </Dialog>
  );
}

// ---------- phone ----------

/** Photographs paper pages with a phone on the same network. */
export function PhonePagesDialog({
  open,
  onClose,
  onPhoto,
}: {
  open: boolean;
  onClose: () => void;
  onPhoto: (src: string) => void;
}) {
  const [pairing, setPairing] = useState<PhonePairingDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  const [count, setCount] = useState(0);
  const photoRef = useRef(onPhoto);
  useEffect(() => {
    photoRef.current = onPhoto;
  });

  useEffect(() => {
    if (!open) return;
    let live = true;
    const off = events.phonePage.listen(({ payload }) => {
      if (payload.kind === "opened") setConnected(true);
      else if (payload.picture) {
        photoRef.current(payload.picture);
        setCount((n) => n + 1);
      }
    });
    unwrap(commands.startPhonePages())
      .then((p) => live && setPairing(p))
      .catch((e: Error) => live && setError(e.message));
    return () => {
      live = false;
      void off.then((f) => f());
      void commands.stopPhoneScan();
    };
  }, [open]);

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Add pages with your phone"
      description="Photos taken on the phone arrive here as new pages."
      className="w-[560px]"
    >
      {error && <p className="text-destructive">{error}</p>}
      {!error && !pairing && (
        <p className="flex items-center gap-2 py-8 text-muted-foreground">
          <Loader2 className="size-4 animate-spin" /> Preparing…
        </p>
      )}
      {pairing && (
        <div className="flex gap-5">
          <div
            className="size-[200px] shrink-0 rounded-lg bg-white p-2 [&_svg]:size-full"
            role="img"
            aria-label="QR code with the address to open on your phone"
            // The SVG is drawn by Libreri (the qrcode crate), not taken from anywhere.
            dangerouslySetInnerHTML={{ __html: pairing.qrSvg }}
          />
          <div className="flex flex-col gap-2">
            <ol className="flex list-decimal flex-col gap-1.5 pl-4">
              <li>Connect your phone to the same Wi‑Fi as this computer.</li>
              <li>Point the phone’s camera at the code and open the link.</li>
              <li>Photograph each page, flat and straight on.</li>
            </ol>
            <p className="font-mono text-[11.5px] break-all text-muted-foreground">{pairing.url}</p>
            <p
              className={cn(
                "flex items-center gap-1.5 text-[12.5px]",
                connected ? "text-foreground" : "text-muted-foreground",
              )}
            >
              {connected ? (
                <>
                  <CheckCircle2 className="size-3.5" />
                  {count
                    ? `${count} ${count === 1 ? "page" : "pages"} added`
                    : "Phone connected. Waiting for photos…"}
                </>
              ) : (
                <>
                  <Loader2 className="size-3.5 animate-spin" /> Waiting for the phone…
                </>
              )}
            </p>
            <p className="text-[12px] text-muted-foreground">
              The link works for 20 minutes and only while this window is open.
            </p>
          </div>
        </div>
      )}
      <div className="flex justify-end">
        <Button onClick={onClose}>Done</Button>
      </div>
    </Dialog>
  );
}

// ---------- new books ----------

export type NewBooksRequest =
  | { kind: "all"; name: string }
  | { kind: "selected"; name: string }
  | { kind: "every"; name: string; size: number }
  | { kind: "atSelected"; name: string };

/** "Save as new book", "Extract pages" and "Split". */
export function NewBooksDialog({
  open,
  mode,
  title,
  selected,
  busy,
  onClose,
  onSave,
}: {
  open: boolean;
  mode: "copy" | "extract" | "split";
  title: string;
  selected: number;
  busy: boolean;
  onClose: () => void;
  onSave: (r: NewBooksRequest) => void;
}) {
  const [name, setName] = useState(
    mode === "copy" ? `${title} (edited)` : mode === "extract" ? `${title} (extract)` : title,
  );
  const [split, setSplit] = useState<"every" | "atSelected">(selected > 1 ? "atSelected" : "every");
  const [size, setSize] = useState(10);
  const heading = {
    copy: "Save as a new book",
    extract: "Extract pages as a new book",
    split: "Split into several books",
  }[mode];
  const description = {
    copy: "The changed pages become a new book next to this one. This book stays as it is.",
    extract: `The ${selected} selected ${selected === 1 ? "page becomes" : "pages become"} a new book next to this one, with your other changes. This book stays as it is.`,
    split: "Each part becomes a new book next to this one, numbered. This book stays as it is.",
  }[mode];
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title={heading}
      description={description}
    >
      <label className="flex flex-col gap-1">
        <span>{mode === "split" ? "Names start with" : "Name"}</span>
        <input
          autoFocus
          value={name}
          onChange={(e) => setName(e.target.value)}
          className={inputClass}
        />
      </label>
      {mode === "split" && (
        <div className="flex flex-col gap-2">
          <label className="flex items-center gap-2">
            <input type="radio" checked={split === "every"} onChange={() => setSplit("every")} />
            Every
            <input
              type="number"
              min={1}
              value={size}
              onChange={(e) => setSize(Math.max(1, Number(e.target.value)))}
              className={cn(inputClass, "w-16")}
            />
            pages
          </label>
          <label className="flex items-center gap-2">
            <input
              type="radio"
              checked={split === "atSelected"}
              disabled={selected === 0}
              onChange={() => setSplit("atSelected")}
            />
            A new part starts at each selected page ({selected})
          </label>
        </div>
      )}
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button
          disabled={!name.trim() || busy}
          onClick={() =>
            onSave(
              mode === "copy"
                ? { kind: "all", name }
                : mode === "extract"
                  ? { kind: "selected", name }
                  : split === "every"
                    ? { kind: "every", name, size }
                    : { kind: "atSelected", name },
            )
          }
        >
          {busy && <Loader2 className="animate-spin" />} Save
        </Button>
      </div>
    </Dialog>
  );
}

// ---------- compress ----------

const QUALITIES: { id: Quality; label: string; help: string }[] = [
  { id: "high", label: "A little smaller", help: "Pictures up to 3000 pixels, barely any change." },
  {
    id: "medium",
    label: "Smaller",
    help: "Pictures up to 2000 pixels; fine for reading on screen.",
  },
  { id: "small", label: "Smallest", help: "Pictures up to 1400 pixels; photos may look soft." },
];

export function CompressDialog({
  open,
  value,
  onClose,
  onChoose,
}: {
  open: boolean;
  value: Quality | null;
  onClose: () => void;
  onChoose: (q: Quality | null) => void;
}) {
  const [q, setQ] = useState<Quality | null>(value ?? "medium");
  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Make the PDF smaller"
      description="Large pictures in the PDF are stored smaller. Text and drawings are not changed."
    >
      <div className="flex flex-col gap-2">
        {QUALITIES.map((o) => (
          <label key={o.id} className="flex items-start gap-2 rounded-md border p-2.5">
            <input type="radio" checked={q === o.id} onChange={() => setQ(o.id)} className="mt-1" />
            <span className="flex flex-col">
              <span className="font-medium">{o.label}</span>
              <span className="text-[12.5px] text-muted-foreground">{o.help}</span>
            </span>
          </label>
        ))}
      </div>
      <div className="flex justify-end gap-2">
        {value && (
          <Button variant="ghost" onClick={() => onChoose(null)}>
            Don’t make smaller
          </Button>
        )}
        <Button variant="ghost" onClick={onClose}>
          Cancel
        </Button>
        <Button onClick={() => onChoose(q)}>Use</Button>
      </div>
    </Dialog>
  );
}

// ---------- versions ----------

/** Earlier versions of a book: restore, save a copy, delete. */
export function VersionsDialog({
  open,
  bookId,
  title,
  canEdit,
  onClose,
  onRestored,
}: {
  open: boolean;
  bookId: string;
  title: string;
  canEdit: boolean;
  onClose: () => void;
  onRestored: (book: BookDto) => void;
}) {
  const qc = useQueryClient();
  const key = ["lib", "versions", bookId];
  const { data: versions, isLoading } = useQuery({
    queryKey: key,
    queryFn: () => unwrap(commands.listVersions(bookId)),
    enabled: open,
  });
  const [busy, setBusy] = useState<string | null>(null);
  const when = (s: string) =>
    new Date(s).toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });

  const restore = async (v: VersionDto) => {
    const ok = await ask(
      `The book goes back to how it was before “${v.reason}” (${when(v.savedAt)}). The current file is kept as a version too, so you can come back to it.`,
      { title: "Restore this version?", okLabel: "Restore" },
    );
    if (!ok) return;
    setBusy(v.id);
    try {
      const book = await unwrap(commands.restoreVersion(bookId, v.id));
      toast.success("Earlier version restored");
      onRestored(book);
    } catch (e) {
      toast.error("The version could not be restored", { description: String(e) });
    } finally {
      setBusy(null);
    }
  };
  const copy = async (v: VersionDto) => {
    const clean = title.replace(/[\\/:*?"<>|]+/g, " ").trim();
    const dest = await saveDialog({
      defaultPath: `${clean} (${new Date(v.savedAt).toISOString().slice(0, 10)}).pdf`,
      filters: [{ name: "PDF", extensions: ["pdf"] }],
    });
    if (!dest) return;
    const r = await commands.saveVersionCopy(bookId, v.id, dest);
    if (r.status === "error") toast.error(r.error.message);
    else
      toast.success("Copy saved", {
        description: dest,
        action: { label: "Show", onClick: () => void commands.revealPath(dest) },
      });
  };
  const remove = async (v: VersionDto) => {
    const ok = await ask(`The version from ${when(v.savedAt)} is deleted for good.`, {
      title: "Delete this version?",
      okLabel: "Delete",
      kind: "warning",
    });
    if (!ok) return;
    const r = await commands.deleteVersion(bookId, v.id);
    if (r.status === "error") toast.error(r.error.message);
    void qc.invalidateQueries({ queryKey: key });
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(o) => !o && onClose()}
      title="Version history"
      description="Libreri keeps the file as it was before each change to its pages, a redaction, a filled-in form or markup saved into it."
      className="w-[560px]"
    >
      {isLoading && <p className="text-muted-foreground">Loading…</p>}
      {versions && !versions.length && (
        <p className="flex items-center gap-2 text-muted-foreground">
          <History className="size-4" /> No earlier versions. This is the file as it was added.
        </p>
      )}
      <ul className="flex flex-col divide-y rounded-lg border">
        {versions?.map((v) => (
          <li key={v.id} className="flex items-center gap-3 px-3 py-2">
            <div className="flex min-w-0 flex-1 flex-col">
              <span className="font-medium">Before: {v.reason}</span>
              <span className="text-[12px] text-muted-foreground">
                {when(v.savedAt)} · {formatSize(v.size ?? 0)}
              </span>
            </div>
            <Button
              variant="ghost"
              size="icon"
              aria-label="Save a copy"
              title="Save a copy…"
              onClick={() => void copy(v)}
            >
              <Save />
            </Button>
            {canEdit && (
              <>
                <Button
                  variant="outline"
                  size="sm"
                  disabled={busy !== null}
                  onClick={() => void restore(v)}
                >
                  {busy === v.id ? <Loader2 className="animate-spin" /> : <RotateCcw />} Restore
                </Button>
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label="Delete this version"
                  title="Delete"
                  onClick={() => void remove(v)}
                >
                  <Trash2 />
                </Button>
              </>
            )}
          </li>
        ))}
      </ul>
    </Dialog>
  );
}
