import { cn } from "@/lib/utils";
import { displayKeys, platform, useShortcutKeys, type ActionId } from "@/lib/shortcuts";

/**
 * Shows keys as keycaps for the current platform: either a shortcut string
 * ("Mod+K") or the keys currently assigned to an action.
 */
export function Kbd({
  shortcut,
  action,
  className,
}: {
  shortcut?: string | null;
  action?: ActionId;
  className?: string;
}) {
  const bound = useShortcutKeys(action ?? "palette.open");
  const keys = shortcut ?? (action ? bound : null);
  if (!keys) return null;
  return (
    <span className={cn("inline-flex items-center gap-1", className)}>
      {displayKeys(keys, platform).map((k, i) =>
        k === "then" ? (
          <span key={i} className="text-[11px] text-muted-foreground">
            then
          </span>
        ) : (
          <kbd
            key={i}
            className="min-w-5 rounded-[5px] border border-b-2 bg-muted px-1.5 text-center font-mono text-[11px] leading-5 text-muted-foreground"
          >
            {k}
          </kbd>
        ),
      )}
    </span>
  );
}
