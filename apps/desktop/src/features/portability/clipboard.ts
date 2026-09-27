import type { Citation } from "@/lib/ipc";

/** Copies a reference with italics where the style wants them. */
export async function copyCitation(c: Citation) {
  try {
    await navigator.clipboard.write([
      new ClipboardItem({
        "text/plain": new Blob([c.text], { type: "text/plain" }),
        "text/html": new Blob([c.html], { type: "text/html" }),
      }),
    ]);
  } catch {
    await navigator.clipboard.writeText(c.text);
  }
}
