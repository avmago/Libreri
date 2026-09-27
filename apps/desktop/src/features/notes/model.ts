import type { HighlightColor, NoteDto } from "@/lib/ipc";

export type NoteKind = "all" | "highlights" | "comments" | "bookmarks";

export interface NoteFilter {
  search: string;
  kind: NoteKind;
  colors: HighlightColor[];
  bookId: string | null;
}

export function filterNotes(notes: NoteDto[], f: NoteFilter): NoteDto[] {
  const q = f.search.trim().toLowerCase();
  return notes.filter(({ annotation: a, bookTitle }) => {
    if (f.bookId && a.bookId !== f.bookId) return false;
    if (f.kind === "highlights" && a.kind !== "highlight") return false;
    if (f.kind === "bookmarks" && a.kind !== "bookmark") return false;
    if (f.kind === "comments" && !a.note?.trim()) return false;
    if (f.colors.length && (a.kind !== "highlight" || !f.colors.includes(a.color ?? "yellow"))) {
      return false;
    }
    if (!q) return true;
    return [a.quote?.exact, a.note, a.label, bookTitle].some((t) => t?.toLowerCase().includes(q));
  });
}

/** Groups notes by book, keeping the order books first appear in. */
export function byBook(notes: NoteDto[]): { bookId: string; title: string; notes: NoteDto[] }[] {
  const groups = new Map<string, { bookId: string; title: string; notes: NoteDto[] }>();
  for (const n of notes) {
    const g = groups.get(n.annotation.bookId) ?? {
      bookId: n.annotation.bookId,
      title: n.bookTitle,
      notes: [],
    };
    g.notes.push(n);
    groups.set(n.annotation.bookId, g);
  }
  return [...groups.values()];
}

/** The shown notes as Markdown, one section per book, with links back. */
export function notesToMarkdown(notes: NoteDto[]): string {
  const out: string[] = [];
  for (const g of byBook(notes)) {
    out.push(`## ${g.title}`, "");
    for (const { annotation: a } of [...g.notes].sort(
      (x, y) => (x.annotation.position ?? 0) - (y.annotation.position ?? 0),
    )) {
      const link = `libreri://book/${a.bookId}#annotation=${a.id}`;
      if (a.kind === "bookmark") {
        out.push(`- Bookmark: [${a.label ?? "page"}](${link})`);
        continue;
      }
      const text = (a.quote?.exact ?? "").replace(/\s+/g, " ").trim();
      out.push(`> ${text}`, ">", `> — [${a.label || "Link"}](${link})`);
      if (a.note?.trim()) out.push("", a.note.trim());
      out.push("");
    }
    out.push("");
  }
  return (
    out
      .join("\n")
      .replace(/\n{3,}/g, "\n\n")
      .trim() + "\n"
  );
}

export function relativeDate(iso: string | number, now = Date.now()): string {
  const t = typeof iso === "number" ? iso * 1000 : Date.parse(iso);
  if (!Number.isFinite(t)) return "";
  const days = Math.floor((now - t) / 86_400_000);
  if (days <= 0) return "Today";
  if (days === 1) return "Yesterday";
  if (days < 7) return `${days} days ago`;
  return new Date(t).toLocaleDateString(undefined, {
    day: "numeric",
    month: "short",
    year: "numeric",
  });
}
