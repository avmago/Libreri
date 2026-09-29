//! Export, import and health checks: aliases, finding a book by its
//! identifiers, database snapshots and consistency checks.

use crate::{Database, Result};
use libreri_core::{Alias, AliasKind, BookId, ProfileId};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

/// What a database snapshot keeps.
#[derive(Debug, Clone, Default)]
pub struct SnapshotScope {
    /// Keep only this profile's personal data (others are removed with
    /// everything they own). `None` keeps everyone's.
    pub only_profile: Option<ProfileId>,
    /// Remove PIN and recovery-code hashes.
    pub strip_pins: bool,
    /// Keep no personal data at all: every profile is removed, and with it
    /// reading status, positions, highlights, notebooks, collections and
    /// sign-in state. Only the catalogue of books stays.
    pub no_personal_data: bool,
}

/// A book with the given title: (id, authors, page count).
pub type TitledBook = (BookId, Vec<String>, Option<u32>);

fn book_id(s: String) -> Option<BookId> {
    BookId::from_hex(s).ok()
}

impl Database {
    /// Earlier ids of a book.
    pub fn aliases_of(&self, book: &BookId) -> Result<Vec<Alias>> {
        let mut stmt = self.conn.prepare_cached(
            "SELECT old_id, kind FROM book_aliases WHERE book_id=?1 ORDER BY old_id",
        )?;
        let rows = stmt.query_map([book.as_str()], |r| {
            Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, kind) = row?;
            if let Some(id) = book_id(id) {
                out.push(Alias {
                    id,
                    kind: AliasKind::parse(&kind),
                });
            }
        }
        Ok(out)
    }

