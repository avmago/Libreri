import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useRecoverOwner } from "../api";
import { pinProblem } from "../model";

/** The owner forgot their PIN: enter the recovery code and a new PIN. */
export function RecoveryDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const [code, setCode] = useState("");
  const [pin, setPin] = useState("");
  const recover = useRecoverOwner();
  const problem = pin ? pinProblem(pin) : null;

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Reset the owner's PIN"
      description="Enter the recovery code you wrote down when you set your PIN, then choose a new PIN."
    >
      <form
        className="flex flex-col gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          recover.mutate(
            { code, pin },
            {
              // The new code is shown by the app (see useRecoverOwner).
              onSuccess: () => onOpenChange(false),
            },
          );
        }}
      >
        <label className="flex flex-col gap-1.5">
          <span className="font-medium">Recovery code</span>
          <Input
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder="XXXX-XXXX-XXXX-XXXX"
            className="font-mono uppercase"
            autoFocus
          />
        </label>
        <label className="flex flex-col gap-1.5">
          <span className="font-medium">New PIN</span>
          <Input
            value={pin}
            onChange={(e) => setPin(e.target.value.replace(/\D/g, "").slice(0, 6))}
            inputMode="numeric"
            type="password"
            aria-invalid={!!problem}
            className="w-32 font-mono tracking-widest"
          />
          {problem && <span className="text-xs text-destructive">{problem}</span>}
        </label>
        {recover.error && <p className="text-destructive">{recover.error.message}</p>}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="ghost" onClick={() => onOpenChange(false)}>
            Cancel
          </Button>
          <Button type="submit" disabled={!code.trim() || !!pinProblem(pin) || recover.isPending}>
            Reset PIN and sign in
          </Button>
        </div>
      </form>
    </Dialog>
  );
}
