<p align="center">
  <img src="assets/app-icon.svg" alt="" width="96" height="96">
</p>

<h1 align="center">Libreri</h1>

<p align="center">
  A local-first library for books, papers, comics and audiobooks, with notes, highlights, handwriting and read-aloud.<br>
  For macOS, Windows and Linux.
</p>

---

Your library is an ordinary folder. Books stay as plain files, notes are Markdown, and everything can be exported and moved to another computer with every link intact. Nothing leaves your computer unless you ask: online lookups, feeds and model downloads are opt-in.

> **Status:** Phases 0–9 are built (Phase 9, bug fixes and reading comfort, is being tried on the Mac). Phase 10, polish and release, is next. See [docs/development-phases.md](docs/development-phases.md).

## Contents

- [What it does](#what-it-does)
- [Formats](#formats)
- [Your library on disk](#your-library-on-disk)
- [Run it](#run-it)
- [How the code is organised](#how-the-code-is-organised)
- [Documentation](#documentation)
- [Licence](#licence)

## What it does

### Library

- **Import** files and whole folders by dragging them in. Duplicates are caught by content, and moved files are found again.
- **Real folders** on disk, plus tags, categories, series, ratings, reading status and favourites.
- **Grid, list and shelf views**, filters, fast search, **smart collections** (saved searches), bulk edit, and an **Organize** view for tidying tags and categories.
- **Online details** by ISBN, DOI, arXiv id or title, from Open Library, Google Books, Crossref, OpenAlex, Semantic Scholar, arXiv, ComicVine and ISBNdb. A field-by-field merge screen, covers, and **barcode scanning** with a webcam, a picture or your phone.
- **Full-text search** inside every book, with **OCR** (Tesseract) to make scans searchable.
- **Library health check**, version history for edited PDFs, and automatic **backups**.

### Reading

- **Tabs**, split view and separate windows, remembered between launches.
- **9 page themes** with dark PDFs (recolour, invert or dim), brightness and contrast, text size and line spacing.
- **Full-screen reading** (F11), focus mode, two-page spreads, manga right-to-left and webtoon scrolling.
- **ADHD reading:** bionic reading, a line highlight that follows the pointer, and a reading mask.
- **Find in book**, table of contents, bookmarks, and your place kept per profile.
- **Compare** two versions or two PDFs, side by side, as an overlay or as a list of changes.

### Notes

- **Highlights** in four colours, **comments**, and a **Markdown notebook** per book whose quotes link back to the page.
- **Markup mode** on PDF, DjVu and comic pages: pen with pressure, highlighter, shapes, arrows, text boxes, sticky notes, stamps and signatures, plus a **measure tool**.
- **Handwriting canvases** (Excalidraw) beside the book, with *clip from page* and *ink to text*.
- **Paper notes:** photograph pages with the camera or your phone. They are straightened, cleaned, made searchable and linked to the page.
- **Voice notes** on a passage or page, written down on your computer (whisper.cpp), and **dictation**.
- **Links** to web pages, videos (with a start time) and recordings, with a player in the side panel and offline copies of pages.
- **Maths as LaTeX:** exact from EPUB and Markdown, rebuilt from PDF text, or read from a picture with an optional on-device model.
- A **Notes hub** for everything across books, with exports to Markdown and Obsidian.

### Listening

- **Read aloud** with the system voices (or eSpeak NG): sentence highlighting, page turning, and maths read as words.
- **Audiobooks** (MP3, M4B, M4A, AAC, OGG, Opus, FLAC) with chapters, a sleep timer, bookmarks and media keys.
- **Link an audiobook to its ebook:** the book follows the audio, and the floating player can start from the place you are reading.

### Writing

- **Spell check** everywhere you write, with 45+ downloadable languages, and **word completion** that learns names and terms from your books.

### Editing PDFs

- **Edit pages:** reorder, rotate, delete, insert (from files, scans, the camera or your phone), crop, extract, split and merge.
- **Redaction** that really removes the text, small **text corrections**, **forms**, compression, and OCR text written into the PDF.
- Every save keeps the previous **version**, with your notes moved to the right pages.

### Feeds

- Follow **RSS and Atom** feeds: arXiv categories and searches, journals, newsletters and blogs, or import them from OPML.
- Sort feeds into **folders** (categories and subcategories), and filter items by topic.
- **Download** new papers (PDF) and articles (a clean Markdown copy), or delete them. Folders can download new items automatically.
- Downloads live in their own **Feeds** folder. **Add to library** moves one into any folder of your books, with the feed's details.

### People and privacy

- **Profiles** with 6-digit PINs, auto-lock, Kids profiles limited to chosen folders, and guests who leave nothing behind.
- Everyone has their own notes, positions, collections and feeds in the same library.
- **Export everything:** Libreri archives, CSV, Excel, JSON, BibTeX, RIS, CSL-JSON, Obsidian and Calibre. **Import** from Calibre, Zotero, Mendeley, Goodreads and StoryGraph.
- **Keyboard first:** over 100 rebindable shortcuts (Ctrl+/ lists them), a command palette (Ctrl+K), and optional Vim keys.

## Formats

| Kind | Formats |
|---|---|
| Documents | PDF (with forms), DjVu |
| Ebooks | EPUB, MOBI, AZW3, FB2 (DRM-free) |
| Text | Markdown (KaTeX maths, Mermaid, code), RTF, plain text |
| Comics | CBZ, CBR, CB7, CBT, CBA |
| Audiobooks | MP3, M4B, M4A, AAC, OGG, Opus, FLAC |

Some formats and features use **helper programs** that Libreri can install for you with one click (Homebrew, winget, apt, dnf, pacman or zypper):

| Helper | Used for |
|---|---|
| DjVuLibre | DjVu books |
| Tesseract | OCR, searchable scans, ink to text on Linux |
| unar | CBR, CB7 and CBA comics |
| eSpeak NG | Read aloud where the system has no voices |

Optional **downloads**, all run on your computer: speech models (whisper.cpp) for voice notes and dictation, the pix2tex maths model, spell-check dictionaries, OCR languages and extra canvas fonts.

## Your library on disk

```
My Library/
  Books/              your books and audiobooks, in your own folders
  Notes/<profile>/    notebooks, canvases, voice notes, paper notes, web pages
  Feeds/<profile>/    papers and articles downloaded from feeds
  .library-data/      Libreri's catalogue, covers, sidecars and versions
```

Only relative paths are stored, so a library can be moved, synced or opened on another computer. The catalogue can be rebuilt from the files and the JSON sidecars at any time. See [docs/data-portability.md](docs/data-portability.md).

## Run it

**Requirements**

- [Rust](https://rustup.rs) via rustup. The right version (1.95) is installed automatically from `rust-toolchain.toml`.
- [Node.js](https://nodejs.org) 20+ and [pnpm](https://pnpm.io) 10 (`corepack enable`).
- Tauri's platform prerequisites: <https://v2.tauri.app/start/prerequisites/>
  - **macOS** 13.3 or newer: Xcode Command Line Tools (`xcode-select --install`).
  - **Linux:** `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.
  - **Windows:** Microsoft C++ Build Tools and WebView2 (included with Windows 11).

**Start**

```sh
pnpm install
pnpm dev          # the app with hot reload (runs pnpm install again when the lockfile changes)
```

The development build is much slower than the finished app. To judge speed, build it:

```sh
pnpm build        # installers in target/release/bundle/
```

**Other commands**

| Command | What it does |
|---|---|
| `pnpm test` | Frontend tests (Vitest) and Rust tests |
| `pnpm lint` | ESLint, TypeScript and Clippy |
| `pnpm format` | Prettier and rustfmt |
| `pnpm gen:ipc` | Regenerates the TypeScript types from the Rust commands |
| `pnpm icons` | Regenerates the app icons from `assets/app-icon.png` |
| `cargo run -p libreri-library --release --example seed -- <books> <library>` | Makes a library from a folder of books, for trying things out |
| `cargo run -p libreri-feeds --example live [addresses…]` | Reads feeds on the internet, to check the feed reader |

## How the code is organised

```
apps/desktop/src          React interface, one folder per feature (features/*); readers/ holds the book renderers
apps/desktop/src-tauri    Thin Tauri shell: commands, events, the book:// protocol

crates/libreri-core       Domain types, ids and the library layout (no app dependencies)
crates/libreri-db         SQLite and migrations
crates/libreri-library    Library folders: import, scan, watcher, sidecars, profiles, notes, versions, feeds
crates/libreri-jobs       Background job queue with progress and cancel
crates/libreri-profiles   6-digit PINs (Argon2id), lockout and recovery codes
crates/libreri-formats    Details, covers and text from every format; OCR
crates/libreri-thumbs     Covers and grid thumbnails
crates/libreri-metadata   Online details from eight sources
crates/libreri-scan       Barcodes, the phone page, and paper-note clean-up
crates/libreri-export     Exports, citations and Libreri archives
crates/libreri-helpers    Finding and installing helper programs; downloads
crates/libreri-search     Full-text search index (SQLite FTS5)
crates/libreri-pdf-edit   Page editing, redaction, markup export, forms
crates/libreri-speech     Voice notes, dictation and audiobook sync (whisper.cpp)
crates/libreri-spell      Spell check and word suggestions
crates/libreri-maths      Maths from pictures (pix2tex with candle)
crates/libreri-links      Links: video details, offline copies, the local player
crates/libreri-feeds      RSS, Atom and OPML; arXiv; feed downloads

docs/                     Phases, architecture, portability and decisions (docs/adr)
```

Built with Tauri 2, React, TypeScript, Tailwind and shadcn/ui, Rust and SQLite. Readers use PDF.js, foliate-js, KaTeX and Mermaid.

Dependencies only point downward: interface → shell → crates → core. Read [docs/code-structure.md](docs/code-structure.md) before adding code, and [docs/data-portability.md](docs/data-portability.md) before touching anything that links notes to books.

## Documentation

| Document | About |
|---|---|
| [docs/development-phases.md](docs/development-phases.md) | What each phase delivers, and the decisions behind it |
| [docs/code-structure.md](docs/code-structure.md) | Where code goes, feature by feature |
| [docs/data-portability.md](docs/data-portability.md) | How books, notes and links are kept, so nothing is locked in |
| [docs/phase9-bugs.md](docs/phase9-bugs.md) | The Phase 9 bug list and what was fixed |
| [docs/adr/](docs/adr) | 27 architecture decision records |

## Licence

Not decided yet (see [docs/adr/0006-licence-deferred.md](docs/adr/0006-licence-deferred.md)). All rights reserved until then.
