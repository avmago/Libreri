import { useState } from "react";
import { Check, Copy } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useRecoveryCode } from "../store";

/** Shows the owner's new recovery code once. Mounted once, at the app level. */
export function RecoveryCodeHost() {
  const { code, show } = useRecoveryCode();
  const [copied, setCopied] = useState(false);
  const onClose = () => {
    show(null);
    setCopied(false);
  };
  return (
    <Dialog
      open={code !== null}
      onOpenChange={(o) => !o && onClose()}
      title="Write down your recovery code"
      description="If you forget your PIN, this code sets a new one. It is shown only now. Keep it somewhere safe, away from this computer."
    >
      <p className="rounded-lg border bg-muted py-4 text-center font-mono text-xl tracking-widest select-all">
        {code}
      </p>
      <div className="flex justify-end gap-2">
        <Button
          variant="outline"
          onClick={() => {
            if (code) void navigator.clipboard.writeText(code).then(() => setCopied(true));
          }}
        >
          {copied ? <Check /> : <Copy />} {copied ? "Copied" : "Copy"}
        </Button>
        <Button onClick={onClose}>I have written it down</Button>
      </div>
    </Dialog>
  );
}
