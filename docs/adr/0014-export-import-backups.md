# 14. Export, import, backups and the library health check

Status: accepted (2026-09-28)

## Export

A new crate, `libreri-export`, turns books and notes into other formats. It is pure: the library crate collects the data and the crate writes bytes. Formats (board 24):

| Format | What it holds |
|---|---|
| Libreri archive (`.libreri`) | Everything, to import into Libreri again (below) |
| Excel (`.xlsx`), CSV | One row per book: every detail, file, `libreri://` link; optionally status, rating, favourite, progress and note counts. CSV is UTF-8 with a byte-order mark, and cells that a spreadsheet would run as a formula are prefixed with `'` |
| JSON | Every detail; optionally personal data, highlights and bookmarks (with their locators and links) and the notebook |
| BibTeX, RIS, CSL-JSON | For LaTeX, Zotero, Mendeley, EndNote, Pandoc |
| Obsidian notes | A folder: one note per book (details as properties, tags made Obsidian-safe, highlights as quotes with a `[p. 4](libreri://book/…#annotation=…)` link), the notes folder copied as it is, and an index note |
| Calibre folders | `Author/Title/` with the book file, `metadata.opf` and `cover.jpg`, for Calibre's *Add books from folders* |
| Catalogue database | A copy of the SQLite catalogue (`VACUUM INTO`) |

**Whose data.** Personal data in an export is the signed-in profile's own. Only the owner can include everyone's notes (archive, database copy); the owner can already reset anyone's PIN. Kids and guests can copy citations but not export. Exports never contain PIN hashes, recovery codes or online-source keys.

**Citations.** *Cite…* (Mod+Alt+C, the book menu, the details panel) shows APA 7, MLA 9, Chicago (notes and bibliography), Harvard (Cite Them Right), IEEE and BibTeX, and copies them as HTML (italics kept) and plain text. Lists are sorted by author (IEEE keeps the order and numbers them). Titles are used as typed (no automatic sentence case), there is no place of publication or page range, and names are split by a simple rule ("Ludwig van Beethoven" → van Beethoven, Ludwig). The dialog says to check references.

## Libreri archives

A ZIP file that mirrors the library folder: `.library-data/metadata`, `annotations`, `profiles` and `covers`, `Notes/`, optionally `Books/`, a copy of the catalogue in `database/library.db`, and `manifest.json` (format version, library, profiles, every book with its hash, old ids, path, identifiers, and every note with its link). Archives are written to `<name>.part` and renamed when complete. Reading refuses entry names that could leave the destination folder (`..`, absolute paths, drive letters, unknown top-level folders) and never writes more bytes than an entry declares.

**Import (owner only).** *Import a Libreri archive* shows what would happen before anything changes, and lets the owner choose where each profile's notes go (suggested: the same profile, else one with the same name, the archive's owner to this owner, else a new profile). Each book is matched:

1. By content hash, or an old id of the book → *already here*. Details: the newer version wins; tags and categories are added together; a missing cover is filled; a missing file is put back from the archive.
2. The archive has the file → it is added (the hash is checked).
3. Same ISBN, DOI, arXiv id, or title + first author (+ page count within 10 %) → *another copy*. The archive's id becomes an **alias** of the book here, of kind `otherFile`, so every `libreri://` link still opens it, and notes find their place by their quoted text.
4. Otherwise the book is added as a record with **file missing**. Its notes are kept; importing or locating the same file later reconnects everything.

Notes are matched by id: new ones are added, the newer version of a changed one wins, nothing is duplicated. Reading status and position follow the most recently opened; a rating is filled only if there is none; favourites are kept. Note files that already exist with other content are kept side by side as "… (imported <date>).md". Smart collections are added by id.

**Aliases survive rebuilds.** Old ids (`book_aliases`, now with a `kind`) are also written into each book's sidecar, so "Rebuild library index" keeps them (database migration 5).

## Backups

Settings › Export & import › Backups. **Off until the owner turns it on** (user, 2026-09-28). A backup is a Libreri archive of the whole library with everyone's notes and profiles *including PIN hashes*, so a restore brings the PINs back; the notes themselves are plain Markdown in the library anyway, so the hashes add nothing an attacker could not already read. Options: folder (refused inside the library folder), every day or week, keep the last 3–50, with or without book files. Settings are kept per computer in `backups.json` in the app-config folder, keyed by library id, because the folder is a path on this computer. While a library is open, a check every five minutes starts a backup when one is due (a failed one is retried after an hour); only files that are this library's backups (read from their manifests) are ever removed. *Back up now* and the list of backups (Show, Restore…) are there too. *Restore from a backup…* on the Welcome screen makes a new library from an archive, and the archive's owner becomes its owner.

**Keep a file up to date.** The same page can keep a BibTeX, CSL-JSON, RIS, CSV or JSON file of the whole catalogue (details only, never notes) in a place of the reader's choosing, rewritten only when its content changes.

## Library health check

Settings › Library & storage (and › Export & import, and the command palette). Reports: books whose file is missing (with *Show them*), links in the reader's own notes that open no book, books sharing an ISBN or DOI, books with notes from another copy (informational), and what Libreri fixes by itself: missing sidecars, notebook records whose file is gone, covers of books no longer in the library. Unreadable note backups and problems SQLite finds in the catalogue are listed with *Rebuild…*.

**Locate file…** (details panel of a missing book): the chosen file is copied into the book's folder. The same file reconnects at once. A different file (another edition) is used only after the reader agrees; the book then takes the new file and its old id becomes an `otherFile` alias.

## Consequences

- New dependencies: `csv`, `rust_xlsxwriter` and writing with `zip` (already used for reading).
- Round-trip tests (`libreri-library/src/archive_tests.rs`) export and import into a new library at another path and check that every note, link, notebook, collection and profile comes back, that importing twice changes nothing, that missing books reconnect when their file is added, and that notes move to another copy of a book.
- Imports from Calibre, Zotero/Mendeley and Goodreads/StoryGraph follow in 4c.
