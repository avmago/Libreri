import { useEffect, useRef, useState } from "react";
import { CheckCircle2, ExternalLink, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { commands, events, type HelperInfo, type HelpersDto } from "@/lib/ipc";
import { useHelpers, useInstallHelper } from "../api";
import { useHelperDialog } from "../store";

const openUrl = (url: string) =>
  void commands
    .openExternalUrl(url)
    .then((r) => r.status === "error" && toast.error(r.error.message));

/** How to get a package manager when Libreri finds none. */
function NoInstaller({ data }: { data: HelpersDto }) {
  if (data.platform === "macos")
    return (
      <div className="flex flex-col gap-2 text-[13px]">
        <p>
          Libreri installs helpers with <strong>Homebrew</strong>, the usual way to add programs
          like this on a Mac. It is not installed yet.
        </p>
        <ol className="list-decimal pl-5 text-muted-foreground">
          <li>Open brew.sh and copy the install command.</li>
          <li>Paste it into Terminal and follow the steps (it asks for your Mac password).</li>
          <li>Come back here and press Check again.</li>
        </ol>
        <Button
          variant="outline"
          size="sm"
          className="self-start"
          onClick={() => openUrl("https://brew.sh")}
        >
          <ExternalLink /> Open brew.sh
        </Button>
      </div>
    );
  if (data.platform === "windows")
    return (
      <p className="text-[13px]">
        Libreri installs helpers with <strong>winget</strong>, which comes with App Installer from
        the Microsoft Store. Install or update App Installer, then press Check again.
      </p>
    );
  return (
    <p className="text-[13px]">
      Libreri did not find apt, dnf, pacman or zypper. Install the helper with your system&apos;s
      software centre, then press Check again.
    </p>
  );
}

function Body({ info, data, onDone }: { info: HelperInfo; data: HelpersDto; onDone: () => void }) {
  const install = useInstallHelper();
  const { refetch, isFetching } = useHelpers();
  const onInstalled = useHelperDialog((s) => s.onInstalled);
  const [lines, setLines] = useState<string[]>([]);
  const logRef = useRef<HTMLPreElement>(null);

  useEffect(() => {
    const off = events.helperInstall.listen(({ payload }) => {
      if (payload.helper !== info.helper || !payload.line) return;
      setLines((l) => [...l.slice(-300), payload.line!]);
    });
    return () => void off.then((f) => f());
  }, [info.helper]);
  useEffect(() => {
    logRef.current?.scrollTo({ top: logRef.current.scrollHeight });
  }, [lines]);

  if (info.installed)
    return (
      <div className="flex flex-col gap-3">
        <p className="flex items-center gap-2">
          <CheckCircle2 className="size-4 text-green-600" aria-hidden />
          {info.name} {info.version ?? ""} is installed.
        </p>
        <p className="font-mono text-[11.5px] break-all text-muted-foreground">{info.path}</p>
        <div className="flex justify-end">
          <Button
            onClick={() => {
              onInstalled?.();
              onDone();
            }}
          >
            Done
          </Button>
        </div>
      </div>
    );

  return (
    <div className="flex flex-col gap-3">
      <p>{info.purpose}</p>
      {info.plan ? (
        <>
          <p className="text-[13px] text-muted-foreground">
            Libreri will run this with{" "}
            {data.installer ? <strong>{installerName(data)}</strong> : null}
            {info.plan.needsAdmin ? "; your computer will ask for your password" : ""}:
          </p>
          <code className="rounded-md bg-muted px-2.5 py-1.5 font-mono text-[12px]">
            {info.plan.display}
          </code>
        </>
      ) : (
        <NoInstaller data={data} />
      )}
      {(install.isPending || lines.length > 0) && (
        <pre
          ref={logRef}
          className="max-h-40 overflow-auto rounded-md bg-muted px-2.5 py-2 font-mono text-[11px] whitespace-pre-wrap"
        >
          {lines.join("\n") || "Starting…"}
        </pre>
      )}
      {install.error && <p className="text-destructive">{install.error.message}</p>}
      <p className="text-[12px] text-muted-foreground">
        {info.name} is open-source software ({info.licence}) that Libreri runs as a separate
        program.
      </p>
      <div className="flex justify-end gap-2">
        <Button variant="ghost" onClick={onDone}>
          Close
        </Button>
        {!info.plan && (
          <Button variant="outline" disabled={isFetching} onClick={() => void refetch()}>
            Check again
          </Button>
        )}
        {info.plan && (
          <Button
            disabled={install.isPending}
            onClick={() =>
              install.mutate(info.helper, {
                onSuccess: () => {
                  toast.success(`${info.name} is installed`);
                  onInstalled?.();
                },
              })
            }
          >
            {install.isPending && <Loader2 className="animate-spin" />}
            {install.isPending ? "Installing…" : "Install"}
          </Button>
        )}
      </div>
    </div>
  );
}

function installerName(data: HelpersDto): string {
  const names: Record<string, string> = {
    homebrew: "Homebrew",
    winget: "winget",
    apt: "apt",
    dnf: "dnf",
    pacman: "pacman",
    zypper: "zypper",
  };
  return data.installer ? (names[data.installer] ?? data.installer) : "";
}

/** Installs a helper program with the computer's package manager. */
export function HelperDialog() {
  const helper = useHelperDialog((s) => s.helper);
  const close = useHelperDialog((s) => s.close);
  const { data } = useHelpers();
  const info = data?.helpers.find((h) => h.helper === helper);
  return (
    <Dialog
      open={helper !== null}
      onOpenChange={(o) => !o && close()}
      title={info ? `Install ${info.name}` : "Helper program"}
      className="w-[520px]"
    >
      {info && data ? (
        <Body key={info.helper} info={info} data={data} onDone={close} />
      ) : (
        <p className="text-muted-foreground">Checking…</p>
      )}
    </Dialog>
  );
}
