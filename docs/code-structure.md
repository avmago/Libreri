# Libreri — Source code structure (updated 2026-10-02)

Visual version: artifact "Libreri Code Map". This doc is the reference for coding.

## Layers (dependencies point down only)
1. **Frontend** `apps/desktop/src` (React + TS) — screens, components, view state. Talks to Rust only via generated IPC client `lib/ipc`; reads book bytes via `book://`. No FS, SQL or business rules.
2. **Tauri shell** `apps/desktop/src-tauri` — app setup, AppState, thin `#[tauri::command]` adapters (one file per feature), typed events, `book://` protocol, capabilities.
3. **Service crates** `crates/libreri-*` — all business logic, no Tauri dependency; each owns one feature area and implements core ports.
4. **Core** `crates/libreri-core` — domain types, IDs, errors, traits (ports), settings schema. Depends only on serde/thiserror.

## Folder tree
```
libreri/
├─ apps/desktop/
│  ├─ src-tauri/  src/{main.rs, state.rs, commands/, events.rs, protocol.rs}  capabilities/  tauri.conf.json
│  └─ src/
│     ├─ app/            providers, router, shell layout, error boundary
│     ├─ features/       canvas command-palette details feeds helpers library notes organize
│     │                  podcasts portability profiles reader search settings speech spell study
│     │                  (each: components/ hooks/ store.ts api.ts index.ts; markup, edit pages,
│     │                  compare, capture, weblinks, listening, print and speech live under reader/)
│     ├─ readers/        types.ts (Renderer interface) + pdf/ ebook/ document/ (Markdown, text, RTF)
│     │                  pages/ (comics, DjVu) markup/ math/ speech/ focus.ts (ADHD reading)
│     ├─ components/     ui/ (shadcn, generated) · common/ (BookCover, TagChip, Kbd…)
│     ├─ lib/            ipc/ (tauri-specta bindings) · shortcuts/ · tabs/ · floating/ (the folding
│     │                  player) · print/ · polyfills.ts (older WebKit) · useWide · useSideOver · utils
│     ├─ styles/         tokens.css, themes, reader themes
│     └─ main.tsx
├─ crates/
│  ├─ libreri-core        domain types, IDs, errors, ports
│  ├─ libreri-db          SQLite, migrations, repositories, FTS5
│  ├─ libreri-library     library folder, lock, scan, watcher, import/move, hashing, sidecars, versions
│  ├─ libreri-formats     extractors: pdf, epub, mobi, fb2, txt, md, comics (zip/rar/7z/tar/ace), djvu (pages, text layer, outline), mobi/azw3, audio; text (book text in pieces), pdftext (hayro: PDF text, page images for OCR), ocr (Tesseract TSV)
│  ├─ libreri-thumbs      thumbnails, covers
│  ├─ libreri-metadata    online details: sources (Open Library, Google Books, Crossref, OpenAlex, Semantic Scholar, arXiv, ComicVine, ISBNdb), scoring, tag/category tidying, cover download
│  ├─ libreri-helpers    helper programs (DjVuLibre, Tesseract, unar, eSpeak NG): find, version, install with the package manager, run; OCR language data (tessdata); downloads with stall detection; eSpeak speech
│  ├─ libreri-scan        barcode reading (rxing) and the one-time phone page for scanning
│  ├─ libreri-feeds       RSS/Atom/OPML parsing and fetching, arXiv, suggested sources, Markdown articles, podcasts (Apple and Podcast Index search)
│  ├─ libreri-ocr         optional OCR models (PaddleOCR-VL with PP-DocLayout, run with candle): download, remove, load, read regions
│  ├─ libreri-voices      natural voices (Kokoro, Piper): downloads, the Piper list and licences, ONNX Runtime loaded at run time, phonemes with eSpeak NG, WAV
│  ├─ libreri-profiles    6-digit PIN rules, Argon2 hashing, lockout, recovery codes
│  ├─ libreri-search      full-text index (SQLite FTS5, per computer): pieces, stamps, queries, snippets
│  ├─ libreri-export      formats: CSV/XLSX/JSON, BibTeX/RIS/CSL-JSON, citation styles, Obsidian notes, Calibre OPF, Libreri archives (zip + manifest); importers from other apps (4c)
│  ├─ libreri-pdf-edit    marked-up copies (6a); page edits, redaction, corrections, OCR text layer, compression, PDF annotations (6b); compare and report (6c)
│  ├─ libreri-links       links to pages and videos: address reading (YouTube, Vimeo, start times), oEmbed details, offline copies (dom_smoothie + ammonia), the local player page
│  ├─ libreri-maths       reading maths from pictures as LaTeX (pix2tex run with candle), model download
│  ├─ libreri-spell       spell check (Hunspell dictionaries via spellbook), dictionary downloads, word completion learned from books and notes
│  ├─ libreri-speech      speech recognition (whisper.cpp): models, audio decoding (symphonia), FLAC voice notes, transcription, audiobook ↔ text matching
│  └─ libreri-jobs        background queue, progress, cancellation
│     (Planned crates that were not needed: annotations live in libreri-library; each model
│      has its own crate — libreri-speech, libreri-maths, libreri-ocr, libreri-voices.)
├─ packages/config/       shared tsconfig, eslint, tailwind presets
├─ sidecars/              DjVuLibre per platform
├─ assets/  docs/adr/  tests/e2e/  scripts/  .github/workflows/
└─ Cargo.toml  pnpm-workspace.yaml  rust-toolchain.toml  README.md  LICENSE (TBD)
```

