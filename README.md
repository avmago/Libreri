<div align="center">

<img src="assets/app-icon.svg" alt="Libreri" width="112" height="112">

# Libreri

**A quiet home for everything you read, and everything you think about it.**<br>
Books, papers, comics, audiobooks and podcasts in one library on your own computer,<br>
with highlights, notes, flashcards and a reading calendar that stay yours.

![macOS](https://img.shields.io/badge/macOS-13.3%2B-18181b?style=flat-square&logo=apple&logoColor=white)
![Windows](https://img.shields.io/badge/Windows-10%2B-18181b?style=flat-square&logo=windows&logoColor=white)
![Linux](https://img.shields.io/badge/Linux-WebKitGTK-18181b?style=flat-square&logo=linux&logoColor=white)
<br>
![Tauri 2](https://img.shields.io/badge/Tauri-2-24c8db?style=flat-square&logo=tauri&logoColor=white)
![Rust](https://img.shields.io/badge/Rust-1.95-b7410e?style=flat-square&logo=rust&logoColor=white)
![React](https://img.shields.io/badge/React-TypeScript-3178c6?style=flat-square&logo=react&logoColor=white)
![Local-first](https://img.shields.io/badge/data-local--first-16a34a?style=flat-square)
[![Licence: GPL v3](https://img.shields.io/badge/licence-GPL--3.0--or--later-18181b?style=flat-square)](LICENSE)

[Why](#why-libreri) · [Screenshots](#screenshots) · [Features](#what-you-can-do) · [Privacy](#what-goes-online) · [Formats](#formats) · [On disk](#your-library-on-disk) · [Install](#install) · [Build](#build-from-source) · [Docs](#documentation)

<br>

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="assets/screenshots/library-dark.png">
  <img src="assets/screenshots/library.png" alt="Libreri's library: books in a grid with generated covers, and the sidebar with lists, collections and folders" width="900">
</picture>

</div>

> [!NOTE]
> Libreri is close to its first public beta. Everything below is built and works on macOS, Windows and Linux; installers are not signed yet, so your system will ask once before opening it.

## Why Libreri

Most reading apps want your books in their cloud and your notes in their format. Libreri takes the opposite view: your library is a folder you own, your notes are plain Markdown next to it, and the app is simply a good place to read, think and remember.

<table>
<tr>
<td width="33%" valign="top">

### Yours, on disk

Books stay as the files they are. Notes, highlights and your reading calendar are kept beside them as readable files. Move the folder, sync it, back it up: every link still works.

</td>
<td width="33%" valign="top">

### Private by default

Nothing leaves your computer unless you ask for it. The speech, voice, OCR and maths models run on your machine. There are no accounts, no tracking, no ads.

</td>
<td width="33%" valign="top">

### Built for study

Highlights that link back to the page, a notebook per book, maths everywhere, flashcards from your own highlights, and search across every word you own.

</td>
</tr>
</table>

## Screenshots

<table>
<tr>
<td width="50%"><img src="assets/screenshots/reader.png" alt="Reading a paper with maths, a table of contents and the paper page theme"><br><sub><b>Reading</b>: maths, contents, page themes, read aloud and notes</sub></td>
<td width="50%"><img src="assets/screenshots/feeds.png" alt="Feeds: arXiv categories, journals and newsletters in folders, with new items to download"><br><sub><b>Feeds</b>: arXiv, journals and newsletters, straight into the library</sub></td>
</tr>
<tr>
<td width="50%"><img src="assets/screenshots/podcasts.png" alt="Podcasts: shows with artwork, episodes, Up next and the player at the bottom"><br><sub><b>Podcasts</b>: follow shows and listen while you read</sub></td>
<td width="50%"><img src="assets/screenshots/library-dark.png" alt="The library in the dark theme"><br><sub><b>Dark theme</b>: the whole app, plus nine page themes for books</sub></td>
</tr>
</table>

## What you can do

### Keep a library

- **Bring books in** by dragging files or whole folders. Duplicates are recognised by their content, and books you move on disk are found again.
- **Organise** with real folders, tags, categories, ratings, reading status and favourites, and save any search as a **smart collection**.
- **Series** get their own view: every series with its covers, how far you are, and the next book to read.
- **Grid, list and shelf** views stay smooth with 10,000 books and more.
- **Fill in details** by ISBN, DOI, arXiv id or title from eight free sources, with covers. Scan a barcode with your camera or phone.
- **Search inside every book**, scans included: Tesseract reads scanned pages, or download PaddleOCR-VL to read tables and formulas too.
- **Look after it:** a health check, version history for edited PDFs, and automatic backups.

### Read comfortably

- **Tabs**, split view and separate windows, all remembered when you come back.
- **Nine page themes**, dark PDFs, brightness, contrast, text size and spacing; full screen, focus mode, two-page spreads, manga and webtoon modes.
- **Zoom** with buttons, Ctrl+scroll or a pinch, and **print** PDFs.
- **Look up** a word or a name you select: its meanings from Wiktionary, or a short Wikipedia summary, in the book's language.
- **ADHD reading:** bionic reading (on ebooks *and* on PDF and scanned pages), a line highlight under the pointer, and a reading mask.
- Works in **narrow windows**, down to a third of the screen.

### Take notes that last

- **Highlights** in four colours and **comments**, which can hold **maths** (`$x^2$`, `$$\int f$$`) drawn as you type.
- A **Markdown notebook** for every book, plus notes that belong to no book.
- **Markup** on PDF and comic pages: pen, shapes, stamps, signatures and a measuring tool.
- **Handwriting canvases** beside the book, with *clip from page* and *ink to text*.
- **Paper notes** photographed with your camera or phone, cleaned up and made searchable.
- **Voice notes** and **dictation**, written down on your computer.
- **Maths as LaTeX**, copied from the text or read from a picture of the page.
- A **Notes hub** that gathers everything across books, and exports to Markdown and Obsidian.

### Remember what you read

- **Daily review** turns your highlights into flashcards on a spaced schedule (FSRS, the method modern Anki uses): passages to complete, questions from your comments, and cloze cards that hide chosen words or whole formulas. Export them to **Anki** at any time.
- A **reading calendar** with month, week and day views, reading goals ("finish by…"), due dates, streaks and the pages a day to stay on track. Export it to any calendar app.
- A **study timer** with focus sessions, a countdown and a stopwatch. It counts as reading the open book, offers to note what you took away at each break, and shows in the macOS menu bar or the Windows and Linux tray.

### Listen

- **Read aloud** with the sentence highlighted and the page turning by itself, maths read as words, in your system's voices or **natural voices made on your computer**: Kokoro (54 voices in nine languages) and Piper (40+ languages, only voices free to use).
- **Audiobooks** with chapters, sleep timer and bookmarks, and an audiobook **linked to its ebook** so the page follows the voice.
- **Podcasts:** find shows (Apple Podcasts, or Podcast Index with your own key), follow them, stream or download, with Up next and a speed for each show. Transcripts and chapters follow along; shows without a transcript can be **written down on your computer**. **Note this moment** saves what was said into the notebook of the book you are reading, linked to the page.
- One **floating player** for all of it, which folds into a small box.

### Follow new research

- **Feeds** for arXiv categories and searches, journals, newsletters and blogs (RSS, Atom, OPML), in folders.
- **Download** papers and articles (as clean Markdown), read them in the full reader, and **add them to your library** with authors, abstract, DOI, arXiv id and journal already filled in.
- **Share** a paper as a link, a BibTeX citation or a Markdown link.

### Edit PDFs safely

- **Pages:** reorder, rotate, delete, insert, crop, extract, split and merge.
- **Redaction** that truly removes the text, **text corrections** and **forms**.
- Make files smaller, and write OCR text into scans.
- Every save keeps the **previous version**, and your notes move to the right pages.

### Share a computer

- **Profiles** with 6-digit PINs, auto-lock, **Kids** profiles limited to chosen folders, and **guests** who leave nothing behind.
- Everyone keeps their own notes, places, collections, feeds, podcasts, flashcards and calendar.

### Work your way

- **100+ keyboard shortcuts**, all changeable, a command palette (Ctrl+K), and optional Vim keys.
- **Accessible:** screen-reader labels, strong contrast and visible keyboard focus throughout.
- **Spell check** in 45+ languages, with word completion that learns from your books.
- **Export** to Libreri archives, CSV, Excel, JSON, BibTeX, RIS, CSL-JSON, Obsidian and Calibre; **import** from Calibre, Zotero, Mendeley, Goodreads and StoryGraph.
- **Backups and archives** carry everything: books, notes, flashcards, the calendar, feeds and podcasts.
- **Updates itself** when a new version is out, after asking you.

## What goes online

Only what you ask for, and only the minimum:

| When you… | Libreri contacts | It sends |
|---|---|---|
| Fill in a book's details | Open Library, Google Books, Crossref, arXiv, OpenAlex, Semantic Scholar (ISBNdb and ComicVine with your own key) | The ISBN, DOI, arXiv id or title |
| Look up a word | Wiktionary, Wikipedia | The selected words and the book's language |
| Follow feeds or podcasts | The feed's own site; Apple Podcasts or Podcast Index to search | The address, or your search words |
| Download a voice, model or dictionary | Its official download site | Nothing about you |
| Check for updates | GitHub (Libreri's releases) | Nothing about you |

Your books, notes, highlights and reading habits never leave your computer. The full [privacy policy](PRIVACY.md) explains it in more detail.

## Formats

| | Formats |
|---|---|
| **Documents** | PDF (with forms), DjVu |
| **Ebooks** | EPUB, MOBI, AZW3, FB2 (DRM-free) |
| **Text** | Markdown (maths, Mermaid diagrams, code), RTF, plain text |
| **Comics** | CBZ, CBR, CB7, CBT, CBA |
| **Audio** | MP3, M4B, M4A, AAC, OGG, Opus, FLAC, and podcasts |

A few formats use small **helper programs**, which Libreri installs with one click (Homebrew, winget, apt, dnf, pacman or zypper):

| Helper | Used for |
|---|---|
| DjVuLibre | DjVu books |
| Tesseract | OCR and searchable scans |
| unar | CBR, CB7 and CBA comics |
| eSpeak NG | Read aloud where the system has no voices, and natural voices |

Optional **downloads**, each with its own Delete button: natural voices (Kokoro and Piper), speech models (whisper.cpp), the PaddleOCR-VL model, the pix2tex maths model, dictionaries, OCR languages and canvas fonts.

## Your library on disk

```
My Library/
├── Books/              your books and audiobooks, in your own folders
├── Notes/<profile>/    notebooks, canvases, voice notes, paper notes, web pages
├── Feeds/<profile>/    papers, articles and podcast episodes you downloaded
└── .library-data/      the catalogue, covers, versions, and a JSON copy of every
                        book's details, your highlights, flashcards and calendar
```

Only relative paths are stored, so a library can be moved, synced or opened on another computer. The catalogue can be rebuilt from the files at any time, and nothing is ever kept only inside the database. Every file is written aside and swapped in, so a sudden quit never leaves half a file. More in [data portability](docs/data-portability.md).

## Install

Download the installer for your system from [Releases](https://github.com/avmg0/Libreri/releases): a `.dmg` for macOS (Apple Silicon or Intel), an `.msi` or `.exe` for Windows, and an AppImage, `.deb` or `.rpm` for Linux. Each release also has a Flatpak (`Libreri.flatpak`) with its helper programs built in; Flathub is on its way ([details](flatpak/README.md)).

Until the builds are signed, macOS asks before opening Libreri the first time: right-click the app, choose **Open**, then **Open** again. Windows may show SmartScreen: choose **More info**, then **Run anyway**.

After that, Libreri tells you when a new version is ready (Settings › General › Updates).

## Build from source

**You need**

- [Rust](https://rustup.rs) via rustup. The right version (1.95) is installed automatically from `rust-toolchain.toml`.
- [Node.js](https://nodejs.org) 20+ and [pnpm](https://pnpm.io) 10 (`corepack enable`).
- Tauri's [platform prerequisites](https://v2.tauri.app/start/prerequisites/):
  - **macOS** 13.3 or newer: Xcode Command Line Tools (`xcode-select --install`).
  - **Linux:** `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`.
  - **Windows:** Microsoft C++ Build Tools and WebView2 (included with Windows 11).

**Run**

```sh
pnpm install
pnpm dev          # the app, with hot reload
pnpm build        # installers in target/release/bundle/
```

The development build is much slower than the finished app; build it to judge speed.

<details>
<summary><b>Other commands</b></summary>
<br>

| Command | What it does |
|---|---|
| `pnpm test` | Frontend tests (Vitest) and Rust tests |
| `pnpm lint` | ESLint, TypeScript and Clippy |
| `pnpm format` | Prettier and rustfmt |
| `pnpm gen:ipc` | Regenerates the TypeScript types from the Rust commands |
| `pnpm icons` | Regenerates the app icons from `assets/app-icon.png` |
| `cargo run -p libreri-library --release --example seed -- <books> <library>` | Makes a library from a folder of books, for trying things out |
| `cargo test -p libreri-library --release scale -- --ignored --nocapture` | Times everyday work with 10,000 books |

</details>

<details>
<summary><b>How the code is organised</b></summary>
<br>

```
apps/desktop/src          React interface, one folder per feature (features/*); readers/ holds the book renderers
apps/desktop/src-tauri    Thin Tauri shell: commands, events, the book:// protocol, the updater

crates/libreri-core       Domain types, ids and the library layout
crates/libreri-db         SQLite and migrations
crates/libreri-library    The library folder: import, scan, sidecars, profiles, notes, versions, feeds, study and review
crates/libreri-jobs       Background jobs with progress and cancel
crates/libreri-profiles   6-digit PINs (Argon2id), lockout and recovery codes
crates/libreri-formats    Details, covers and text from every format; OCR words
crates/libreri-thumbs     Covers and thumbnails
crates/libreri-metadata   Online details from eight sources; word look-up
crates/libreri-scan       Barcodes, the phone page, paper-note clean-up
crates/libreri-export     Exports, citations, Libreri archives, Anki decks
crates/libreri-helpers    Finding and installing helper programs; downloads
crates/libreri-search     Full-text search (SQLite FTS5)
crates/libreri-pdf-edit   Page editing, redaction, markup export, forms
crates/libreri-speech     Voice notes, dictation, audiobook sync, transcripts (whisper.cpp)
crates/libreri-spell      Spell check and word suggestions
crates/libreri-maths      Maths from pictures (pix2tex with candle)
crates/libreri-links      Links to pages and videos, offline copies, the local player
crates/libreri-feeds      RSS, Atom, OPML, arXiv and podcasts
crates/libreri-ocr        Optional OCR models (PaddleOCR-VL)
crates/libreri-voices     Natural voices for read aloud (Kokoro, Piper)
```

Dependencies only point downward: interface → shell → crates → core.

</details>

**Built with** Tauri 2, Rust, SQLite, React, TypeScript, Tailwind and shadcn/ui. Books are drawn with PDF.js, foliate-js, KaTeX and Mermaid; models run with candle, whisper.cpp and ONNX Runtime; flashcards are scheduled with ts-fsrs.

## Documentation

| Document | What it covers |
|---|---|
| [Architecture](docs/architecture.md) | How Libreri is built: the stack, the layers, every crate, the library folder and the optional downloads |
| [Code structure](docs/code-structure.md) | Where each feature lives, from the interface down to the crates, and the rules for adding code |
| [Data portability](docs/data-portability.md) | How books, notes, highlights and links are kept, moved, backed up and restored, so nothing is ever locked in |
| [Third-party licences](docs/third-party-licenses.md) | Every library Libreri is built on, with its licence |
| [Privacy policy](PRIVACY.md) | What stays on your computer, and exactly what goes online when you ask |
| [Terms of use](TERMS.md) | What the free-software licence means for you, in everyday words |
| [Contributing](CONTRIBUTING.md) | Reporting bugs, sending changes, and the [contributor agreement](CLA.md) |

## Licence

Libreri is free software: you can use it, study it, change it and share it under the terms of the [GNU General Public License](LICENSE), version 3 or (at your option) any later version. If you share a changed version, share its source under the same licence.

It comes with no warranty. The libraries it is built on, and their licences, are listed in [third-party licences](docs/third-party-licenses.md).

<div align="center">
<br>
<sub>Made for people who read to learn.</sub>
</div>
