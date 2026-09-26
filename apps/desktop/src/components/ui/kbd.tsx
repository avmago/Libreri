import { cn } from "@/lib/utils";
import { displayKeys, platform } from "@/lib/shortcuts";

/** Shows a shortcut such as "Mod+K" as keycaps for the current platform. */
export function Kbd({ shortcut, className }: { shortcut: string; className?: string }) {
  return (
    <span className={cn("inline-flex gap-1", className)}>
      {displayKeys(shortcut, platform).map((k) => (
        <kbd
          key={k}
          className="min-w-5 rounded-[5px] border border-b-2 bg-muted px-1.5 text-center font-mono text-[11px] leading-5 text-muted-foreground"
        >
          {k}
        </kbd>
      ))}
    </span>
  );
}
