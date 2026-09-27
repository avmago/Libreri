import { UserRound } from "lucide-react";
import { cn } from "@/lib/utils";
import { colourOf, initials } from "../model";

export function ProfileAvatar({
  name,
  colour,
  guest = false,
  size = 32,
  className,
}: {
  name: string;
  colour: string;
  guest?: boolean;
  size?: number;
  className?: string;
}) {
  return (
    <span
      aria-hidden
      className={cn(
        "inline-flex shrink-0 items-center justify-center rounded-full font-semibold text-white select-none",
        guest && "border border-dashed border-muted-foreground/60 bg-muted text-muted-foreground",
        className,
      )}
      style={{
        width: size,
        height: size,
        fontSize: Math.round(size * 0.38),
        background: guest ? undefined : colourOf(colour),
      }}
    >
      {guest ? <UserRound style={{ width: size * 0.5, height: size * 0.5 }} /> : initials(name)}
    </span>
  );
}
