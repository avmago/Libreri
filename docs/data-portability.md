# Libreri — Data portability: links must survive backup, export and a new computer

Requirement (user, 2026-09-27): when the database, metadata and every kind of note (highlights, comments, Markdown notebooks, canvases, voice notes, handwriting captures, drawings, links) are exported and imported on another computer, **every link back to a page / paragraph / position in a PDF, EPUB, DjVu, comic, Markdown or audiobook must still work.** This is a non-negotiable rule for all phases.

## Design rules that make this true
1. **Books are identified by content, not by path.** Every book has a `bookId` derived from its BLAKE3 content hash (plus a stable UUID for the record). Links never store absolute paths or computer-specific data.
2. **One link format everywhere:** `libreri://book/<bookId>#<anchor>` — used in the database, annotations JSON, Markdown notebooks, canvas files, voice-note metadata and exports.
3. **Robust anchors per format** (each anchor stores a primary locator *and* a text quote fallback):
   | Format | Primary locator | Fallback |
   |---|---|---|
   | PDF | page index + rectangle (PDF coordinates, zoom-independent) | quoted text + surrounding words (W3C TextQuoteSelector) |
   | EPUB / MOBI / AZW3 / FB2 / TXT | EPUB CFI / character offset | quoted text + context |
   | DjVu | page + rectangle | OCR text quote |
   | Comics | page index + rectangle | page image hash |
   | Markdown | heading path + line | quoted text |
   | Audiobook | timestamp (ms) + chapter | — |
4. **Relative paths only** inside the library (`Books/…`, `Notes/<user>/…`). The library folder can move anywhere.
5. **Notes stay readable outside Libreri:** Markdown and canvas files keep `libreri://` links (clickable in Libreri) and a plain-text label such as "p. 4" so they still make sense in Obsidian.

## Export / backup
- Libreri archive contains a **manifest.json**: format version, every book's `bookId`, content hash, size, title/authors/ISBN/DOI, original relative path, and a list of every note/annotation/attachment with the anchors it uses.
- Includes: database snapshot, JSON sidecars, annotations, notebooks, canvases, voice notes (audio files), captured images, links, versions history, settings (no PINs, no API keys).
- Book files optional; when left out, links are kept and re-attached on import.

## Import on another computer — re-linking
1. Match each `bookId` to a book in the target library by **content hash** (exact).
2. If not found: match by ISBN / DOI / title+authors+page count (e.g. a different copy or edition of the same book) → mark as "matched, different file"; anchors then fall back to **text-quote re-anchoring**.
3. Still missing: keep all notes and show the book as **"file missing"** with a *Locate file…* button; once the file is added, every link reconnects automatically.
4. Import summary: "248 books linked · 3 matched by ISBN (re-anchored) · 1 missing", with a repair list.
- Merging into an existing library never duplicates notes (IDs are UUIDs; conflicts resolved by last-modified with a report).
- Profiles: imported notes attach to the matching profile or a new one; the owner chooses.

## Personal data on disk (Phase 3)
- Each profile is backed up to `.library-data/profiles/<id>.json` (PIN hashes included, so a rebuild keeps them; exports leave them out) and its smart collections to `<id>.collections.json`.
- Per book and profile, `.library-data/annotations/<profile>/<book>.json` holds reading status, rating, favourite, progress, position and every highlight, comment and bookmark (format 2; format 1 files with only annotations still load).
- Notebooks stay Markdown in `Notes/<profile name>/`; renaming a profile renames the folder and notebook links follow. Guests leave nothing on disk.
- See `docs/adr/0010-profiles-and-pins.md`.

## How it is built (Phase 4b)
- Archives, import, re-linking, backups and the health check: `docs/adr/0014-export-import-backups.md`. Round-trip tests live in `crates/libreri-library/src/archive_tests.rs`.
- Old ids of a book (edited file, or notes from another copy) are *aliases* kept in the database and in the book's sidecar, so links survive a rebuild and an import.
- Exports carry only the signed-in profile's personal data (the owner may include everyone's) and never PINs or keys. Backups carry everyone's data and PIN hashes, so a restore is complete.
- Differences from the plan above: the archive's catalogue copy is for reference and other tools; import works from the sidecars and backups, which are the source of truth. Versions history is in the archive since Phase 6b (with book files).

## Coming from other apps (Phase 4c)
- Calibre, Zotero, BibTeX/RIS and Goodreads/StoryGraph imports copy files and never change the other app's library. Zotero highlights are converted to Libreri anchors (page + rectangles, with the quoted text as fallback) and get stable ids, so importing again never duplicates them. See `docs/adr/0015-import-from-other-apps.md`.

## Per-computer settings (Phase 4)
- Backup settings and the file kept up to date are in `backups.json` (per library id), because they name folders on this computer.
- Online sources in use and API keys (ComicVine, ISBNdb) are kept in `online-sources.json` in the computer's app-config folder, not in the library, so exports, backups and copies of the library folder never carry them. See `docs/adr/0013-online-details.md`.
- Details and covers found online are saved like any edit: in the database and the book's sidecar, so they travel with the library.

