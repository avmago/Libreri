/**
 * Sharing a feed item (a paper, an article): its best address, and the
 * same as text, a Markdown link, a BibTeX entry or an email.
 */
import type { FeedItem } from "@/lib/ipc";

type Shareable = Pick<
  FeedItem,
  "title" | "link" | "authors" | "published" | "doi" | "arxivId" | "source"
>;

/** The address that lasts: arXiv's abstract page, the DOI, or the link. */
export function shareUrl(it: Shareable): string | null {
  if (it.arxivId) return `https://arxiv.org/abs/${it.arxivId.replace(/v\d+$/, "")}`;
  if (it.doi) return `https://doi.org/${it.doi}`;
  return it.link ?? null;
}

function year(it: Shareable): string | null {
  const y = it.published ? new Date(it.published).getFullYear() : NaN;
  return Number.isFinite(y) ? String(y) : null;
}

/** "Ada Lovelace, Alan Turing et al." */
export function authorsShort(authors: string[]): string {
  if (authors.length <= 2) return authors.join(" and ");
  return `${authors[0]}, ${authors[1]} et al.`;
}

/** Title, authors and address, one per line. */
export function shareText(it: Shareable): string {
  const url = shareUrl(it);
  const who = it.authors.length ? authorsShort(it.authors) : null;
  return [it.title, [who, year(it)].filter(Boolean).join(", ") || null, url]
    .filter(Boolean)
    .join("\n");
}

export function shareMarkdown(it: Shareable): string {
  const url = shareUrl(it);
  const title = it.title.replace(/[[\]]/g, "");
  return url ? `[${title}](${url})` : title;
}

/** A BibTeX entry: @misc for arXiv preprints, @article with a DOI. */
export function shareBibtex(it: Shareable): string {
  const y = year(it);
  const first = (it.authors[0] ?? "anon")
    .split(/\s+/)
    .pop()!
    .toLowerCase()
    .replace(/[^a-z]/g, "");
  const word = (it.title.toLowerCase().match(/[a-z]{4,}/) ?? ["paper"])[0];
  const key = `${first || "anon"}${y ?? ""}${word}`;
  const esc = (s: string) => s.replace(/[{}]/g, "");
  const fields: [string, string][] = [["title", `{${esc(it.title)}}`]];
  if (it.authors.length) fields.push(["author", esc(it.authors.join(" and "))]);
  if (y) fields.push(["year", y]);
  if (it.arxivId) {
    fields.push(["eprint", it.arxivId], ["archivePrefix", "arXiv"]);
  }
  if (it.doi) fields.push(["doi", it.doi]);
  const url = shareUrl(it);
  if (url) fields.push(["url", url]);
  const kind = it.doi && !it.arxivId ? "article" : "misc";
  return `@${kind}{${key},\n${fields.map(([k, v]) => `  ${k} = {${v}}`).join(",\n")}\n}\n`;
}

/** A mailto: address with the subject and the text filled in. */
export function shareMailto(it: Shareable): string {
  const body = `${shareText(it)}\n\nShared from Libreri`;
  return `mailto:?subject=${encodeURIComponent(it.title)}&body=${encodeURIComponent(body)}`;
}
