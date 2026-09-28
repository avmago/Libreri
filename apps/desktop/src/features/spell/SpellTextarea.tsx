import { forwardRef, useImperativeHandle, useRef, type TextareaHTMLAttributes } from "react";
import { Textarea } from "@/components/ui/input";
import { useSpell } from "./useSpell";

type Props = TextareaHTMLAttributes<HTMLTextAreaElement> & {
  /** The book being written about: its names and terms are accepted. */
  bookId?: string | null;
  /** A bare text box (no input styling). */
  plain?: boolean;
};

/** A text box with Libreri's spell check and word completion. */
export const SpellTextarea = forwardRef<HTMLTextAreaElement, Props>(function SpellTextarea(
  { bookId, plain, ...props },
  ref,
) {
  const inner = useRef<HTMLTextAreaElement>(null);
  useImperativeHandle(ref, () => inner.current!, []);
  useSpell(inner, { bookId });
  return plain ? <textarea ref={inner} {...props} /> : <Textarea ref={inner} {...props} />;
});
