import { useEffect, useRef, useState } from "react";
import { open as pickFile } from "@tauri-apps/plugin-dialog";
import { Camera, CheckCircle2, ImageUp, Loader2, Smartphone } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { commands, events, unwrap, type PhonePairingDto, type ScannedDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";

type Way = "camera" | "phone" | "picture";

const WAYS: { id: Way; label: string; Icon: typeof Camera }[] = [
  { id: "camera", label: "Camera", Icon: Camera },
  { id: "phone", label: "Phone", Icon: Smartphone },
  { id: "picture", label: "Picture", Icon: ImageUp },
];

/**
 * Scans a book's barcode with the computer's camera, a phone on the same
 * network, or a picture file. Calls `onScanned` with the ISBN-13.
 */
export function ScanDialog({
  open,
  onOpenChange,
  onScanned,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onScanned: (isbn13: string) => void;
}) {
  const [way, setWay] = useState<Way>("camera");
  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Scan a barcode"
      description="Hold the barcode on the back of the book up to the camera, or use your phone."
      className="w-[560px]"
    >
      <div role="tablist" aria-label="How to scan" className="flex gap-1 rounded-lg bg-muted p-1">
        {WAYS.map(({ id, label, Icon }) => (
          <button
            key={id}
            type="button"
            role="tab"
            aria-selected={way === id}
            onClick={() => setWay(id)}
            className={cn(
              "flex flex-1 items-center justify-center gap-1.5 rounded-md py-1.5 text-[13px] font-medium text-muted-foreground",
              way === id && "bg-background text-foreground shadow-sm",
            )}
          >
            <Icon className="size-4" /> {label}
          </button>
        ))}
      </div>
      {open && way === "camera" && <CameraScan onScanned={onScanned} />}
      {open && way === "phone" && <PhoneScan onScanned={onScanned} />}
      {open && way === "picture" && <PictureScan onScanned={onScanned} />}
    </Dialog>
  );
}

/** "Not an ISBN" message for codes that are not books. */
function useResult(onScanned: (isbn: string) => void) {
  const [note, setNote] = useState<string | null>(null);
  const take = (s: ScannedDto | null) => {
    if (!s) return false;
    if (s.isbn13) {
      onScanned(s.isbn13);
      return true;
    }
    setNote(`Read ${s.code}, but that is not an ISBN. Look for the barcode starting 978 or 979.`);
    return false;
  };
  return { note, setNote, take };
}

function CameraScan({ onScanned }: { onScanned: (isbn: string) => void }) {
  const video = useRef<HTMLVideoElement>(null);
  const [state, setState] = useState<"starting" | "live" | "failed">("starting");
  const [error, setError] = useState("");
  const { note, take } = useResult(onScanned);
  const takeRef = useRef(take);
  useEffect(() => {
    takeRef.current = take;
  });

  useEffect(() => {
    let stream: MediaStream | null = null;
    let timer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;
    const canvas = document.createElement("canvas");

    // One frame at a time: the next is taken when Rust has read the last.
    const tick = async () => {
      const v = video.current;
      if (stopped || !v || v.videoWidth === 0) {
        timer = setTimeout(() => void tick(), 300);
        return;
      }
      const scale = Math.min(1, 1280 / Math.max(v.videoWidth, v.videoHeight));
      canvas.width = Math.round(v.videoWidth * scale);
      canvas.height = Math.round(v.videoHeight * scale);
      canvas.getContext("2d")?.drawImage(v, 0, 0, canvas.width, canvas.height);
      const found = await unwrap(commands.scanPicture(canvas.toDataURL("image/jpeg", 0.85))).catch(
        () => null,
      );
      if (stopped) return;
      if (!takeRef.current(found)) timer = setTimeout(() => void tick(), 250);
    };

    const start = async () => {
      try {
        if (!navigator.mediaDevices?.getUserMedia) throw new Error("unsupported");
        stream = await navigator.mediaDevices.getUserMedia({
          video: { facingMode: "environment", width: { ideal: 1280 }, height: { ideal: 720 } },
          audio: false,
        });
        if (stopped) return stream.getTracks().forEach((t) => t.stop());
        if (video.current) {
          video.current.srcObject = stream;
          await video.current.play().catch(() => {});
        }
        setState("live");
        void tick();
      } catch (e) {
        setState("failed");
        setError(
          e instanceof DOMException && e.name === "NotAllowedError"
            ? "Libreri is not allowed to use the camera. Allow it in your system settings, or use your phone or a picture instead."
            : "No camera is available here. Use your phone or a picture instead.",
        );
      }
    };
    void start();
    return () => {
      stopped = true;
      clearTimeout(timer);
      stream?.getTracks().forEach((t) => t.stop());
    };
  }, []);

  return (
    <div className="flex flex-col gap-3">
      <div className="relative aspect-video overflow-hidden rounded-lg bg-black">
        <video ref={video} muted playsInline className="size-full object-cover" />
        {state === "live" && (
          <div className="pointer-events-none absolute inset-x-[15%] top-1/2 h-[30%] -translate-y-1/2 rounded-md border-2 border-white/80" />
        )}
        {state === "starting" && (
          <div className="absolute inset-0 flex items-center justify-center gap-2 text-white/80">
            <Loader2 className="size-4 animate-spin" /> Starting the camera…
          </div>
        )}
        {state === "failed" && (
          <div className="absolute inset-0 flex items-center justify-center p-6 text-center text-white/90">
            {error}
          </div>
        )}
      </div>
      <p className="text-[12.5px] text-muted-foreground">
        {note ??
          (state === "live"
            ? "Fill the frame with the barcode and hold still. It is read as soon as it is sharp."
            : "The picture never leaves this computer.")}
      </p>
    </div>
  );
}

