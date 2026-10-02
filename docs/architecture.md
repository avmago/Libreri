# Libreri: architecture (as built, 2026-10-02)

This replaces the agreed concept (v7, 2026-09-26). The concept's decisions are now built, or written up as decision records in `docs/adr/` (32 so far). This page is the map: what Libreri is made of, where data lives, and how the parts talk. For where to put new code, see [code structure](code-structure.md). For how notes keep their links on another computer, see [data portability](data-portability.md).

## Stack

- **Shell:** Tauri 2 (Rust), for macOS 13.3+, Windows 10+ and Linux (WebKitGTK). Rust is pinned at 1.95 (`rust-toolchain.toml`).
- **Interface:** React, TypeScript, Vite, Tailwind v4 and shadcn/ui, with `lucide-react` icons and no emojis.
  - Data from Rust goes through TanStack Query; view state through zustand.
  - IPC types are generated from the Rust commands by tauri-specta (`lib/ipc/bindings.ts`).
- **Data:** SQLite (rusqlite) with FTS5. A book's identity is the BLAKE3 hash of its content.
  - A file watcher (`notify`) notices changes in the library folder.
  - A background job queue runs long work with progress and cancel.
- **Book bytes:** a custom `book://` protocol streams files, page images, audio and Markdown made from RTF to the web view, with range requests.
- **Models:** run on the computer, never on a server.
  - candle runs pix2tex (maths from pictures) and PaddleOCR-VL (OCR).
  - whisper.cpp runs speech-to-text.
  - ONNX Runtime runs the Kokoro and Piper voices; it is downloaded with the first voice and loaded at run time.
- **Helper programs:** DjVuLibre, Tesseract, unar and eSpeak NG. Libreri installs them with one click through Homebrew, winget, apt, dnf, pacman or zypper. They stay separate programs, so no GPL code is linked into Libreri.

## Layers

Dependencies only point down.

1. **Interface:** `apps/desktop/src`. One folder per feature (`features/*`) and the book renderers (`readers/*`). It has no file system access and no SQL.
2. **Shell:** `apps/desktop/src-tauri`. Thin commands, one file per feature, plus typed events, `AppState`, the `book://` protocol, and the tray and menu bar.
3. **Service crates:** `crates/libreri-*`. All the rules, with no Tauri dependency.
4. **Core:** `crates/libreri-core`. Domain types, ids, the library layout and settings.

| Crate | What it does |
|---|---|
| libreri-core | Book, metadata, profile and settings types; the library layout |
| libreri-db | SQLite, migrations, repositories |
| libreri-library | The library folder: import, scan, watcher, sidecars, profiles, reading data, notes, versions, feeds and podcasts files, the study calendar and review |
| libreri-jobs | Background jobs with progress and cancel |
| libreri-profiles | 6-digit PINs (Argon2id), lockout, recovery codes |
| libreri-formats | Details, covers and text from every format (PDF, EPUB, MOBI/AZW3, FB2, Markdown, RTF, text, comics, DjVu, audio); OCR words |
| libreri-thumbs | Covers and thumbnails |
| libreri-metadata | Online details from eight sources |
| libreri-scan | Barcodes, the phone page, paper-note clean-up |
| libreri-export | Exports, citations, Libreri archives, Anki decks, imports from other apps |
| libreri-helpers | Finding, installing and running helper programs; downloads; eSpeak NG speech |
| libreri-search | Full-text index (FTS5), per computer |
| libreri-pdf-edit | Page editing, redaction, corrections, forms, markup export, compare |
| libreri-speech | Voice notes, dictation, audiobook sync (whisper.cpp) |
| libreri-spell | Spell check and word suggestions |
| libreri-maths | Maths from pictures (pix2tex) |
| libreri-links | Links to pages and videos, offline copies, the local player |
| libreri-feeds | RSS, Atom and OPML, arXiv, podcasts (Apple and Podcast Index search) |
| libreri-ocr | Optional OCR models (PaddleOCR-VL) |
| libreri-voices | Natural voices for read aloud (Kokoro, Piper) |

## The library folder

