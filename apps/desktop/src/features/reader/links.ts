/** `libreri://book/<id>#annotation=<uuid>` → its parts. */
export function parseBookLink(href: string): { bookId: string; annotation?: string } | null {
  const m = /^libreri:\/\/book\/([0-9a-f]{64})(?:#annotation=([0-9a-f-]{36}))?$/i.exec(href.trim());
  if (!m) return null;
  return { bookId: m[1]!.toLowerCase(), annotation: m[2]?.toLowerCase() };
}
