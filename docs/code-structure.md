# Libreri — Source code structure (draft 1, 2026-09-26)

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
│     ├─ features/       library reader notes profiles details portability search organize
│     │                  markup page-editor compare audio voice canvas capture links stats settings
│     │                  (each: components/ hooks/ store.ts api.ts types.ts index.ts)
│     ├─ readers/        types.ts (Renderer interface) + pdf/ ebook/ markdown/ comic/ djvu/
│     ├─ components/     ui/ (shadcn, generated) · common/ (BookCover, TagChip, Kbd…)
│     ├─ lib/            ipc/ (tauri-specta bindings) · shortcuts/ · i18n/ · theme/ · utils/
│     ├─ styles/         tokens.css, themes, reader themes
│     └─ main.tsx
├─ crates/
│  ├─ libreri-core        domain types, IDs, errors, ports
│  ├─ libreri-db          SQLite, migrations, repositories, FTS5
│  ├─ libreri-library     library folder, lock, scan, watcher, import/move, hashing, sidecars, versions
│  ├─ libreri-formats     extractors: pdf, epub, mobi, fb2, txt, md, comics (zip/rar/7z/tar/ace), djvu (pages, text layer, outline), audio
│  ├─ libreri-thumbs      thumbnails, covers
│  ├─ libreri-metadata    online details: sources (Open Library, Google Books, Crossref, OpenAlex, Semantic Scholar, arXiv, ComicVine, ISBNdb), scoring, tag/category tidying, cover download
│  ├─ libreri-helpers    helper programs (DjVuLibre, Tesseract, unar): find, version, install with the package manager, run
│  ├─ libreri-scan        barcode reading (rxing) and the one-time phone page for scanning
│  ├─ libreri-annotations highlights, comments, ink, voice notes, links, anchors
│  ├─ libreri-profiles    6-digit PIN rules, Argon2 hashing, lockout, recovery codes
│  ├─ libreri-search      full-text index
│  ├─ libreri-export      formats: CSV/XLSX/JSON, BibTeX/RIS/CSL-JSON, citation styles, Obsidian notes, Calibre OPF, Libreri archives (zip + manifest); importers from other apps (4c)
│  ├─ libreri-pdf-edit    page ops, redaction, compare, versions
│  ├─ libreri-speech      TTS, dictation, transcription
│  ├─ libreri-recognition OCR, handwriting, maths (optional cargo features)
│  ├─ libreri-models      optional model downloads, checksums
│  └─ libreri-jobs        background queue, progress, cancellation
├─ packages/config/       shared tsconfig, eslint, tailwind presets
├─ sidecars/              DjVuLibre per platform
├─ assets/  docs/adr/  tests/e2e/  scripts/  .github/workflows/
└─ Cargo.toml  pnpm-workspace.yaml  rust-toolchain.toml  README.md  LICENSE (TBD)
```

## Flows
- **Open book:** UI → reader feature → `lib/ipc.openBook` → `commands/reader.rs` → `libreri-library::open(bookId, profile)` → `libreri-db` (record + position) → `BookHandle` → renderer chosen by format → bytes streamed via `book://file/{token}` (range requests) → page at saved position.
- **Import:** drop → `api.importFiles` → `commands/library.rs::import_files` → `libreri-jobs` queue → BLAKE3 hash → duplicate check (skip/replace/keep both) → move/copy into `Books/` → `libreri-formats` extract → `libreri-thumbs` → `libreri-db` insert → JSON sidecar → optional online lookup. Progress + books-changed events update the UI.
- **Page images (5a):** `PageRenderer` asks `open_pages` for page count, sizes and outline → images load from `book://…/.pages/<bookId>/<page>?w=` → `libreri-library::pages` (CBZ read directly; CBR/CB7/CBT/CBA unpacked once; DjVu rendered by `ddjvu` at width steps) → per-computer page cache. DjVu text comes from `page_words` / `page_texts` (`djvutxt`).

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
| Downloadable model | `libreri-models` + cargo feature | installer |

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
