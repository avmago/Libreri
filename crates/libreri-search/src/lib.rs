//! The full-text search index: the words inside every book.
//!
//! The index is an SQLite FTS5 database in the app's cache folder on each
//! computer, one per library (user, 2026-09-28). It holds nothing that
//! cannot be read again from the books and their saved OCR text, so it is
//! never exported or backed up, and it rebuilds itself when it is missing,
//! damaged or from an older version.
//!
//! Each book is stored as pieces (a page, or part of a chapter) with where
//! they are, so a result can open the book at the match. Each book also has
//! a `stamp` saying which version of its text was indexed; the library
//! re-indexes a book when its stamp changes.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Bump to rebuild every index (the table layout or the text changed).
const SCHEMA_VERSION: i64 = 1;

pub type Result<T> = std::result::Result<T, String>;

fn err(e: rusqlite::Error) -> String {
    format!("the search index could not be used: {e}")
}

/// Whether a book's words can be searched.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextState {
    /// Every page (or the whole book) has text.
    Text,
    /// Some pages have text, some are scans without it.
    Partial,
    /// A scan without text: OCR can make it searchable.
    NoText,
    /// A format without words (comics, audio).
    NoWords,
    /// The text could not be read (damaged, DRM, a helper is missing).
    Failed,
}

/// What the index knows about one book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub stamp: String,
    pub state: TextState,
    /// Pages without text (for page-based books).
    pub empty_pages: u32,
    pub pages: u32,
    /// True when some text came from OCR.
    pub ocr: bool,
    pub message: Option<String>,
}

/// A piece of text to store.
#[derive(Debug, Clone, Default)]
pub struct Piece {
    pub page: Option<u32>,
    pub section: Option<u32>,
    pub label: Option<String>,
    pub text: String,
}

/// Part of a snippet: matched words are marked.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnippetPart {
    pub text: String,
    pub hit: bool,
}

/// One place in a book where the words were found.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Hit {
    pub page: Option<u32>,
    pub section: Option<u32>,
    pub label: Option<String>,
    pub snippet: Vec<SnippetPart>,
}

/// The matches in one book.
#[derive(Debug, Clone, PartialEq)]
pub struct BookHits {
    pub book_id: String,
    /// Pieces that match (pages, or parts of chapters).
    pub total: u32,
    /// The best few, in reading order.
    pub hits: Vec<Hit>,
    /// Lower is better (FTS5 bm25).
    pub score: f64,
}

/// Counts for Settings.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexCounts {
    pub books: u32,
    pub with_text: u32,
    pub partial: u32,
    pub no_text: u32,
    pub failed: u32,
}

/// Turns what the user typed into an FTS5 query. Words must all appear
/// (each also matches longer words starting with it), "quoted words" must
/// appear together, and `-word` leaves out pieces containing it.
pub fn fts_query(input: &str) -> Option<String> {
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    let mut rest = input.trim();
    let words = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect()
    };
    while !rest.is_empty() {
        rest = rest.trim_start();
        let negative = rest.starts_with('-');
        let body = rest.trim_start_matches('-');
        if let Some(inner) = body.strip_prefix('"') {
            let end = inner.find('"').unwrap_or(inner.len());
            let phrase = words(&inner[..end]);
            if !phrase.is_empty() {
                let q = format!("\"{}\"", phrase.join(" "));
                if negative { &mut exclude } else { &mut include }.push(q);
            }
            rest = inner.get(end + 1..).unwrap_or("");
        } else {
            let end = body.find(char::is_whitespace).unwrap_or(body.len());
            for w in words(&body[..end]) {
                let q = format!("\"{w}\"*");
                if negative { &mut exclude } else { &mut include }.push(q);
            }
            rest = &body[end..];
        }
    }
    if include.is_empty() {
        return None;
    }
    let mut q = include.join(" ");
    for e in exclude {
        q.push_str(" NOT ");
        q.push_str(&e);
    }
    Some(q)
}

const START: char = '\u{E000}';
const END: char = '\u{E001}';

fn snippet_parts(s: &str) -> Vec<SnippetPart> {
    let mut parts = Vec::new();
    let mut hit = false;
    let mut cur = String::new();
    for ch in s.chars() {
        if ch == START || ch == END {
            if !cur.is_empty() {
                parts.push(SnippetPart {
                    text: std::mem::take(&mut cur),
                    hit,
                });
            }
            hit = ch == START;
        } else {
            cur.push(if ch == '\n' { ' ' } else { ch });
        }
    }
    if !cur.is_empty() {
        parts.push(SnippetPart { text: cur, hit });
    }
    parts
}

