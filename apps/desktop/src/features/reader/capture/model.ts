import type { Annotation } from "@/lib/ipc";

/** A capture's PDF and title, from its place. */
export function captureOf(a: Annotation): { path: string; title: string; pages: number } | null {
  if (a.kind !== "capture") return null;
  try {
    const v = JSON.parse(a.locator) as { capture?: string; title?: string; pages?: number };
    return v.capture
      ? { path: v.capture, title: v.title ?? "Paper notes", pages: v.pages ?? 1 }
      : null;
  } catch {
    return null;
  }
}
