//! Forward-only schema migrations, tracked with SQLite's `user_version`.
//!
//! Rules: never edit a migration that has shipped; add a new one instead.
//! Each migration runs inside a transaction.

use crate::{Error, Result};
use rusqlite::Connection;

/// Migrations in order. Index 0 upgrades version 0 → 1, and so on.
pub const MIGRATIONS: &[&str] = &[
    // 1 — foundations (Phase 0). Later phases add their own tables.
    r#"
    CREATE TABLE library_meta (
        key   TEXT PRIMARY KEY,
        value TEXT NOT NULL
    ) STRICT;

    CREATE TABLE profiles (
        id          TEXT PRIMARY KEY,            -- UUID
        name        TEXT NOT NULL,
        colour      TEXT NOT NULL DEFAULT 'graphite',
        pin_hash    TEXT,                        -- Argon2 hash of the 6-digit PIN, NULL = no PIN
        is_owner    INTEGER NOT NULL DEFAULT 0,
        created_at  TEXT NOT NULL
    ) STRICT;

    CREATE TABLE books (
        id          TEXT PRIMARY KEY,            -- BLAKE3 content hash (hex)
        rel_path    TEXT NOT NULL UNIQUE,        -- path relative to the library root
        title       TEXT NOT NULL,
        file_type   TEXT NOT NULL,
        file_size   INTEGER NOT NULL,
        added_at    TEXT NOT NULL
    ) STRICT;
    "#,
    // 2 — library MVP (Phase 1): full book records, tags, categories,
    // per-profile state and metadata search. `books` was always empty in v1.
    r#"
    DROP TABLE books;

    CREATE TABLE books (
        id            TEXT PRIMARY KEY,          -- BLAKE3 content hash (hex)
        rel_path      TEXT NOT NULL UNIQUE,      -- "Books/…", '/' separators
        file_type     TEXT NOT NULL,
        file_size     INTEGER NOT NULL,
        file_mtime    INTEGER NOT NULL DEFAULT 0, -- seconds; skips re-hashing unchanged files
        has_cover     INTEGER NOT NULL DEFAULT 0,
        missing       INTEGER NOT NULL DEFAULT 0,
        added_at      TEXT NOT NULL,
        modified_at   TEXT NOT NULL,
        title         TEXT NOT NULL,
        sort_title    TEXT NOT NULL,
        subtitle      TEXT,
        authors       TEXT NOT NULL DEFAULT '[]', -- JSON array
        sort_author   TEXT NOT NULL DEFAULT '',
        contributors  TEXT NOT NULL DEFAULT '[]', -- JSON array
        about         TEXT,
        year          INTEGER,
        publisher     TEXT,
        pages         INTEGER,
        isbn13        TEXT,
        isbn10        TEXT,
        edition       TEXT,
        language      TEXT,
        content_type  TEXT NOT NULL DEFAULT 'book',
        series        TEXT,
        series_number REAL,
        doi           TEXT,
        arxiv_id      TEXT,
        journal       TEXT,
        volume        TEXT,
        issue         TEXT,
        url           TEXT
    ) STRICT;
    CREATE INDEX books_isbn13 ON books(isbn13);

    -- Earlier ids of a book whose file changed, so old links still resolve.
    CREATE TABLE book_aliases (
        old_id   TEXT PRIMARY KEY,
        book_id  TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE ON UPDATE CASCADE
    ) STRICT;

    CREATE TABLE tags (
        id    INTEGER PRIMARY KEY,
        name  TEXT NOT NULL UNIQUE COLLATE NOCASE
    ) STRICT;
    CREATE TABLE book_tags (
        book_id  TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE ON UPDATE CASCADE,
        tag_id   INTEGER NOT NULL REFERENCES tags(id) ON DELETE CASCADE,
        PRIMARY KEY (book_id, tag_id)
    ) STRICT;

    CREATE TABLE categories (
        id    INTEGER PRIMARY KEY,
        path  TEXT NOT NULL UNIQUE COLLATE NOCASE  -- "Science/Physics"
    ) STRICT;
    CREATE TABLE book_categories (
        book_id      TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE ON UPDATE CASCADE,
        category_id  INTEGER NOT NULL REFERENCES categories(id) ON DELETE CASCADE,
        PRIMARY KEY (book_id, category_id)
    ) STRICT;

    CREATE TABLE book_user (
        book_id      TEXT NOT NULL REFERENCES books(id) ON DELETE CASCADE ON UPDATE CASCADE,
        profile_id   TEXT NOT NULL REFERENCES profiles(id) ON DELETE CASCADE,
        status       TEXT NOT NULL DEFAULT 'none',
        rating       INTEGER NOT NULL DEFAULT 0,
        favorite     INTEGER NOT NULL DEFAULT 0,
        progress     REAL NOT NULL DEFAULT 0,
        last_opened  TEXT,
        PRIMARY KEY (book_id, profile_id)
    ) STRICT;

    CREATE VIRTUAL TABLE books_fts USING fts5(
        book_id UNINDEXED,
        title, authors, about, tags, categories, publisher, series, identifiers,
        tokenize = 'unicode61 remove_diacritics 2',
        prefix = '2 3'
    );
    "#,
];

/// Schema version this build of Libreri writes.
pub fn latest_version() -> u32 {
    MIGRATIONS.len() as u32
}

pub(crate) fn run(conn: &mut Connection) -> Result<()> {
    let current: u32 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let latest = latest_version();
    if current > latest {
        return Err(Error::TooNew {
            found: current,
            supported: latest,
        });
    }
    for (index, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", index as u32 + 1)?;
        tx.commit()?;
    }
    Ok(())
}
