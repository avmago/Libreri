import { useState } from "react";
import { useWide } from "@/lib/useWide";

/**
 * A side column that stays beside the content in a wide window, and in a
 * narrow one slides over it from a button, closing when something in it
 * is chosen (`closeOn` changes) or when the content is clicked.
 */
export function useSideOver(minWidth: number, closeOn: unknown) {
  const roomy = useWide(minWidth);
  const [open, setOpen] = useState(false);
  const [last, setLast] = useState(closeOn);
  // Something was chosen in it: close (adjusting state while rendering).
  if (last !== closeOn) {
    setLast(closeOn);
    if (open) setOpen(false);
  }
  // Widened: it is beside the content again, and starts closed next time.
  const [wasRoomy, setWasRoomy] = useState(roomy);
  if (wasRoomy !== roomy) {
    setWasRoomy(roomy);
    if (roomy && open) setOpen(false);
  }
  return { roomy, open, setOpen };
}