## Page cache and helper programs (Phase 5a)
- Rendered DjVu pages and unpacked comic pages are kept in the app cache folder on each computer (`pages/<bookId>/`, up to 2 GB). They can always be made again from the book file, so they are never exported or backed up.
- Helper programs (DjVuLibre, Tesseract, unar) are installed per computer and are not part of the library. On a new computer, Libreri offers to install them the first time a book needs one. See `docs/adr/0016-helpers-comics-djvu.md`.
- The full-text search index lives in the app cache (`search/<libraryId>.sqlite`) and rebuilds itself on any computer; it is never exported.
- OCR text is saved in the library (`.library-data/text/<bookId>.json`: each page's text and word boxes), included in backups and Libreri archives and restored with the book. OCR language files are per computer (app data `tessdata/`). See `docs/adr/0017-full-text-search-and-ocr.md`.

## Markup (Phase 6a)
- Drawings, text boxes, sticky notes, stamps, signatures, pictures and measurements are annotations of kind `markup`: per profile, in the profile's JSON backups, backups and Libreri archives, and restored with the book like highlights. Pictures and signatures are stored inside the mark. The book file is never changed; *Export marked-up copy* makes a separate PDF. Saved signatures and your own stamps are in your profile's preferences. See `docs/adr/0018-markup-mode.md`.

## Audiobooks (Phase 7a)
- An audiobook's link to the book it reads, and its sync points, are in `.library-data/audio-links/<audiobook id>.json`. Each point has a time in seconds, a reader locator, a 0–1 place in the text and a label. The file follows the audiobook when its id changes, and is included in backups and Libreri archives and restored with the book. Positions and bookmarks in audiobooks are ordinary reading data, with `{"type":"audio","t":…}` locators. Read-aloud settings are profile preferences. See `docs/adr/0021-read-aloud-and-audiobooks.md`.
- Sync points found by listening (Phase 7b) are the same sync points with `"auto": true`.
- Voice notes are FLAC files in `Notes/<profile>/Voice notes/`, so they are copied, backed up and archived with the rest of the profile's notes folder.
  - **In a book:** an annotation of kind `voice`. Its locator is an ordinary reader locator (a PDF page or highlight rectangles, an EPUB CFI, a text range, or a scroll fraction) plus `"audio": "Notes/<profile>/Voice notes/<file>.flac"` and `"duration"`. The quote is the selected text, if any; `note` is the transcript.
  - **In a notebook:** a relative Markdown link to the file (`[0:42](<Voice notes/<file>.flac>)`), which also works in other editors.
  - Speech models are per computer (app data `whisper/`) and never exported. See `docs/adr/0022-voice-notes-dictation-and-sync.md`.
- The personal spelling dictionary is `Notes/<profile>/Dictionary.txt` (one word per line), so it travels with the notes folder. Writing settings (spell check, completion, languages) are profile preferences; downloaded dictionaries are per computer (app data `dictionaries/`) and never exported. See `docs/adr/0023-spell-check-and-word-suggestions.md`.
- Canvases are standard Excalidraw files in `Notes/<profile>/Canvases/` (pictures inside). Their `libreri.book` is a `libreri://book/<id>` link, and clipped figures link to `libreri://book/<id>#page=<n>&rect=x,y,w,h`. Both follow book id changes through aliases. They open in other Excalidraw tools, which ignore Libreri's details. The ink-to-text choice is per computer. See `docs/adr/0024-handwriting-canvases.md`.

## Edited PDFs (versions)
- Page edits create a new version with a **page map** (old page → new page). Anchors are migrated through the map; text-quote fallback catches the rest. Exports include the version history so links to older versions still resolve.
- **As built (Phase 6b):** every change to a PDF's file (edited pages, redaction, text corrections, a filled-in form, markup saved into the PDF, a smaller copy, an OCR text layer) keeps the file as it was in `.library-data/versions/<current book id>/` (`<old id>.pdf` plus `versions.json`: date, reason and each version's page map). The book's id changes with its content and the old id stays an alias, so every link still opens. Notes, highlights, markup, reading positions and OCR text move with their pages (turning and cropping included); notes on removed pages are kept with the version and come back when it is restored. Versions stay until you delete them, travel in backups and exports that include book files (with their page maps) and are merged on import when the same file is in the library. See `docs/adr/0019-edit-pages-and-versions.md`.

## Verification (built into CI from Phase 1 onward)
- Round-trip tests: create library → add books + every note type → export → import into an empty library on a different path → assert every link opens the same page / paragraph / timestamp.
- Tests for moved files, renamed files, edited PDFs, and "same book, different file".
- Library health check reports any broken or re-anchored link.
