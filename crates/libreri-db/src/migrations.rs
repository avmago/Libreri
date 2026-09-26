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
