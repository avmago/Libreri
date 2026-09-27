import { useCallback, useEffect, useRef, useState } from "react";
import { Delete } from "lucide-react";
import { cn } from "@/lib/utils";

/**
 * Six dots and a number pad. Digits can be typed or clicked; the PIN is
 * submitted as soon as the sixth digit is entered.
 */
export function PinPad({
  onSubmit,
  disabled = false,
  error,
  label = "Enter your PIN",
  resetKey,
}: {
  onSubmit: (pin: string) => void;
  disabled?: boolean;
  error?: string | null;
  label?: string;
  /** Changing this clears the dots (after a wrong PIN). */
  resetKey?: unknown;
}) {
  const [pin, setPin] = useState("");
  const [shake, setShake] = useState(false);
  const [seenKey, setSeenKey] = useState(resetKey);
  const ref = useRef<HTMLDivElement>(null);

  // A wrong PIN clears the dots and shakes them.
  if (resetKey !== seenKey) {
    setSeenKey(resetKey);
    setPin("");
    setShake(true);
  }
  useEffect(() => {
    if (!shake) return;
    const t = setTimeout(() => setShake(false), 400);
    return () => clearTimeout(t);
  }, [shake]);

  useEffect(() => ref.current?.focus(), []);

  const press = useCallback(
    (digit: string) => {
      if (disabled) return;
      setPin((p) => {
        if (p.length >= 6) return p;
        const next = p + digit;
        if (next.length === 6) setTimeout(() => onSubmit(next), 60);
        return next;
      });
    },
    [disabled, onSubmit],
  );
  const back = useCallback(() => setPin((p) => p.slice(0, -1)), []);

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (/^\d$/.test(e.key)) {
      e.preventDefault();
      press(e.key);
    } else if (e.key === "Backspace") {
      e.preventDefault();
      back();
    }
  };

  return (
    <div
      ref={ref}
      tabIndex={0}
      onKeyDown={onKeyDown}
      role="group"
      aria-label={label}
      className="flex flex-col items-center gap-5 outline-none"
    >
      <div
        className={cn("flex gap-3", shake && "animate-[lb-shake_0.35s_ease-in-out]")}
        aria-live="polite"
        aria-label={`${pin.length} of 6 digits entered`}
      >
        {Array.from({ length: 6 }, (_, i) => (
          <span
            key={i}
            className={cn(
              "size-3.5 rounded-full border-2 border-foreground/70 transition-colors",
              i < pin.length && "bg-foreground",
            )}
          />
        ))}
      </div>
      <p
        className={cn(
          "h-5 text-center text-[13px]",
          error ? "text-destructive" : "text-muted-foreground",
        )}
        role={error ? "alert" : undefined}
      >
        {error ?? label}
      </p>
      <div className="grid grid-cols-3 gap-2.5">
        {["1", "2", "3", "4", "5", "6", "7", "8", "9"].map((d) => (
          <PadKey key={d} onClick={() => press(d)} disabled={disabled}>
            {d}
          </PadKey>
        ))}
        <span />
        <PadKey onClick={() => press("0")} disabled={disabled}>
          0
        </PadKey>
        <PadKey onClick={back} disabled={disabled || pin.length === 0} label="Delete last digit">
          <Delete className="size-5" />
        </PadKey>
      </div>
    </div>
  );
}

function PadKey({
  children,
  onClick,
  disabled,
  label,
}: {
  children: React.ReactNode;
  onClick: () => void;
  disabled?: boolean;
  label?: string;
}) {
  return (
    <button
      type="button"
      tabIndex={-1}
      aria-label={label}
      onClick={onClick}
      disabled={disabled}
      className="flex size-16 items-center justify-center rounded-full border bg-background text-xl font-medium tabular-nums hover:bg-muted active:bg-muted disabled:opacity-40"
    >
      {children}
    </button>
  );
}
