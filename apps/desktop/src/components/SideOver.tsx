import type { ReactNode } from "react";
import { PanelLeft } from "lucide-react";
import { Button } from "@/components/ui/button";

/** A side column beside the content, or sliding over it (see `useSideOver`). */
export function SideOver({
  roomy,
  open,
  onClose,
  children,
}: {
  roomy: boolean;
  open: boolean;
  onClose: () => void;
  children: ReactNode;
}) {
  if (roomy) return <>{children}</>;
  if (!open) return null;
  return (
    <>
      <div className="absolute inset-0 z-20 bg-black/20" aria-hidden onClick={onClose} />
      <div className="absolute inset-y-0 left-0 z-30 flex max-w-[85%] shadow-2xl">{children}</div>
    </>
  );
}

/** The button that opens the column in a narrow window. */
export function SideOverButton({ label, onOpen }: { label: string; onOpen: () => void }) {
  return (
    <div className="flex shrink-0 items-center border-b px-3 py-1.5">
      <Button variant="ghost" size="sm" onClick={onOpen}>
        <PanelLeft /> {label}
      </Button>
    </div>
  );
}
