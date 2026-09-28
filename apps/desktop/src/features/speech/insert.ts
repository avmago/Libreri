export type Field = HTMLTextAreaElement | HTMLInputElement;

/** Types `text` at the cursor as if typed (so undo and onChange work),
 * with a space before it when needed. */
export function insertAtCursor(el: Field | null, text: string) {
  if (!el || !text.trim()) return;
  el.focus();
  const at = el.selectionStart ?? el.value.length;
  const before = el.value.slice(0, at);
  const piece = (before && !/\s$/.test(before) ? " " : "") + text.trim();
  if (document.execCommand?.("insertText", false, piece)) return;
  // Fallback: set the value the way React notices.
  const proto = Object.getPrototypeOf(el) as object;
  const setter = Object.getOwnPropertyDescriptor(proto, "value")?.set;
  const end = el.selectionEnd ?? at;
  setter?.call(el, before + piece + el.value.slice(end));
  el.dispatchEvent(new Event("input", { bubbles: true }));
  el.setSelectionRange(at + piece.length, at + piece.length);
}