```
My Library/
├── Books/              books and audiobooks, in your own folders (real folders on disk)
├── Notes/<profile>/    notebooks, canvases, voice notes, paper notes, links, web pages, Dictionary.txt
├── Feeds/<profile>/    .feeds.json, .podcasts.json, downloaded papers, articles and episodes
└── .library-data/      library.db, sidecars, covers, annotations/<profile>/, text/ (OCR),
                        versions/, audio-links/, profiles/<id>.json, <id>.collections.json,
                        <id>.study.json, <id>.review.json
```

- **Paths:** only relative paths are stored. The database can be rebuilt from the files, sidecars and backups at any time.
- **Locking:** a lock file keeps two computers from writing at once when the folder is synced.
- **Kept per computer, never in the library:**
  - settings and the online keys;
  - the search index and the page cache;
  - helper programs;
  - downloaded models, voices, dictionaries and OCR languages.

## People

Profiles have an optional 6-digit PIN, auto-lock, Kids profiles (chosen folders only) and guests (nothing kept).

- **Shared:** books, folders, bibliographic details, covers and categories.
- **Per profile:** reading data, notes, collections, feeds, podcasts, the reading calendar, review cards and preferences.
- **Security:** the signed-in profile belongs to the open library session, so the interface cannot ask for someone else's data (ADR 0010).

## Reading

- **Renderers:** each one implements `readers/types.ts::Renderer`.
  - PDF uses PDF.js 6 (legacy build).
  - EPUB, MOBI, AZW3 and FB2 use foliate-js.
  - Markdown, text and RTF go through the document renderer, with KaTeX and Mermaid.
  - Comics and DjVu are page images.
  - Audiobooks use their own player.
- **Reader features:** tabs, separate windows, nine page themes, zoom (buttons, Ctrl+scroll, pinch), print (PDF), full screen and ADHD reading (bionic, line highlight, mask).
- **Modes:** Read, Markup, and Edit pages (PDF).
- **One floating player** at the bottom of the book for read aloud, the linked audiobook and podcasts. It folds into a small box that can sit on either side.
- **Read aloud:** sentences come from the renderer (`readers/speech`). Voices are:
  - the system's (Web Speech);
  - eSpeak NG through Libreri;
  - natural voices made on the computer (Kokoro and Piper, ADR 0030).
- **Older WebKit:** small fixes in `lib/polyfills.ts` cover what the macOS web view lacks.

## Feeds, podcasts, study, review

- **Feeds** (ADR 0027): RSS, Atom and arXiv in folders, with downloads in `Feeds/<profile>/`.
  - **Add to library** moves a download in with the feed's details: title, authors, abstract, year, DOI, arXiv id, link, tags, categories, journal and kind. An online lookup then fills the rest.
  - **Share** copies the link, text, Markdown or BibTeX, or emails it.
- **Podcasts** (ADR 0028): Apple search by default, Podcast Index with your own key. One player for the window, with saved places, per-show speed and Up next.
- **Study** (ADR 0031):
  - A reading calendar (month, week and day), goals and due dates, and a study timer (focus, countdown, stopwatch).
  - Sessions count as reading the open book.
  - While a timer runs it shows in the macOS menu bar and Dock, on the Windows taskbar and tray, or in the Linux tray.

- **Daily review** (ADR 0032): highlights come back as passage, question-and-answer and cloze cards on an FSRS schedule (`ts-fsrs`), chosen in each highlight's menu. Cards are worked out from the highlights; only choices and the schedule are kept. Export to Anki (.apkg).

## Optional downloads

Each optional download has its own Download and Delete in Settings, runs on the computer, and is never exported.

| What | Where | Run with |
|---|---|---|
| Speech models (whisper) | app data `whisper/` | whisper.cpp |
| Maths model (pix2tex) | app data `maths/` | candle |
| OCR model (PaddleOCR-VL 1.6) | app data `ocr-models/` | candle (Metal on Apple computers) |
| Natural voices (Kokoro, Piper) and ONNX Runtime | app data `voices/` | ONNX Runtime |
| Dictionaries, OCR languages, canvas fonts | app data | — |

## Not yet

- Bionic reading on PDF pages: the design (word starts thickened on the page) is waiting for approval.
- Archives (`.libreri`) do not carry Feeds, podcasts, the reading calendar or review; backups carry the calendar and review.
- Version 2 brings phones and tablets with sync over the home network, printing and sharing, AI research tools, and more ideas (see the [development phases](development-phases.md)).