## Flows
- **Open book:** UI → reader feature → `lib/ipc.openBook` → `commands/reader.rs` → `libreri-library::open(bookId, profile)` → `libreri-db` (record + position) → `BookHandle` → renderer chosen by format → bytes streamed via `book://file/{token}` (range requests) → page at saved position.
- **Import:** drop → `api.importFiles` → `commands/library.rs::import_files` → `libreri-jobs` queue → BLAKE3 hash → duplicate check (skip/replace/keep both) → move/copy into `Books/` → `libreri-formats` extract → `libreri-thumbs` → `libreri-db` insert → JSON sidecar → optional online lookup. Progress + books-changed events update the UI.
- **Page images (5a):** `PageRenderer` asks `open_pages` for page count, sizes and outline → images load from `book://…/.pages/<bookId>/<page>?w=` → `libreri-library::pages` (CBZ read directly; CBR/CB7/CBT/CBA unpacked once; DjVu rendered by `ddjvu` at width steps) → per-computer page cache. DjVu text comes from `page_words` / `page_texts` (`djvutxt`).
- **Search (5b):** after each scan the indexer thread (`state.rs`) calls `Library::update_index` → `libreri-formats::text` per changed book (+ saved OCR text) → `libreri-search`. The search screen calls `search_text` / `search_in_book`; a result opens a tab with `findText`, and the reader goes to the page or chapter (`prepareFind`) and runs Find. *Make searchable* queues a job: `Library::make_searchable` renders pages (hayro / ddjvu), runs Tesseract, saves `.library-data/text/<id>.json` and re-indexes.
- **Read aloud (7a, natural voices in Phase 9):** `readers/speech/` (sentences, maths, `DomSpeech` for HTML, `WordSpeech` for pages with word boxes; `peek()` gives the next sentence) behind `Renderer.readAloud()` → `features/reader/speech/` (`useReadAloud` loop; `engine.ts`: Web Speech, eSpeak NG through `system_speak`, and natural voices through `speak_natural`, which makes each sentence as a WAV while the one before plays; `choose.ts` picks the voice for the book's language) → the floating player `features/reader/listening/ListenBar.tsx`.
- **Natural voices (ADR 0030):** Settings › Reader `sections/ReadAloudVoices.tsx` and `features/reader/speech/natural.ts` → `commands/voices.rs` (`natural_voices`, `piper_languages`, `piper_voices`, `download_voice`, `remove_voice`, `set_voice_on`, `speak_natural`) → `libreri-voices` (`runtime.rs` ONNX Runtime, `kokoro.rs`, `piper.rs`, `phonemes.rs`, `fetch.rs`, `wav.rs`).
- **OCR models (ADR 0029):** Settings › Helper programs `sections/OcrEngines.tsx` → `commands/ocr_models.rs` → `libreri-ocr`; *Make searchable* asks `ocr_models::reader` for the chosen model through `libreri-formats::ocr::PageReader`.
- **Podcasts (ADR 0028):** `features/podcasts/` (`PodcastsView`, `FindPodcastsDialog`, `player.ts` with the one `<audio>` of the window, `PodcastPlayer` floating or folded) → `commands/feeds.rs` with space `podcasts` → `libreri-feeds` (podcast search and episodes) and `Feeds/<profile>/.podcasts.json`.
- **Daily review (ADR 0032):** `features/review/` (`model.ts` cards from highlights, the queue and ts-fsrs scheduling, Anki notes; `store.ts` the profile's review document and `useCards`; `ReviewView` overview and session; `CardOptions` in the highlight menu) → `commands/study.rs` (`review_read`, `review_write`, `export_anki`) → `libreri-library/src/study.rs` (`<id>.review.json`) and `libreri-export/src/anki.rs` (`.apkg`).
- **Study (ADR 0031):** `features/study/` (`CalendarView` with month, week and day; `GoalDialog`; `TimerButton` in the reader's toolbar; `timer.ts` the timer; `store.ts` the calendar and where reading is; `StudyHost` in `AppShell` ticks the timer, shows the break card and mirrors the timer to the menu bar) → `commands/study.rs` (`study_read`, `study_write`, `save_calendar_file`, `timer_tray` with Tauri's tray and `set_progress_bar`) → `libreri-library/src/study.rs` (`.library-data/profiles/<id>.study.json`).
- **Print:** `features/reader/print/PrintDialog.tsx` draws PDF pages into a print-only part of the window, then `lib/print` → `print_window` (`commands/app.rs`).
- **Audiobooks (7a):** `ReaderView` opens audio files in `features/reader/listening/AudiobookView` (`<audio>` on `book://`, chapters from `audio_info` → `libreri-formats::audio`); links and sync points via `get_audio_link` / `set_audio_link` / `set_sync_points` → `libreri-library/src/listening.rs`; the `useListening` store connects the open audiobook and the open text (follow, add a sync point, listen from here).
- **Speech (7b):** `features/speech/` (`useRecorder`: microphone → 16 kHz PCM, `PauseDetector` for dictation; `DictateButton`; `useVoiceNote` + `VoiceNoteBar`; `voice.ts` for links and players in notes) → `commands/speech.rs` (models, `transcribe_pcm`, `save_voice_note`, `transcribe_voice_note`, `auto_sync_audiobook` job) → `libreri-speech` (whisper, decoding, FLAC, `sync`) and `libreri-library` (`voice.rs`: recordings in the notes folder; `listening.rs`: `sync_words`, `add_auto_points`). The loaded model is cached in `AppState`. Settings › Speech is `features/settings/components/sections/Speech.tsx`.
- **Writing (7c):** `features/spell/` (`field.ts`: `attachSpell` puts a layer over any `<textarea>` that draws wavy lines under misspelt words, plus the corrections menu and completion list; `useSpell` / `SpellTextarea` for React; markup text boxes through `MarkupEvents.onEditorOpen`) → `commands/spell.rs` (`spell_check`, `spell_suggest`, `spell_complete`, dictionaries, own words) with `spell_cache.rs` (loaded dictionaries, words of recent books and notes, learned in the background) → `libreri-spell` and `libreri-library/src/own_words.rs` (`Notes/<profile>/Dictionary.txt`, note texts, book words). Settings › Writing is `sections/Writing.tsx`.
- **Canvases (8a):** `features/canvas/` (`CanvasPanel` in the reader, `CanvasEditor` with the paper choice, *Clip from page* and *Ink to text*, `ExcalidrawHost` loaded lazily with saving and the Excalidraw calls, `paper.ts`) → `commands/canvas.rs` → `libreri-library/src/canvases.rs` (files in `Notes/<profile>/Canvases/`). Clipping is `Renderer.clipPicture` (`readers/clip.ts`, PDF and page renderers). Ink to text is `libreri-helpers/src/ink.rs` (macOS Vision via osascript, Windows OCR via PowerShell) or `libreri-formats::ocr::recognize_block` (Tesseract).
- **Compare (6c):** `features/reader/compare/` (CompareDialog, CompareView with side-by-side, overlay and change list) → `start_compare` (`commands/compare.rs`) → `AppState::start_compare` opens both sides (`Library::compare_doc`) and runs `Library::compare` as a job (words from `pdftext` / `djvu`, pairing and word diff in `libreri-pdf-edit::compare`, looks by rendering) → `CompareFinished` event → `get_comparison`. Pages come from `book://…/.compare/<id>/<a|b>/<page>`; `export_compare_report` → `libreri-pdf-edit::compare_report`.
- **Feeds (Phase 9, ADR 0027):** `features/feeds/` (`FeedsView` with the folder tree, item list and menus; `AddFeedDialog` with Address, arXiv, Suggested and OPML tabs; `ItemDialogs` with the PDF and Markdown preview and *Add to library*; `useFeedsBackground` in `AppShell` checks for new items and listens for `FeedsChanged`) → `commands/feeds.rs` (state in `Feeds/<profile>/.feeds.json` under `AppState.feeds_lock`; `feeds_refresh` reads feeds six at a time, then downloads for auto-download feeds; `feed_item_download`, `feed_item_to_library`) → `libreri-feeds` (`parse` with feed-rs, `fetch` with discovery and downloads, `state`, `opml`, `sources` for arXiv and suggestions, `article` for Markdown) and `libreri-library/src/feeds.rs` (the folder, `book://` access, adding to the library, profile rename and delete). Articles use `libreri-links::copy::readable`.
- **Paper notes (8b):** `features/reader/capture/` (`CaptureDialog` with camera, phone and pictures, the corner editor, clean-up and page order; `CaptureViewer`; `captureOf`) → `commands/capture.rs` (`capture_add`, `capture_add_file`, `capture_preview`, `capture_save` with Tesseract) → `libreri-scan/src/paper.rs` (detect, straighten, clean), `libreri-pdf-edit::pictures_pdf` and `libreri-library/src/captures.rs` (`Notes/<profile>/Captures/`). Notes are annotations of kind `capture`.
- **Maths (8b):** `readers/math/latex.ts` (`mathmlToLatex`, `textToLatex`), `RendererEvents.mathClick` from the document and ebook renderers, and `features/reader/components/MathPopover.tsx`. Reading from pictures is `features/reader/maths/api.ts` → `commands/maths.rs` → `libreri-maths` (`model.rs` the network, `prepare.rs` the picture, `tokens.rs` the LaTeX). Settings › Writing › Maths is in `sections/Writing.tsx`.
- **Links (8c):** `features/reader/weblinks/` (`AddLinkDialog`, `LinksPanel` for the Links tab with `Player`, `LinkCard`, `CopyViewer`, `model.ts` with `linkOf`) → `commands/links.rs` (`link_fetch`, `link_save`, `link_file`, `link_media_url`, `link_player`, `link_pop_out`, `link_open_file`) → `libreri-links` and `libreri-library/src/links.rs` (pictures and offline copies in the notes folder). `protocol.rs` serves `.media/<token>/…` for linked files and `.html` copies with a sandbox policy. Clicks on drawn annotations use `readers/types.ts::highlightAt`.
- **Edit pages (6b):** `features/reader/edit/` (model with undo, `PdfPages` thumbnails and text via PDF.js, `PageEditor` grid, `PageDetail` for crop/redact/correct, dialogs for inserting, new books, compression and version history) → `edit_pages` / `save_pages_as_book` / `save_filled_form` / `save_markup_into_pdf` (`commands/edit.rs`) → `Library::edit_pages` etc. (`libreri-library/src/edit.rs`) → `libreri-pdf-edit::apply` (redact.rs walks content streams, inspect.rs checks with hayro, edit.rs rebuilds pages) → `Library::save_version` (`versions.rs`: keeps the old file, changes the id, moves notes through the page map). Phone photos come from the `libreri-scan` phone page in *pages* mode.
- **Markup (6a):** `readers/markup/` (model, SVG rendering, `MarkupLayer` controller mounted by the PDF and page renderers on each page, export draw list) + `features/reader/markup/` (useMarkup, toolbar, panel, dialogs). Marks are saved with `save_annotation` (kind `markup`); *Export marked-up copy* sends the draw list to `export_marked_up` → `Library::export_marked_up` → `libreri-pdf-edit`.

## Where things go
| Adding… | Put it in | Not in |
|---|---|---|
| Button/dialog/screen | `features/<f>/components` | `components/ui` |
| New file format | `libreri-formats` + `readers/<format>` | reader feature |
| Business rule | owning service crate | Tauri command / React hook |
| Table or query | `libreri-db` migration + repository | SQL in other crates |
| Metadata source | `libreri-metadata/src/providers/` | lookup screen |
| Keyboard shortcut | `lib/shortcuts` registry (action ID) | component keydown listener |
| Setting | schema in core + `features/settings` | localStorage |
| Downloadable model | its own crate (`libreri-ocr`, `libreri-voices`, …) with download, remove and load, and a Settings group | installer |
| Fix for an older web view | `lib/polyfills.ts` | the feature that needs it |

## House rules
1. Dependencies point down only (CI: ESLint import boundaries, cargo dependency checks).
2. Commands are thin: validate → one service call → map error.
3. Rust types exported to TS automatically (tauri-specta); never duplicated by hand.
4. TanStack Query for data from Rust; Zustand for view state only.
5. Features import each other only via `index.ts`.
6. One `Error` enum per crate (thiserror); typed IPC errors with plain-language messages.
7. Personal data methods in `libreri-db` take an explicit `ProfileId`; there is no global current user. The signed-in profile belongs to the open `Library` session so the interface cannot read someone else's data by passing their id (ADR 0010).
8. Optional capabilities (OCR, handwriting, dictation, maths) are cargo features; models load at runtime.
9. Tests next to code (crate unit tests, Vitest), plus e2e for import/open/highlight/export.
10. rustfmt, clippy -D warnings, ESLint + Prettier, Conventional Commits, ADRs in `docs/adr`.

## UI typography rules (decided 2026-09-27)
- The app interface uses **one sans-serif family** (Geist, falling back to the system UI font) for every heading, label, button and menu. **No italic or serif headings** anywhere in the app UI.
- **Geist Mono** only for technical text: file paths, keyboard keys, ISBNs, LaTeX source, timestamps.
- Heading hierarchy comes from size and weight only (e.g. 20/600 page title, 15/600 section, 13/500 labels), defined once as tokens in `styles/tokens.css`.
- Serif, italic or handwriting fonts appear **only inside content**, never as UI:
  - book pages (the book's own fonts, or the reader font the user picks in Reader settings);
  - maths (italic variables are the normal convention, rendered by KaTeX / the PDF);
  - text the user writes with a handwriting-style font in Markup or Canvas;
  - generated placeholder covers for books without a cover image.
- The wireframes' serif/italic text is simulated book content, not a style to copy into the app chrome.
- **Look up and series (Phase 10):** `features/reader/components/LookupPopover.tsx` → `commands/reader.rs::look_up` → `libreri-metadata/src/lookup.rs` (Wiktionary definitions, Wikipedia summary). `features/library/series.ts` (grouping, order, next book) and `components/SeriesView.tsx`; nav kind `series` (with a name: that series in `LibraryView`).
