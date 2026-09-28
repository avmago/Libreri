/** `libreri://book/<id>#annotation=<uuid>` or `…#page=<n>` (with an
 * optional `&rect=x,y,w,h`) → its parts. */
export function parseBookLink(
  href: string,
): { bookId: string; annotation?: string; page?: number } | null {
  const m =
    /^libreri:\/\/book\/([0-9a-f]{64})(?:#(?:annotation=([0-9a-f-]{36})|page=(\d+)(?:&rect=[\d.,]+)?))?$/i.exec(
      href.trim(),
    );
  if (!m) return null;
  return {
    bookId: m[1]!.toLowerCase(),
    annotation: m[2]?.toLowerCase(),
    page: m[3] ? Number(m[3]) : undefined,
  };
}

/** A tab's `jumpTo` for a page (annotation ids are used as they are). */
export const pageJump = (page: number) => `page:${page}`;
