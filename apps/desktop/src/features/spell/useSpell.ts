import { useEffect, useRef, type RefObject } from "react";
import { attachSpell, type SpellOptions } from "./field";
import { learnBook } from "./api";

/**
 * Libreri's spell check and word completion on a text box. `bookId` is
 * the book being read or written about: its names and terms are accepted
 * and offered first.
 */
export function useSpell(
  ref: RefObject<HTMLTextAreaElement | null>,
  options: SpellOptions = {},
  enabled = true,
) {
  const opts = useRef(options);
  useEffect(() => {
    opts.current = options;
  });
  const bookId = options.bookId ?? null;
  useEffect(() => {
    if (bookId) void learnBook(bookId);
  }, [bookId]);
  useEffect(() => {
    const area = ref.current;
    if (!enabled || !area) return;
    return attachSpell(area, () => opts.current);
  }, [ref, enabled]);
}
