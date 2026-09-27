# 17. Searching inside books, and OCR on request

Status: accepted (2026-09-28)

Phase 5b makes the words inside books searchable, reads scanned pages with OCR when asked, and adds the global search screen (board 15).

## Decisions (user, 2026-09-28)

- **The search index stays on each computer** (app cache, one SQLite file per library) and rebuilds itself. It holds nothing that cannot be read again, so it is never exported or backed up.
- **OCR runs only when asked**: *Make searchable* on a book, a selection or all scanned books. Books without text are marked.
- **OCR text is saved in the library** (`.library-data/text/<bookId>.json`), so it travels with the library, its backups and Libreri archives. Hours of OCR are never lost to a new computer.

## Text

`libreri-formats::text` reads each format into pieces, each with the place it came from:

| Format | Pieces | Place |
|---|---|---|
| PDF | one per page | page |
| DjVu | one per page (`djvutxt`) | page |
| EPUB | chapters (spine items), split at ~6,000 bytes | chapter number and title (from the navigation document or NCX) |
| MOBI / AZW3 | the text, split at page breaks | — |
| FB2 | sections | section title |
| Markdown, text | split at paragraphs | — |
| Comics, audio | none ("pictures only") | — |

- **PDF text comes from hayro** (a PDF interpreter and rasterizer in safe Rust, MIT/Apache): each drawn glyph's Unicode value and position are put back into words and lines, and hyphenated words are joined back together. Invisible OCR layers made by other apps count as text. Password-less encrypted PDFs are read, and so are PDFs whose fonts are not embedded.
- A page with fewer than 16 letters counts as having **no text**. A book is *scanned without text* when under 10 % of its pages have text, and *partly scanned* when at least 3 pages (and over 10 %) have none.
- **MOBI/AZW3** (also carried over from Phase 1): details, cover (EXTH 201) and text (PalmDOC and HUFF/CDIC compression). Books with DRM keep their details; their text is not read.

## Index

- `libreri-search`: SQLite FTS5, `unicode61` with accents removed and prefix indexes. What people type becomes: every word must appear (and matches longer words starting with it), `"quoted words"` must appear together, and `-word` leaves pieces out. Results are grouped by book (best first) with up to three snippets each, and *Show all* lists every match in reading order.
- Each book has a stamp: the extractor version, the OCR file's size and date, and whether DjVuLibre is installed. A background thread re-reads books whose stamp changed after every folder scan, and forgets books that are gone. Progress goes to the interface as `SearchIndexProgress` events.
- Results only include books the signed-in profile may open (Kids see only their folders).
- Known limit: languages written without spaces (Chinese, Japanese) are matched only by whole runs of text. A trigram index may follow.

## OCR

- **Tesseract** (a helper program, ADR 0016) reads page images. PDF pages are rendered by hayro at 300 dpi (at most 5,000 px), in grey; DjVu pages by `ddjvu` at their own resolution. Up to four pages are read side by side, one Tesseract thread each.
- Only pages without text are read. Pages read before are skipped unless *Read again* is ticked (for example in another language). Results are saved every 8 pages, so a stopped run keeps its work.
- Saved per page: the text (lines and paragraphs) and every word with its box as fractions of the page, the same shape as DjVu text layers.
- **In the reader**, OCR words become a hidden text layer over scanned PDF and DjVu pages, so they can be selected, highlighted, noted and found. *Find in book* falls back to OCR text when PDF.js finds nothing.
- **Languages**: Tesseract ships English (and sometimes more). Other languages come from the Tesseract project's `tessdata_fast` set, downloaded into Libreri's data folder on this computer (Settings › Helper programs). A book's own language is used first, then the languages ticked as default. When languages come from both places, Tesseract's own files are copied next to the downloaded ones, because Tesseract reads from one folder.

## Search screen (board 15)

- *Search* in the sidebar, Mod+Alt+F, or typing in the command palette and choosing *Search books, text and notes for …*.
- Scopes: everything, inside books, titles and details (the library's own search), and notes and highlights.
- A match opens the book at the page (PDF, DjVu) or chapter (EPUB) and runs *Find in book* from there.
- With nothing typed, it lists scanned books that cannot be searched yet, with *Make all searchable*.

## Consequences

- New crate `libreri-search`. New modules `libreri-formats::{text, pdftext, ocr, mobi}`, `libreri-helpers::tessdata` and `libreri-library::text`.
- Archives may now contain `.library-data/text/`. Renames follow a book whose file changed, and the health check removes OCR text of books that are gone.
- CI installs `tesseract-ocr` so the OCR tests run; they skip where Tesseract is missing.
