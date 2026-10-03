# 19. Edit pages, redaction and version history

Status: accepted.

Libreri lets you change a PDF's file: reorder, turn, delete, insert, extract, split, merge, crop, redact, correct small bits of text, fill in forms, save markup into the file, make it smaller and write OCR text into it. Every change keeps the file as it was.

## Decisions

- **Save makes a new version; the original is kept.** The old file goes to `.library-data/versions/`, is kept until you delete it, and travels in backups and exports that include book files. Notes, highlights, markup and reading positions move with their pages.
- **Text corrections cover the old words and write the new text over them.** The old words are removed from the file (as with redaction) and the new text is written in a standard PDF font; letters those fonts lack are drawn as a picture.
- **All four extras:** filling in PDF forms, making the PDF smaller, writing OCR text into it as an invisible layer, and adding pages from a camera, a phone or scanned pictures.

## Editing (`libreri-pdf-edit`)

- `apply(src, EditPlan, ocr, dest)` writes a new file; the book is never changed in place. A plan lists the pages of the result in order: book pages (turned by 90° steps, cropped to fractions of the page as shown after turning), pages of other PDFs, blank pages and pictures. It returns a **page map** (old page → new page, with turning and cropping).
- Order of work: OCR layer → redactions and corrections → check → page tree → smaller pictures → save. Pages that move keep their inherited size, turning and resources; pages from other PDFs are merged after renumbering their objects; a page used twice is copied.
- **Redaction removes, not covers.** A content-stream walker keeps track of the graphics and text state, and knows every character's width (the PDF's `/Widths` and `/W`, and built-in metrics for the standard 14 fonts). Characters whose middle is under a box are cut out of their text operator; the rest keep their exact positions (kerning replaces what was removed). Paths under a box are dropped, pictures are cut (the covered pixels are painted over, for JPEG, 8-bit, 1-bit and mask images), forms are cleaned inside, inline images and annotations under a box are removed. Then the page is read back with hayro: if any character is still under a box, or something could not be cut (JBIG2, CCITT, JPEG 2000 pictures), **that page becomes a picture** (200 dpi) and the report says so. Saved OCR words under the boxes are removed too.
- **Corrections** remove what is under their box, fill it with the paper colour (taken from around the box) and write the text in Helvetica, Times or Courier (WinAnsi); otherwise the picture the interface drew is placed.
- **OCR text layer:** saved OCR words are written as invisible text (render mode 3) in a glyph-less Type 0 font with a ToUnicode map, stretched to each word's box, so other apps can find and select the text of scans.
- **Smaller:** large JPEG and 8-bit pictures are re-encoded as JPEG, at most 3000, 2000 or 1400 pixels, only when that saves at least 10 %.
- `annotate` writes markup as standard PDF annotations (Ink, Square, Circle, Line, FreeText, Text, Stamp) with appearance streams drawn from the same drawing list as the marked-up copy; the marks then leave Libreri's own markup.
- Forms are filled in the reader (PDF.js form layer); *Save form* sends the PDF that PDF.js writes.

## Versions (`libreri-library`)

- `.library-data/versions/<current book id>/versions.json` lists earlier files (`<old id>.pdf`) with the date, the reason ("Edited pages", "Redacted", "Filled in form", …) and how their pages became the next version's pages. The folder is renamed when the book's id changes (ids are content hashes, ADR 0004); the old id stays an alias, so links keep working.
- Saving a version: the old file is copied into the folder, the new file moved into place, the id changed, sidecar, covers, OCR text and backups renamed; then notes, markup geometry (points, boxes, widths), highlights, reading positions and OCR words follow the page map. Notes on removed pages are kept with the version.
- **Restore** makes an earlier version current; the current file becomes a version, notes go back through the inverted page maps, and notes kept with the versions come back. **Delete** removes one version and joins the page maps around it.
- Settings › Storage shows how much room versions take and can delete them all; the health check lists versions of books no longer in the library.

## Adding pages

- Blank pages (A4, Letter, A5, Legal), pages from a PDF file or from a book in the library, pictures (including a scanner's saved files), photos from the computer's camera, and photos from a phone: the barcode phone page (ADR 0013) gained a *pages* mode that sends each photo as a new page.
- Scanners are not driven directly (TWAIN, WIA, SANE and ImageCapture differ on every system); their saved pictures or PDFs are added like any other.

## Consequences

- A book's id changes whenever its file does; everything keyed by id follows (tabs, sidecars, covers, OCR text, backups, the search index, which is updated in the background).
- Versions can take a lot of room; they are listed and deletable, and only travel with book files.
