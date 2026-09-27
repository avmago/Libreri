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
- Differences from the plan above: the archive's catalogue copy is for reference and other tools; import works from the sidecars and backups, which are the source of truth. Versions history arrives with Phase 6 and will be added to the archive then.

## Coming from other apps (Phase 4c)
- Calibre, Zotero, BibTeX/RIS and Goodreads/StoryGraph imports copy files and never change the other app's library. Zotero highlights are converted to Libreri anchors (page + rectangles, with the quoted text as fallback) and get stable ids, so importing again never duplicates them. See `docs/adr/0015-import-from-other-apps.md`.

## Per-computer settings (Phase 4)
- Backup settings and the file kept up to date are in `backups.json` (per library id), because they name folders on this computer.
- Online sources in use and API keys (ComicVine, ISBNdb) are kept in `online-sources.json` in the computer's app-config folder, not in the library, so exports, backups and copies of the library folder never carry them. See `docs/adr/0013-online-details.md`.
- Details and covers found online are saved like any edit: in the database and the book's sidecar, so they travel with the library.

## Edited PDFs (versions)
- Page edits create a new version with a **page map** (old page → new page). Anchors are migrated through the map; text-quote fallback catches the rest. Exports include the version history so links to older versions still resolve.

## Verification (built into CI from Phase 1 onward)
- Round-trip tests: create library → add books + every note type → export → import into an empty library on a different path → assert every link opens the same page / paragraph / timestamp.
- Tests for moved files, renamed files, edited PDFs, and "same book, different file".
- Library health check reports any broken or re-anchored link.
