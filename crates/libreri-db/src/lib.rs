//! SQLite storage for one library.
//!
//! All SQL lives in this crate. Other crates use [`Database`] and the
//! repository functions it exposes; they never write SQL themselves.

mod migrations;

use rusqlite::{Connection, OpenFlags};
use std::path::Path;

pub use migrations::{latest_version, MIGRATIONS};

/// Errors from the storage layer.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    #[error("this library was created by a newer version of Libreri (database version {found}, this app supports up to {supported})")]
    TooNew { found: u32, supported: u32 },
}

pub type Result<T> = std::result::Result<T, Error>;

/// An open library database.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Opens (creating if needed) the database file and brings its schema up
    /// to date.
    pub fn open(path: &Path) -> Result<Self> {
        let conn = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        Self::init(conn)
    }

    /// Opens a private in-memory database, for tests.
    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self> {
        // WAL keeps reads fast while writing; the lock file in
        // `libreri-library` guarantees a single writer, and `close()`
        // checkpoints so synced folders only ever see one clean file.
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "busy_timeout", 5000)?;
        let mut db = Self { conn };
        migrations::run(&mut db.conn)?;
        Ok(db)
    }

    /// Current schema version (SQLite `user_version`).
    pub fn schema_version(&self) -> Result<u32> {
        Ok(self
            .conn
            .query_row("PRAGMA user_version", [], |r| r.get(0))?)
    }

    /// Reads a value from the `library_meta` key/value table.
    pub fn meta_get(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self
            .conn
            .prepare_cached("SELECT value FROM library_meta WHERE key = ?1")?;
        let mut rows = stmt.query([key])?;
        Ok(match rows.next()? {
            Some(row) => Some(row.get(0)?),
            None => None,
        })
    }

    /// Writes a value to the `library_meta` key/value table.
    pub fn meta_set(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO library_meta (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            [key, value],
        )?;
        Ok(())
    }

    /// Number of books in the library.
    pub fn book_count(&self) -> Result<u64> {
        Ok(self
            .conn
            .query_row("SELECT COUNT(*) FROM books", [], |r| r.get(0))?)
    }

    /// Flushes the write-ahead log into the main file and closes the
    /// connection.
    pub fn close(self) -> Result<()> {
        self.conn
            .pragma_update(None, "wal_checkpoint", "TRUNCATE")?;
        self.conn.close().map_err(|(_, e)| Error::Sqlite(e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_bring_a_new_database_to_latest() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), latest_version());
        assert_eq!(db.book_count().unwrap(), 0);
    }

    #[test]
    fn meta_round_trip_and_overwrite() {
        let db = Database::open_in_memory().unwrap();
        assert_eq!(db.meta_get("name").unwrap(), None);
        db.meta_set("name", "MyLibrary").unwrap();
        db.meta_set("name", "Renamed").unwrap();
        assert_eq!(db.meta_get("name").unwrap().as_deref(), Some("Renamed"));
    }

    #[test]
    fn reopening_a_file_keeps_data_and_does_not_rerun_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        let db = Database::open(&path).unwrap();
        db.meta_set("k", "v").unwrap();
        db.close().unwrap();

        let db = Database::open(&path).unwrap();
        assert_eq!(db.meta_get("k").unwrap().as_deref(), Some("v"));
        assert_eq!(db.schema_version().unwrap(), latest_version());
    }

    #[test]
    fn refuses_a_database_from_a_newer_app() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("library.db");
        {
            let conn = Connection::open(&path).unwrap();
            conn.pragma_update(None, "user_version", latest_version() + 1)
                .unwrap();
        }
        match Database::open(&path) {
            Err(Error::TooNew { found, .. }) => assert_eq!(found, latest_version() + 1),
            other => panic!("expected TooNew, got {:?}", other.map(|_| ())),
        }
    }
}
