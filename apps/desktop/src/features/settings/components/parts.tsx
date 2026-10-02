import type { ReactNode } from "react";
import { Laptop, Library, UserRound } from "lucide-react";
import { cn } from "@/lib/utils";

export type Scope = "yours" | "library" | "computer";

const SCOPES: Record<Scope, { label: string; hint: string; Icon: typeof Laptop }> = {
  yours: { label: "Yours", hint: "Only for your profile", Icon: UserRound },
  library: { label: "Library", hint: "For everyone who uses this library", Icon: Library },
  computer: { label: "This computer", hint: "For every library on this computer", Icon: Laptop },
};

export function ScopeBadge({ scope }: { scope: Scope }) {
  const { label, hint, Icon } = SCOPES[scope];
  return (
    <span
      title={hint}
      className="inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-medium text-muted-foreground"
    >
      <Icon className="size-3" aria-hidden /> {label}
    </span>
  );
}

/** A titled group of settings with who they apply to. */
export function Group({
  title,
  scope,
  description,
  children,
}: {
  title: string;
  scope: Scope;
  description?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-1" aria-label={title}>
      <div className="flex items-center gap-2 pb-1">
        <h2 className="text-[15px] font-semibold">{title}</h2>
        <ScopeBadge scope={scope} />
      </div>
      {description && <p className="pb-2 text-muted-foreground">{description}</p>}
      <div className="flex flex-col divide-y rounded-lg border bg-background">{children}</div>
    </section>
  );
}

/** One setting: label and help on the left, the control on the right. */
export function Row({
  label,
  help,
  children,
  className,
  htmlFor,
}: {
  label: string;
  help?: ReactNode;
  children: ReactNode;
  className?: string;
  htmlFor?: string;
}) {
  return (
    <div className={cn("flex items-center gap-6 px-4 py-3", className)}>
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <label htmlFor={htmlFor} className="font-medium">
          {label}
        </label>
        {help && <p className="text-[12.5px] leading-snug text-muted-foreground">{help}</p>}
      </div>
      <div className="flex shrink-0 items-center gap-2">{children}</div>
    </div>
  );
}

/** An on/off switch that reads well with a screen reader. */
export function Switch({
  checked,
  onChange,
  label,
  id,
  disabled,
  title,
}: {
  checked: boolean;
  onChange: (v: boolean) => void;
  label: string;
  id?: string;
  disabled?: boolean;
  title?: string;
}) {
  return (
    <button
      id={id}
      type="button"
      role="switch"
      aria-checked={checked}
      aria-label={label}
      title={title}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn(
        "relative h-5 w-9 shrink-0 rounded-full transition-colors disabled:cursor-not-allowed disabled:opacity-40",
        checked ? "bg-primary" : "bg-muted-foreground/30",
      )}
    >
      <span
        className={cn(
          "absolute top-0.5 left-0.5 size-4 rounded-full bg-background shadow transition-transform",
          checked && "translate-x-4",
        )}
      />
    </button>
  );
}

/** A labelled range slider with its value. */
export function Slider({
  value,
  onChange,
  min,
  max,
  step = 1,
  label,
  format = (v) => `${v}`,
}: {
  value: number;
  onChange: (v: number) => void;
  min: number;
  max: number;
  step?: number;
  label: string;
  format?: (v: number) => string;
}) {
  return (
    <span className="flex items-center gap-3">
      <input
        type="range"
        min={min}
        max={max}
        step={step}
        value={value}
        aria-label={label}
        onChange={(e) => onChange(Number(e.target.value))}
        className="w-44 accent-primary"
      />
      <span className="w-12 text-right text-[12.5px] text-muted-foreground tabular-nums">
        {format(value)}
      </span>
    </span>
  );
}