    /// Makes `old` another name of `book`. Ignored when `old` is the id of a
    /// book in the library (a real book always wins over an alias).
    pub fn add_alias(&self, old: &BookId, book: &BookId, kind: AliasKind) -> Result<()> {
        if old == book {
            return Ok(());
        }
        let is_book: bool = self.conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM books WHERE id=?1)",
            [old.as_str()],
            |r| r.get(0),
        )?;
        if !is_book {
            self.conn.execute(
                "INSERT OR REPLACE INTO book_aliases(old_id, book_id, kind) VALUES (?1, ?2, ?3)",
                params![old.as_str(), book.as_str(), kind.as_str()],
            )?;
        }
        Ok(())
    }

    /// Every book id in the library.
    pub fn book_ids(&self) -> Result<Vec<BookId>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM books ORDER BY sort_title")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter_map(book_id)
            .collect())
    }

    /// A book with any of these identifiers (ISBNs compared as stored:
    /// digits only; DOIs without case).
    pub fn find_book_by_identifiers(
        &self,
        isbn13: Option<&str>,
        isbn10: Option<&str>,
        doi: Option<&str>,
        arxiv: Option<&str>,
    ) -> Result<Option<BookId>> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM books WHERE
                    (?1 IS NOT NULL AND isbn13 = ?1) OR (?2 IS NOT NULL AND isbn10 = ?2)
                    OR (?3 IS NOT NULL AND lower(doi) = lower(?3))
                    OR (?4 IS NOT NULL AND arxiv_id = ?4)
                 ORDER BY missing, added_at LIMIT 1",
                params![isbn13, isbn10, doi, arxiv],
                |r| r.get(0),
            )
            .optional()?;
        Ok(found.and_then(book_id))
    }

    /// Books whose title is `title` (ignoring case and outer spaces), with
    /// their authors (JSON list) and page count.
    pub fn books_titled(&self, title: &str) -> Result<Vec<TitledBook>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, authors, pages FROM books
             WHERE lower(trim(title)) = lower(trim(?1)) ORDER BY missing, added_at",
        )?;
        let rows = stmt.query_map([title], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, authors, pages) = row?;
            if let Some(id) = book_id(id) {
                out.push((
                    id,
                    serde_json::from_str(&authors).unwrap_or_default(),
                    pages.and_then(|p| u32::try_from(p).ok()),
                ));
            }
        }
        Ok(out)
    }

    /// Every notebook row: (book, profile, notebook path).
    pub fn notebook_rows(&self) -> Result<Vec<(BookId, ProfileId, String)>> {
        let mut stmt = self
            .conn
            .prepare("SELECT book_id, profile_id, rel_path FROM notebooks")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (b, p, rel) = row?;
            if let (Some(b), Ok(p)) = (book_id(b), p.parse()) {
                out.push((b, p, rel));
            }
        }
        Ok(out)
    }

    pub fn delete_notebook_row(&self, book: &BookId, profile: &ProfileId) -> Result<()> {
        self.conn.execute(
            "DELETE FROM notebooks WHERE book_id=?1 AND profile_id=?2",
            params![book.as_str(), profile.to_string()],
        )?;
        Ok(())
    }

    /// Books that have notes brought over from another copy or edition,
    /// with how many notes they have (all profiles).
    pub fn books_with_other_file_notes(&self) -> Result<Vec<(BookId, u32)>> {
        let mut stmt = self.conn.prepare(
            "SELECT b.id, (SELECT COUNT(*) FROM annotations a WHERE a.book_id = b.id)
             FROM books b
             WHERE EXISTS(SELECT 1 FROM book_aliases x WHERE x.book_id = b.id AND x.kind = 'otherFile')
             ORDER BY b.sort_title",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, n) = row?;
            if let Some(id) = book_id(id) {
                out.push((id, n.max(0) as u32));
            }
        }
        Ok(out)
    }

    /// Groups of books that share an ISBN or a DOI.
    pub fn books_sharing_identifiers(&self) -> Result<Vec<(String, Vec<BookId>)>> {
        let mut out = Vec::new();
        for (label, column) in [
            ("ISBN", "isbn13"),
            ("ISBN", "isbn10"),
            ("DOI", "lower(doi)"),
        ] {
            let mut stmt = self.conn.prepare(&format!(
                "SELECT {column}, group_concat(id) FROM books WHERE {column} IS NOT NULL
                 AND {column} <> '' GROUP BY {column} HAVING COUNT(*) > 1"
            ))?;
            let rows =
                stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
            for row in rows {
                let (value, ids) = row?;
                let ids: Vec<BookId> = ids
                    .split(',')
                    .filter_map(|s| BookId::from_hex(s).ok())
                    .collect();
                let key = format!("{label} {value}");
                if ids.len() > 1
                    && !out.iter().any(|(_, seen): &(String, Vec<BookId>)| {
                        ids.iter().all(|i| seen.contains(i))
                    })
                {
                    out.push((key, ids));
                }
            }
        }
        Ok(out)
    }

    /// Problems SQLite finds in the file, and books missing from the search
    /// index. Empty when all is well.
    pub fn check(&self) -> Result<Vec<String>> {
        let mut problems = Vec::new();
        let mut stmt = self.conn.prepare("PRAGMA quick_check")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        for row in rows {
            let line = row?;
            if line != "ok" {
                problems.push(line);
            }
        }
        let unindexed: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM books WHERE id NOT IN (SELECT book_id FROM books_fts)",
            [],
            |r| r.get(0),
        )?;
        if unindexed > 0 {
            problems.push(format!(
                "{unindexed} books are missing from the search index"
            ));
        }
        Ok(problems)
    }

    /// Writes a copy of the database to `dest` (which must not exist),
    /// keeping only what `scope` allows.
    pub fn snapshot(&self, dest: &Path, scope: &SnapshotScope) -> Result<()> {
        self.conn
            .execute("VACUUM INTO ?1", [dest.to_string_lossy().as_ref()])?;
        let conn = Connection::open(dest)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        if let Some(keep) = &scope.only_profile {
            let keep = keep.to_string();
            conn.execute("DELETE FROM profiles WHERE id <> ?1", [&keep])?;
            conn.execute(
                "DELETE FROM library_meta WHERE key LIKE 'session:%' AND key <> ?1",
                [format!("session:{keep}")],
            )?;
        }
        if scope.no_personal_data {
            // Deleting the profiles cascades to book_user, annotations,
            // notebooks and collections; the explicit deletes make sure.
            conn.execute_batch(
                "DELETE FROM annotations; DELETE FROM notebooks; DELETE FROM book_user;
                 DELETE FROM collections; DELETE FROM profiles;
                 DELETE FROM library_meta WHERE key LIKE 'session:%';",
            )?;
        }
        if scope.strip_pins {
            conn.execute(
                "UPDATE profiles SET pin_hash = NULL, recovery_hash = NULL,
                    failed_pins = 0, locked_until = 0",
                [],
            )?;
        }
        conn.execute_batch("VACUUM")?;
        conn.close().map_err(|(_, e)| crate::Error::Sqlite(e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libreri_core::{Book, BookMetadata, BookUserState, FileType};

    fn id(c: char) -> BookId {
        BookId::from_hex(c.to_string().repeat(64)).unwrap()
    }

    fn book(c: char, title: &str, isbn: Option<&str>) -> Book {
        Book {
            id: id(c),
            rel_path: format!("Books/{title}.pdf"),
            file_type: FileType::Pdf,
            file_size: 1,
            has_cover: false,
            missing: false,
            added_at: "2026-01-01T00:00:00Z".into(),
            modified_at: "2026-01-01T00:00:00Z".into(),
            metadata: BookMetadata {
                title: title.into(),
                authors: vec!["Jane Smith".into()],
                isbn13: isbn.map(str::to_owned),
                ..Default::default()
            },
            user: BookUserState::default(),
        }
    }

    #[test]
    fn aliases_identifiers_and_duplicates() {
        let db = Database::open_in_memory().unwrap();
        db.insert_book(&book('a', "Optics", Some("9780131103627")), 0)
            .unwrap();
        db.insert_book(&book('b', "Optics again", Some("9780131103627")), 0)
            .unwrap();

        db.add_alias(&id('c'), &id('a'), AliasKind::OtherFile)
            .unwrap();
        db.add_alias(&id('b'), &id('a'), AliasKind::OtherFile)
            .unwrap(); // a real book: ignored
        assert_eq!(db.resolve_book_id(&id('c')).unwrap(), Some(id('a')));
        assert_eq!(db.resolve_book_id(&id('b')).unwrap(), Some(id('b')));
        assert_eq!(
            db.aliases_of(&id('a')).unwrap(),
            vec![Alias {
                id: id('c'),
                kind: AliasKind::OtherFile
            }]
        );
        assert_eq!(
            db.books_with_other_file_notes().unwrap(),
            vec![(id('a'), 0)]
        );

        assert_eq!(
            db.find_book_by_identifiers(Some("9780131103627"), None, None, None)
                .unwrap(),
            Some(id('a'))
        );
        assert_eq!(
            db.find_book_by_identifiers(None, None, Some("10.1/x"), None)
                .unwrap(),
            None
        );
        assert_eq!(db.books_titled("  optics ").unwrap()[0].0, id('a'));
        let dups = db.books_sharing_identifiers().unwrap();
        assert_eq!(dups.len(), 1);
        assert_eq!(dups[0].1.len(), 2);
        assert!(db.check().unwrap().is_empty());
    }

    #[test]
    fn snapshots_keep_only_what_is_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("library.db")).unwrap();
        let owner = db.ensure_owner_profile("Owner", "now").unwrap();
        let other = ProfileId::new();
        db.insert_profile(&other, "Sam", "now").unwrap();
        db.insert_book(&book('a', "Optics", None), 0).unwrap();
        db.set_user_state(
            &id('a'),
            &other,
            &BookUserState {
                rating: 4,
                ..Default::default()
            },
        )
        .unwrap();
        let mut p = db.profile(&owner).unwrap().unwrap();
        p.pin_hash = Some("$argon2id$secret".into());
        db.save_profile(&p).unwrap();

        let dest = dir.path().join("copy.db");
        db.snapshot(
            &dest,
            &SnapshotScope {
                only_profile: Some(owner),
                strip_pins: true,
                ..Default::default()
            },
        )
        .unwrap();
        let copy = Database::open(&dest).unwrap();
        let profiles = copy.profiles().unwrap();
        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].pin_hash, None);
        assert_eq!(copy.book(&id('a'), &other).unwrap().unwrap().user.rating, 0);
        assert_eq!(copy.book_count().unwrap(), 1);
        drop(copy);

        let bare = dir.path().join("bare.db");
        db.snapshot(
            &bare,
            &SnapshotScope {
                no_personal_data: true,
                ..Default::default()
            },
        )
        .unwrap();
        let conn = Connection::open(&bare).unwrap();
        for table in [
            "profiles",
            "book_user",
            "annotations",
            "notebooks",
            "collections",
        ] {
            let n: i64 = conn
                .query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(n, 0, "{table} should be empty");
        }
        let books: i64 = conn
            .query_row("SELECT count(*) FROM books", [], |r| r.get(0))
            .unwrap();
        assert_eq!(books, 1);
    }
}
