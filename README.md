# Libreri

A local-first library for PDFs, ebooks, Markdown, comics, DjVu and audiobooks — with notes, highlights, handwriting and read-aloud — for macOS, Windows and Linux.

Your library is an ordinary folder: books stay as plain files, notes are Markdown, and everything can be exported and moved to another computer with every link intact.

> **Status:** Phase 0 (foundations) — you can create and open libraries. Importing books arrives in Phase 1. See [docs/development-phases.md](docs/development-phases.md).

## Run it

Requirements:

- [Rust](https://rustup.rs) (stable, 1.89 or newer)
- [Node.js](https://nodejs.org) 20+ and [pnpm](https://pnpm.io) 10 (`corepack enable`)
- Platform prerequisites for Tauri: <https://v2.tauri.app/start/prerequisites/>
  - macOS: Xcode Command Line Tools (`xcode-select --install`)
  - Linux: `libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev`
  - Windows: Microsoft C++ Build Tools and WebView2 (preinstalled on Windows 11)

```sh
pnpm install
pnpm dev          # starts the app with hot reload
```

Other commands:

| Command | What it does |
|---|---|
| `pnpm build` | Builds installers for your platform |
| `pnpm test` | Frontend tests (Vitest) and Rust tests |
| `pnpm lint` | ESLint, TypeScript and Clippy |
| `pnpm format` | Prettier and rustfmt |
| `pnpm gen:ipc` | Regenerates TypeScript types from the Rust commands |

## How the code is organised

```
apps/desktop/src        React UI, one folder per feature
apps/desktop/src-tauri  Thin Tauri shell: commands, events, book:// protocol
crates/libreri-core     Domain types, IDs, library layout (no dependencies on the app)
crates/libreri-db       SQLite and migrations
crates/libreri-library  Create, open and lock library folders
crates/libreri-jobs     Background job queue with progress and cancel
docs/                   Architecture, phases, decisions (docs/adr)
```

Dependencies only point downward (UI → shell → crates → core). Read [docs/code-structure.md](docs/code-structure.md) before adding code, and [docs/data-portability.md](docs/data-portability.md) before touching anything that links notes to books.

## Licence

Not decided yet (see [docs/adr/0006-licence-deferred.md](docs/adr/0006-licence-deferred.md)). All rights reserved until then.