function PhoneScan({ onScanned }: { onScanned: (isbn: string) => void }) {
  const [pairing, setPairing] = useState<PhonePairingDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [connected, setConnected] = useState(false);
  const { note, take } = useResult(onScanned);
  const takeRef = useRef(take);
  useEffect(() => {
    takeRef.current = take;
  });

  useEffect(() => {
    let live = true;
    const off = events.phoneScan.listen(({ payload }) => {
      if (payload.kind === "opened") setConnected(true);
      else takeRef.current(payload.scanned);
    });
    unwrap(commands.startPhoneScan())
      .then((p) => live && setPairing(p))
      .catch((e: Error) => live && setError(e.message));
    return () => {
      live = false;
      void off.then((f) => f());
      void commands.stopPhoneScan();
    };
  }, []);

  if (error) return <p className="text-destructive">{error}</p>;
  if (!pairing)
    return (
      <p className="flex items-center gap-2 py-8 text-muted-foreground">
        <Loader2 className="size-4 animate-spin" /> Preparing…
      </p>
    );
  return (
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
          <li>Take a photo of the barcode. Its details appear here.</li>
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
              <CheckCircle2 className="size-3.5" /> Phone connected. Waiting for a photo…
            </>
          ) : (
            <>
              <Loader2 className="size-3.5 animate-spin" /> Waiting for the phone…
            </>
          )}
        </p>
        {note && <p className="text-[12.5px] text-destructive">{note}</p>}
        <p className="text-[12px] text-muted-foreground">
          The link works for 10 minutes and only while this window is open. If your computer asks
          whether Libreri may accept incoming connections, allow it.
        </p>
      </div>
    </div>
  );
}

function PictureScan({ onScanned }: { onScanned: (isbn: string) => void }) {
  const [busy, setBusy] = useState(false);
  const { note, setNote, take } = useResult(onScanned);
  const choose = async () => {
    const path = await pickFile({
      multiple: false,
      filters: [{ name: "Pictures", extensions: ["jpg", "jpeg", "png", "webp", "gif", "bmp"] }],
    });
    if (typeof path !== "string") return;
    setBusy(true);
    setNote(null);
    try {
      const found = await unwrap(commands.scanPictureFile(path));
      if (!found) setNote("No barcode found in that picture. Try a sharper, closer photo.");
      else take(found);
    } catch (e) {
      setNote(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="flex flex-col items-center gap-3 rounded-lg border border-dashed px-6 py-10 text-center">
      <p className="text-muted-foreground">Choose a photo or screenshot that shows the barcode.</p>
      <Button onClick={() => void choose()} disabled={busy}>
        {busy ? <Loader2 className="animate-spin" /> : <ImageUp />} Choose a picture…
      </Button>
      {note && <p className="text-[12.5px] text-destructive">{note}</p>}
    </div>
  );
}
