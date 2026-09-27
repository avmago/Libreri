# 15. Importing from Calibre, Zotero, Mendeley, Goodreads and StoryGraph

Status: accepted (2026-09-28)

*Import from another app* (the Import menu, Settings › Export & import, the command palette) reads another app's library and brings it into Libreri. Owners and standard profiles can import; reading data goes to the signed-in profile.

## Rules (user, 2026-09-28)

- **Files are copied.** The other app's folder is only read (Calibre's `metadata.db` read-only, Zotero's database through a temporary copy because Zotero locks it), so Calibre and Zotero keep working as before.
- **One file per book.** Calibre: the first format in an order the reader can change in the dialog (default EPUB, PDF, AZW3, MOBI, FB2, DjVu, comics, text, audio). Zotero: the attachment with the most highlights, then PDFs.
- **Goodreads and StoryGraph only update books already here** (matched by ISBN, else title and first author). Books not found are listed in the report, not added.

## What comes along

| Source | Details | Personal (signed-in profile) |
|---|---|---|
| Calibre library folder | title, authors, tags, series and number, publisher, year, language, ISBN/DOI/arXiv identifiers, description, cover | rating |
| Zotero data folder | every field Libreri has, creators (editors and others as contributors), tags, collections as categories (`Parent/Child`), trash left out | PDF highlights and notes (Zotero 6+), child notes appended to the book's notebook under "From Zotero" |
| BibTeX (.bib) / RIS (.ris) — Mendeley, JabRef, EndNote, Zotero exports | the usual fields, LaTeX accents decoded, attached files from `file = {…}` (Mendeley, JabRef, Zotero styles) or `L1`/`L4` | — |
| Goodreads / StoryGraph CSV | — | status (read, currently reading, to read, did not finish), rating (StoryGraph half stars rounded), favourites shelf, last read date |

- Details from the other app win over what Libreri reads from the file itself; tags and categories are added together. For a book whose file is already in the library, only empty fields are filled.
- Items without a file (a reference with no PDF) add their details to a matching book here, if there is one; otherwise they are listed as "no file".
- Status and rating fill only empty ones unless *Replace my reading status and ratings* is ticked.
- **Zotero highlights** keep their page and rectangles: Zotero stores PDF points from the bottom left; Libreri converts them to fractions of the page as shown (crop box within the media box, page rotation applied), the format its PDF reader uses. Highlights keep their text as the quote (the fallback anchor), their comment and a colour mapped to Libreri's four. Zotero notes on the page become bookmarks with their text. Image, ink and text-box annotations are skipped.
- **Importing twice adds nothing twice:** files are matched by content hash, and Zotero notes get a UUID made from their Zotero key.

## Consequences

- `libreri-export::foreign` holds the readers (it now also depends on `rusqlite`, `libreri-metadata` for tidying and `libreri-formats`); `libreri-library::foreign` decides what to copy and merges. `libreri-formats::page_boxes` reads PDF page boxes.
- Readers are tested against small fixtures that follow each app's schema (Calibre and Zotero SQLite tables, BibTeX, RIS, Goodreads and StoryGraph CSV). Real exports vary; unknown fields are ignored rather than refused.
- Not imported: Calibre custom columns (such as a "read" column), Zotero linked files relative to its base folder, Mendeley Reference Manager's cloud library (export BibTeX from it instead), Goodreads shelves other than the exclusive one and "favorites".