fn state_name(s: TextState) -> &'static str {
    match s {
        TextState::Text => "text",
        TextState::Partial => "partial",
        TextState::NoText => "noText",
        TextState::NoWords => "noWords",
        TextState::Failed => "failed",
    }
}

fn state_from(s: &str) -> TextState {
    match s {
        "text" => TextState::Text,
        "partial" => TextState::Partial,
        "noText" => TextState::NoText,
        "noWords" => TextState::NoWords,
        _ => TextState::Failed,
    }
}

/// An open index.
pub struct SearchIndex {
    path: PathBuf,
    conn: Mutex<Connection>,
}

impl SearchIndex {
    /// Opens the index at `path`, creating it — or starting it again when it
    /// is damaged or from another version.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let conn = match Self::connect(path) {
            Ok(c) => c,
            Err(_) => {
                for suffix in ["", "-wal", "-shm"] {
                    let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
                }
                Self::connect(path)?
            }
        };
        Ok(Self {
            path: path.to_path_buf(),
            conn: Mutex::new(conn),
        })
    }

    fn connect(path: &Path) -> Result<Connection> {
        let conn = Connection::open(path).map_err(err)?;
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(err)?;
        conn.pragma_update(None, "synchronous", "NORMAL")
            .map_err(err)?;
        let version: i64 = conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .map_err(err)?;
        if version != SCHEMA_VERSION {
            conn.execute_batch(
                "DROP TABLE IF EXISTS books;
                 DROP TABLE IF EXISTS pieces;",
            )
            .map_err(err)?;
            conn.execute_batch(
                "CREATE TABLE books (
                    book_id TEXT PRIMARY KEY,
                    stamp TEXT NOT NULL,
                    state TEXT NOT NULL,
                    pages INTEGER NOT NULL DEFAULT 0,
                    empty_pages INTEGER NOT NULL DEFAULT 0,
                    ocr INTEGER NOT NULL DEFAULT 0,
                    message TEXT
                 );
                 CREATE VIRTUAL TABLE pieces USING fts5(
                    text,
                    book_id UNINDEXED,
                    page UNINDEXED,
                    section UNINDEXED,
                    label UNINDEXED,
                    tokenize = 'unicode61 remove_diacritics 2',
                    prefix = '2 3'
                 );",
            )
            .map_err(err)?;
            conn.pragma_update(None, "user_version", SCHEMA_VERSION)
                .map_err(err)?;
        }
        // Proves the file is readable.
        conn.query_row("SELECT count(*) FROM books", [], |r| r.get::<_, i64>(0))
            .map_err(err)?;
        Ok(conn)
    }

    fn conn(&self) -> std::sync::MutexGuard<'_, Connection> {
        self.conn.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Size of the index on disk, in bytes.
    pub fn size(&self) -> u64 {
        ["", "-wal"]
            .iter()
            .filter_map(|s| std::fs::metadata(format!("{}{s}", self.path.display())).ok())
            .map(|m| m.len())
            .sum()
    }

    /// Every indexed book.
    pub fn entries(&self) -> Result<HashMap<String, Entry>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare("SELECT book_id, stamp, state, pages, empty_pages, ocr, message FROM books")
            .map_err(err)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    Entry {
                        stamp: r.get(1)?,
                        state: state_from(&r.get::<_, String>(2)?),
                        pages: r.get(3)?,
                        empty_pages: r.get(4)?,
                        ocr: r.get::<_, i64>(5)? != 0,
                        message: r.get(6)?,
                    },
                ))
            })
            .map_err(err)?;
        rows.collect::<std::result::Result<_, _>>().map_err(err)
    }

    pub fn entry(&self, book: &str) -> Result<Option<Entry>> {
        self.conn()
            .query_row(
                "SELECT stamp, state, pages, empty_pages, ocr, message FROM books WHERE book_id = ?1",
                [book],
                |r| {
                    Ok(Entry {
                        stamp: r.get(0)?,
                        state: state_from(&r.get::<_, String>(1)?),
                        pages: r.get(2)?,
                        empty_pages: r.get(3)?,
                        ocr: r.get::<_, i64>(4)? != 0,
                        message: r.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(err)
    }

    /// Replaces a book's text.
    pub fn put(&self, book: &str, entry: &Entry, pieces: &[Piece]) -> Result<()> {
        let mut conn = self.conn();
        let tx = conn.transaction().map_err(err)?;
        tx.execute("DELETE FROM pieces WHERE book_id = ?1", [book])
            .map_err(err)?;
        {
            let mut insert = tx
                .prepare(
                    "INSERT INTO pieces (text, book_id, page, section, label)
                     VALUES (?1, ?2, ?3, ?4, ?5)",
                )
                .map_err(err)?;
            for p in pieces {
                insert
                    .execute(params![p.text, book, p.page, p.section, p.label])
                    .map_err(err)?;
            }
        }
        tx.execute(
            "INSERT OR REPLACE INTO books (book_id, stamp, state, pages, empty_pages, ocr, message)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                book,
                entry.stamp,
                state_name(entry.state),
                entry.pages,
                entry.empty_pages,
                entry.ocr as i64,
                entry.message
            ],
        )
        .map_err(err)?;
        tx.commit().map_err(err)
    }

    pub fn remove(&self, book: &str) -> Result<()> {
        let conn = self.conn();
        conn.execute("DELETE FROM pieces WHERE book_id = ?1", [book])
            .map_err(err)?;
        conn.execute("DELETE FROM books WHERE book_id = ?1", [book])
            .map_err(err)?;
        Ok(())
    }

    /// Forgets books that are no longer in the library.
    pub fn retain(&self, keep: &HashSet<String>) -> Result<u32> {
        let gone: Vec<String> = self
            .entries()?
            .into_keys()
            .filter(|id| !keep.contains(id))
            .collect();
        for id in &gone {
            self.remove(id)?;
        }
        Ok(gone.len() as u32)
    }

    /// Empties the index (it fills again in the background).
    pub fn clear(&self) -> Result<()> {
        let conn = self.conn();
        conn.execute_batch("DELETE FROM pieces; DELETE FROM books; VACUUM;")
            .map_err(err)
    }

    pub fn counts(&self) -> Result<IndexCounts> {
        let mut c = IndexCounts::default();
        for e in self.entries()?.values() {
            c.books += 1;
            match e.state {
                TextState::Text => c.with_text += 1,
                TextState::Partial => c.partial += 1,
                TextState::NoText => c.no_text += 1,
                TextState::Failed => c.failed += 1,
                TextState::NoWords => {}
            }
        }
        Ok(c)
    }

    /// Books whose text matches `query`, best first. `allowed` limits the
    /// books (what the signed-in profile may see); `per_book` is how many
    /// places to show for each.
    pub fn search(
        &self,
        query: &str,
        allowed: &dyn Fn(&str) -> bool,
        limit: usize,
        per_book: usize,
    ) -> Result<Vec<BookHits>> {
        let Some(q) = fts_query(query) else {
            return Ok(Vec::new());
        };
        let mut books: Vec<(String, u32, f64)> = {
            let conn = self.conn();
            let mut stmt = conn
                .prepare(
                    "SELECT book_id, count(*), min(rank) FROM pieces
                     WHERE pieces MATCH ?1 GROUP BY book_id",
                )
                .map_err(err)?;
            let rows = stmt
                .query_map([&q], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .map_err(err)?;
            rows.filter_map(std::result::Result::ok)
                .filter(|(id, _, _): &(String, u32, f64)| allowed(id))
                .collect()
        };
        books.sort_by(|a, b| a.2.total_cmp(&b.2));
        books.truncate(limit);
        books
            .into_iter()
            .map(|(book_id, total, score)| {
                let hits = self.hits_with(&q, &book_id, per_book)?;
                Ok(BookHits {
                    book_id,
                    total,
                    hits,
                    score,
                })
            })
            .collect()
    }

    /// Every place in one book (up to `limit`), in reading order.
    pub fn book_hits(&self, query: &str, book: &str, limit: usize) -> Result<Vec<Hit>> {
        match fts_query(query) {
            Some(q) => self.hits_with(&q, book, limit),
            None => Ok(Vec::new()),
        }
    }

    fn hits_with(&self, q: &str, book: &str, limit: usize) -> Result<Vec<Hit>> {
        let conn = self.conn();
        let mut stmt = conn
            .prepare(
                "SELECT page, section, label, rowid,
                        snippet(pieces, 0, char(57344), char(57345), '…', 24)
                 FROM pieces WHERE pieces MATCH ?1 AND book_id = ?2
                 ORDER BY rank LIMIT ?3",
            )
            .map_err(err)?;
        let mut rows: Vec<(i64, Hit)> = stmt
            .query_map(params![q, book, limit as i64], |r| {
                Ok((
                    r.get::<_, i64>(3)?,
                    Hit {
                        page: r.get(0)?,
                        section: r.get(1)?,
                        label: r.get(2)?,
                        snippet: snippet_parts(&r.get::<_, String>(4)?),
                    },
                ))
            })
            .map_err(err)?
            .filter_map(std::result::Result::ok)
            .collect();
        // Reading order: pieces were stored in order.
        rows.sort_by_key(|(rowid, _)| *rowid);
        Ok(rows.into_iter().map(|(_, h)| h).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(stamp: &str, state: TextState) -> Entry {
        Entry {
            stamp: stamp.into(),
            state,
            empty_pages: 0,
            pages: 2,
            ocr: false,
            message: None,
        }
    }

    fn page(n: u32, text: &str) -> Piece {
        Piece {
            page: Some(n),
            text: text.into(),
            ..Default::default()
        }
    }

    #[test]
    fn queries() {
        assert_eq!(fts_query("light house").unwrap(), "\"light\"* \"house\"*");
        assert_eq!(
            fts_query("\"edge of glass\" -prism").unwrap(),
            "\"edge of glass\" NOT \"prism\"*"
        );
        assert_eq!(fts_query("naïve's").unwrap(), "\"naïve\"* \"s\"*");
        assert_eq!(fts_query("-only"), None);
        assert_eq!(fts_query("  "), None);
        assert_eq!(fts_query("\"unclosed quote").unwrap(), "\"unclosed quote\"");
    }

    #[test]
    fn finds_words_in_books() {
        let dir = tempfile::tempdir().unwrap();
        let idx = SearchIndex::open(&dir.path().join("i.sqlite")).unwrap();
        idx.put(
            "a",
            &entry("1", TextState::Text),
            &[
                page(1, "The lighthouse keeper wrote every night."),
                page(2, "Storms came; the keeper stayed."),
            ],
        )
        .unwrap();
        idx.put(
            "b",
            &entry("1", TextState::Text),
            &[page(1, "A café by the harbour; the keeper's daughter.")],
        )
        .unwrap();
        let all = |_: &str| true;
        let r = idx.search("keeper", &all, 10, 3).unwrap();
        assert_eq!(r.len(), 2);
        let a = r.iter().find(|b| b.book_id == "a").unwrap();
        assert_eq!(a.total, 2);
        assert_eq!(a.hits[0].page, Some(1));
        assert!(a.hits[0]
            .snippet
            .iter()
            .any(|p| p.hit && p.text == "keeper"));
        // Accents do not matter; prefixes match.
        assert_eq!(idx.search("cafe", &all, 10, 3).unwrap().len(), 1);
        assert_eq!(idx.search("lightho", &all, 10, 3).unwrap().len(), 1);
        assert_eq!(
            idx.search("\"keeper wrote\"", &all, 10, 3).unwrap().len(),
            1
        );
        assert_eq!(idx.search("keeper -storms", &all, 10, 3).unwrap().len(), 2);
        let only_b = |id: &str| id == "b";
        assert_eq!(idx.search("keeper", &only_b, 10, 3).unwrap().len(), 1);
        assert_eq!(idx.book_hits("keeper", "a", 10).unwrap().len(), 2);

        // Replacing and forgetting.
        idx.put("a", &entry("2", TextState::Text), &[page(1, "Nothing")])
            .unwrap();
        assert_eq!(idx.entry("a").unwrap().unwrap().stamp, "2");
        assert_eq!(idx.search("keeper", &all, 10, 3).unwrap().len(), 1);
        let keep: HashSet<String> = ["a".to_owned()].into();
        assert_eq!(idx.retain(&keep).unwrap(), 1);
        assert!(idx.search("keeper", &all, 10, 3).unwrap().is_empty());
        assert_eq!(idx.counts().unwrap().books, 1);
    }

    #[test]
    fn damaged_index_starts_again() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("i.sqlite");
        std::fs::write(&path, b"this is not a database at all, not even close").unwrap();
        let idx = SearchIndex::open(&path).unwrap();
        assert!(idx.entries().unwrap().is_empty());
    }
}
