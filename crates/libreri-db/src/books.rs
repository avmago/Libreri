//! Book records, tags, categories, per-profile state and metadata search.

use crate::{Database, Result};
use libreri_core::query::{fts_query, sort_author, sort_title};
use libreri_core::{
    Book, BookId, BookMetadata, BookQuery, BookUserState, ContentType, FileType, ProfileId,
    ReadingStatus, SortKey,
};
use rusqlite::types::Value;
use rusqlite::{params, params_from_iter, OptionalExtension, Row, Transaction};

/// What the scanner needs to know about each file without loading metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub id: BookId,
    pub rel_path: String,
    pub file_size: u64,
    pub file_mtime: i64,
    pub missing: bool,
}

/// Counts shown in the sidebar and filter chips.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Facets {
    pub total: u32,
    pub want_to_read: u32,
    pub reading: u32,
    pub finished: u32,
    pub favorites: u32,
    pub audio: u32,
    pub missing: u32,
    pub file_types: Vec<(FileType, u32)>,
    pub content_types: Vec<(ContentType, u32)>,
    pub tags: Vec<(String, u32)>,
    pub categories: Vec<(String, u32)>,
}

const SELECT: &str = "
    SELECT b.id, b.rel_path, b.file_type, b.file_size, b.has_cover, b.missing,
           b.added_at, b.modified_at,
           b.title, b.subtitle, b.authors, b.contributors, b.about, b.year, b.publisher,
           b.pages, b.isbn13, b.isbn10, b.edition, b.language, b.content_type, b.series,
           b.series_number, b.doi, b.arxiv_id, b.journal, b.volume, b.issue, b.url,
           (SELECT json_group_array(name) FROM (SELECT t.name FROM book_tags bt
               JOIN tags t ON t.id = bt.tag_id WHERE bt.book_id = b.id ORDER BY t.name
               COLLATE NOCASE)),
           (SELECT json_group_array(path) FROM (SELECT c.path FROM book_categories bc
               JOIN categories c ON c.id = bc.category_id WHERE bc.book_id = b.id
               ORDER BY c.path COLLATE NOCASE)),
           COALESCE(u.status, 'none'), COALESCE(u.rating, 0), COALESCE(u.favorite, 0),
           COALESCE(u.progress, 0), u.last_opened
    FROM books b
    LEFT JOIN book_user u ON u.book_id = b.id AND u.profile_id = ?1";

fn json_list(text: String) -> Vec<String> {
    serde_json::from_str(&text).unwrap_or_default()
}

fn row_to_book(r: &Row<'_>) -> rusqlite::Result<Book> {
    let id: String = r.get(0)?;
    let file_type: String = r.get(2)?;
    let content_type: String = r.get(20)?;
    let status: String = r.get(31)?;
    Ok(Book {
        id: BookId::from_hex(id).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, e.into())
        })?,
        rel_path: r.get(1)?,
        file_type: FileType::parse(&file_type).unwrap_or(FileType::Pdf),
        file_size: r.get::<_, i64>(3)?.max(0) as u64,
        has_cover: r.get(4)?,
        missing: r.get(5)?,
        added_at: r.get(6)?,
        modified_at: r.get(7)?,
        metadata: BookMetadata {
            title: r.get(8)?,
            subtitle: r.get(9)?,
            authors: json_list(r.get(10)?),
            contributors: json_list(r.get(11)?),
            about: r.get(12)?,
            year: r.get(13)?,
            publisher: r.get(14)?,
            pages: r.get(15)?,
            isbn13: r.get(16)?,
            isbn10: r.get(17)?,
            edition: r.get(18)?,
            language: r.get(19)?,
            content_type: ContentType::parse(&content_type).unwrap_or_default(),
            series: r.get(21)?,
            series_number: r.get(22)?,
            doi: r.get(23)?,
            arxiv_id: r.get(24)?,
            journal: r.get(25)?,
            volume: r.get(26)?,
            issue: r.get(27)?,
            url: r.get(28)?,
            tags: json_list(r.get(29)?),
            categories: json_list(r.get(30)?),
        },
        user: BookUserState {
            status: ReadingStatus::parse(&status).unwrap_or_default(),
            rating: r.get::<_, i64>(32)?.clamp(0, 5) as u8,
            favorite: r.get(33)?,
            progress: r.get::<_, f64>(34)? as f32,
            last_opened: r.get(35)?,
        },
    })
}

/// Escapes `%`, `_` and `\` for a LIKE pattern using `ESCAPE '\'`.
fn like_escape(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// SQL condition on `b` limiting books to `within` folders (relative to
/// `Books/`). Empty means no limit. Values are quoted inline because this
/// is reused in several statements with their own parameters.
fn scope_sql(within: &[String]) -> String {
    if within.is_empty() {
        return "1=1".to_owned();
    }
    let parts: Vec<String> = within
        .iter()
        .map(|f| {
            let f = f.trim_matches('/');
            let prefix = if f.is_empty() {
                "Books/".to_owned()
            } else {
                format!("Books/{f}/")
            };
            // A NUL cannot be in a path (and would cut the SQL short).
            if prefix.contains('\0') {
                return "0=1".to_owned();
            }
            // Compared exactly: LIKE ignores ASCII case, so "Kids" would
            // also let in "kids/".
            format!(
                "substr(b.rel_path, 1, {}) = '{}'",
                prefix.chars().count(),
                prefix.replace('\'', "''")
            )
        })
        .collect();
    format!("({})", parts.join(" OR "))
}

fn write_metadata(tx: &Transaction<'_>, id: &str, m: &BookMetadata) -> Result<()> {
    tx.execute(
        "UPDATE books SET title=?2, sort_title=?3, subtitle=?4, authors=?5, sort_author=?6,
            contributors=?7, about=?8, year=?9, publisher=?10, pages=?11, isbn13=?12,
            isbn10=?13, edition=?14, language=?15, content_type=?16, series=?17,
            series_number=?18, doi=?19, arxiv_id=?20, journal=?21, volume=?22, issue=?23,
            url=?24
         WHERE id=?1",
        params![
            id,
            m.title,
            sort_title(&m.title),
            m.subtitle,
            serde_json::to_string(&m.authors).unwrap_or_else(|_| "[]".into()),
            sort_author(&m.authors),
            serde_json::to_string(&m.contributors).unwrap_or_else(|_| "[]".into()),
            m.about,
            m.year,
            m.publisher,
            m.pages,
            m.isbn13,
            m.isbn10,
            m.edition,
            m.language,
            m.content_type.as_str(),
            m.series,
            m.series_number,
            m.doi,
            m.arxiv_id,
            m.journal,
            m.volume,
            m.issue,
            m.url,
        ],
    )?;

    tx.execute("DELETE FROM book_tags WHERE book_id=?1", [id])?;
    for tag in &m.tags {
        tx.execute("INSERT OR IGNORE INTO tags(name) VALUES (?1)", [tag])?;
        tx.execute(
            "INSERT OR IGNORE INTO book_tags(book_id, tag_id)
             SELECT ?1, id FROM tags WHERE name = ?2",
            [id, tag],
        )?;
    }
    tx.execute("DELETE FROM book_categories WHERE book_id=?1", [id])?;
    for cat in &m.categories {
        tx.execute("INSERT OR IGNORE INTO categories(path) VALUES (?1)", [cat])?;
        tx.execute(
            "INSERT OR IGNORE INTO book_categories(book_id, category_id)
             SELECT ?1, id FROM categories WHERE path = ?2",
            [id, cat],
        )?;
    }
    // Tags and categories nobody uses any more disappear from the filters.
    tx.execute(
        "DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM book_tags)",
        [],
    )?;
    tx.execute(
        "DELETE FROM categories WHERE id NOT IN (SELECT category_id FROM book_categories)",
        [],
    )?;

    let identifiers = [&m.isbn13, &m.isbn10, &m.doi, &m.arxiv_id]
        .into_iter()
        .flatten()
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    let title = match &m.subtitle {
        Some(sub) => format!("{} {}", m.title, sub),
        None => m.title.clone(),
    };
    tx.execute("DELETE FROM books_fts WHERE book_id=?1", [id])?;
    tx.execute(
        "INSERT INTO books_fts(book_id, title, authors, about, tags, categories, publisher,
            series, identifiers) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            id,
            title,
            [m.authors.clone(), m.contributors.clone()]
                .concat()
                .join(" "),
            m.about.as_deref().unwrap_or(""),
            m.tags.join(" "),
            m.categories.join(" ").replace('/', " "),
            m.publisher.as_deref().unwrap_or(""),
            m.series.as_deref().unwrap_or(""),
            identifiers,
        ],
    )?;
    Ok(())
}

impl Database {
    /// Adds a new book. `book.user` is ignored (see [`Database::set_user_state`]).
    pub fn insert_book(&self, book: &Book, file_mtime: i64) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO books(id, rel_path, file_type, file_size, file_mtime, has_cover,
                missing, added_at, modified_at, title, sort_title)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, '', '')",
            params![
                book.id.as_str(),
                book.rel_path,
                book.file_type.as_str(),
                book.file_size as i64,
                file_mtime,
                book.has_cover,
                book.missing,
                book.added_at,
                book.modified_at,
            ],
        )?;
        write_metadata(&tx, book.id.as_str(), &book.metadata)?;
        tx.commit()?;
        Ok(())
    }

    /// Replaces a book's metadata. Returns `false` if the book does not exist.
    pub fn update_metadata(&self, id: &BookId, m: &BookMetadata, now: &str) -> Result<bool> {
        let tx = self.conn.unchecked_transaction()?;
        let n = tx.execute(
            "UPDATE books SET modified_at=?2 WHERE id=?1",
            params![id.as_str(), now],
        )?;
        if n == 0 {
            return Ok(false);
        }
        write_metadata(&tx, id.as_str(), m)?;
        tx.commit()?;
        Ok(true)
    }

    /// Finds the current id for `id`, following aliases left by file changes.
    pub fn resolve_book_id(&self, id: &BookId) -> Result<Option<BookId>> {
        let found: Option<String> = self
            .conn
            .query_row(
                "SELECT id FROM books WHERE id=?1
                 UNION ALL SELECT book_id FROM book_aliases WHERE old_id=?1 LIMIT 1",
                [id.as_str()],
                |r| r.get(0),
            )
            .optional()?;
        Ok(found.and_then(|s| BookId::from_hex(s).ok()))
    }

    pub fn book(&self, id: &BookId, profile: &ProfileId) -> Result<Option<Book>> {
        let sql = format!("{SELECT} WHERE b.id = ?2");
        Ok(self
            .conn
            .prepare_cached(&sql)?
            .query_row(params![profile.to_string(), id.as_str()], row_to_book)
            .optional()?)
    }

    pub fn book_by_path(&self, rel_path: &str, profile: &ProfileId) -> Result<Option<Book>> {
        let sql = format!("{SELECT} WHERE b.rel_path = ?2");
        Ok(self
            .conn
            .prepare_cached(&sql)?
            .query_row(params![profile.to_string(), rel_path], row_to_book)
            .optional()?)
    }

    /// Books matching `q`, in the requested order.
    pub fn query_books(&self, q: &BookQuery, profile: &ProfileId) -> Result<Vec<Book>> {
        let mut sql = format!("{SELECT} WHERE {}", scope_sql(&q.within_folders));
        let mut args: Vec<Value> = vec![Value::Text(profile.to_string())];
        let arg = |v: Value, args: &mut Vec<Value>| {
            args.push(v);
            format!("?{}", args.len())
        };

        if let Some(folder) = &q.folder {
            let prefix = if folder.is_empty() {
                "Books/".to_owned()
            } else {
                format!("Books/{}/", folder.trim_matches('/'))
            };
            let len = prefix.chars().count() as i64;
            let p = arg(Value::Text(prefix), &mut args);
            let l = arg(Value::Integer(len), &mut args);
            sql += &format!(" AND substr(b.rel_path, 1, {l}) = {p}");
            if !q.include_subfolders {
                let n = arg(Value::Integer(len + 1), &mut args);
                sql += &format!(" AND instr(substr(b.rel_path, {n}), '/') = 0");
            }
        }
        if let Some(fts) = q.search.as_deref().and_then(fts_query) {
            let p = arg(Value::Text(fts), &mut args);
            sql +=
                &format!(" AND b.id IN (SELECT book_id FROM books_fts WHERE books_fts MATCH {p})");
        }
        if !q.file_types.is_empty() {
            let list: Vec<String> = q
                .file_types
                .iter()
                .map(|t| arg(Value::Text(t.as_str().into()), &mut args))
                .collect();
            sql += &format!(" AND b.file_type IN ({})", list.join(","));
        }
        if !q.content_types.is_empty() {
            let list: Vec<String> = q
                .content_types
                .iter()
                .map(|t| arg(Value::Text(t.as_str().into()), &mut args))
                .collect();
            sql += &format!(" AND b.content_type IN ({})", list.join(","));
        }
        for tag in &q.tags {
            let p = arg(Value::Text(tag.clone()), &mut args);
            sql += &format!(
                " AND EXISTS (SELECT 1 FROM book_tags bt JOIN tags t ON t.id = bt.tag_id
                   WHERE bt.book_id = b.id AND t.name = {p})"
            );
        }
        if let Some(cat) = &q.category {
            let p = arg(Value::Text(cat.clone()), &mut args);
            let l = arg(Value::Text(format!("{}/%", like_escape(cat))), &mut args);
            sql += &format!(
                " AND EXISTS (SELECT 1 FROM book_categories bc JOIN categories c
                   ON c.id = bc.category_id WHERE bc.book_id = b.id
                   AND (c.path = {p} OR c.path LIKE {l} ESCAPE '\\'))"
            );
        }
        if let Some(status) = q.status {
            let p = arg(Value::Text(status.as_str().into()), &mut args);
            sql += &format!(" AND COALESCE(u.status, 'none') = {p}");
        }
        if q.favorites_only {
            sql += " AND COALESCE(u.favorite, 0) = 1";
        }
        if let Some(audio) = q.audio {
            let list = FileType::ALL
                .iter()
                .filter(|t| t.is_audio())
                .map(|t| format!("'{}'", t.as_str()))
                .collect::<Vec<_>>()
                .join(",");
            sql += &format!(
                " AND b.file_type {} ({list})",
                if audio { "IN" } else { "NOT IN" }
            );
        }
        if q.missing_only {
            sql += " AND b.missing = 1";
        }

        let dir = if q.descending { "DESC" } else { "ASC" };
        let order = match q.sort {
            SortKey::Title => format!("b.sort_title COLLATE NOCASE {dir}"),
            SortKey::Author => {
                format!("b.sort_author COLLATE NOCASE {dir}, b.sort_title COLLATE NOCASE")
            }
            SortKey::Added => format!("b.added_at {dir}"),
            SortKey::Year => format!("b.year IS NULL, b.year {dir}"),
            SortKey::Size => format!("b.file_size {dir}"),
            SortKey::Pages => format!("b.pages IS NULL, b.pages {dir}"),
            SortKey::LastOpened => format!("u.last_opened IS NULL, u.last_opened {dir}"),
            SortKey::Rating => format!("COALESCE(u.rating, 0) {dir}"),
        };
        sql += &format!(" ORDER BY {order}, b.sort_title COLLATE NOCASE, b.id");

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(params_from_iter(args), row_to_book)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    /// Path, size and modification time of every book, for the scanner.
    pub fn file_records(&self) -> Result<Vec<FileRecord>> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, rel_path, file_size, file_mtime, missing FROM books")?;
        let rows = stmt.query_map([], |r| {
            let id: String = r.get(0)?;
            Ok((id, r.get(1)?, r.get::<_, i64>(2)?, r.get(3)?, r.get(4)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            let (id, rel_path, size, mtime, missing) = row?;
            if let Ok(id) = BookId::from_hex(id) {
                out.push(FileRecord {
                    id,
                    rel_path,
                    file_size: size.max(0) as u64,
                    file_mtime: mtime,
                    missing,
                });
            }
        }
        Ok(out)
    }

    /// Records that a book's file now lives at `rel_path` (and is present).
    pub fn set_book_path(&self, id: &BookId, rel_path: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET rel_path=?2, missing=0 WHERE id=?1",
            params![id.as_str(), rel_path],
        )?;
        Ok(())
    }

    /// Rewrites the paths of every book under folder `old` (library-relative,
    /// such as "Books/Physics") to `new`. Returns how many books moved.
    pub fn move_folder_paths(&self, old: &str, new: &str) -> Result<usize> {
        let old = format!("{}/", old.trim_end_matches('/'));
        let new = format!("{}/", new.trim_end_matches('/'));
        Ok(self.conn.execute(
            // Exact, case-sensitive prefix: renaming "physics" must not
            // touch the books in a sibling "Physics".
            "UPDATE books SET rel_path = ?2 || substr(rel_path, ?3)
             WHERE substr(rel_path, 1, ?3 - 1) = ?1",
            params![old, new, old.chars().count() as i64 + 1],
        )?)
    }

    /// The book's file is now of another kind (another edition was located).
    pub fn set_file_type(&self, id: &BookId, file_type: FileType) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET file_type=?2 WHERE id=?1",
            params![id.as_str(), file_type.as_str()],
        )?;
        Ok(())
    }

    pub fn set_missing(&self, id: &BookId, missing: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET missing=?2 WHERE id=?1",
            params![id.as_str(), missing],
        )?;
        Ok(())
    }

    /// Records whether a book has a cover. Also bumps `modified_at` so the
    /// interface reloads the image.
    pub fn set_has_cover(&self, id: &BookId, has_cover: bool, now: &str) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET has_cover=?2, modified_at=?3 WHERE id=?1",
            params![id.as_str(), has_cover, now],
        )?;
        Ok(())
    }

    /// Stores the size and time of an unchanged file so it is not re-hashed.
    pub fn set_file_stamp(&self, id: &BookId, size: u64, mtime: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE books SET file_size=?2, file_mtime=?3 WHERE id=?1",
            params![id.as_str(), size as i64, mtime],
        )?;
        Ok(())
    }

    /// The file of book `old` was changed outside Libreri and now hashes to
    /// `new`. Keeps every detail and note, and remembers the old id.
    pub fn change_book_id(
        &self,
        old: &BookId,
        new: &BookId,
        size: u64,
        mtime: i64,
        now: &str,
    ) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE books SET id=?2, file_size=?3, file_mtime=?4, modified_at=?5, missing=0
             WHERE id=?1",
            params![old.as_str(), new.as_str(), size as i64, mtime, now],
        )?;
        tx.execute(
            "UPDATE books_fts SET book_id=?2 WHERE book_id=?1",
            [old.as_str(), new.as_str()],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO book_aliases(old_id, book_id) VALUES (?1, ?2)",
            [old.as_str(), new.as_str()],
        )?;
        tx.execute("DELETE FROM book_aliases WHERE old_id = ?1", [new.as_str()])?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_book(&self, id: &BookId) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM books_fts WHERE book_id=?1", [id.as_str()])?;
        tx.execute("DELETE FROM books WHERE id=?1", [id.as_str()])?;
        tx.execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT tag_id FROM book_tags)",
            [],
        )?;
        tx.execute(
            "DELETE FROM categories WHERE id NOT IN (SELECT category_id FROM book_categories)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn set_user_state(
        &self,
        id: &BookId,
        profile: &ProfileId,
        s: &BookUserState,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO book_user(book_id, profile_id, status, rating, favorite, progress,
                last_opened) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(book_id, profile_id) DO UPDATE SET status=excluded.status,
                rating=excluded.rating, favorite=excluded.favorite,
                progress=excluded.progress, last_opened=excluded.last_opened",
            params![
                id.as_str(),
                profile.to_string(),
                s.status.as_str(),
                s.rating.min(5),
                s.favorite,
                f64::from(s.progress.clamp(0.0, 1.0)),
                s.last_opened,
            ],
        )?;
        Ok(())
    }

    /// Counts for the sidebar and filters.
    pub fn facets(&self, profile: &ProfileId, within: &[String]) -> Result<Facets> {
        let p = profile.to_string();
        let scope = scope_sql(within);
        let audio = FileType::ALL
            .iter()
            .filter(|t| t.is_audio())
            .map(|t| format!("'{}'", t.as_str()))
            .collect::<Vec<_>>()
            .join(",");
        let mut f = self.conn.query_row(
            &format!(
                "SELECT COUNT(*),
                    SUM(COALESCE(u.status,'none') = 'wantToRead'),
                    SUM(COALESCE(u.status,'none') = 'reading'),
                    SUM(COALESCE(u.status,'none') = 'finished'),
                    SUM(COALESCE(u.favorite,0) = 1),
                    SUM(b.file_type IN ({audio})),
                    SUM(b.missing = 1)
                 FROM books b LEFT JOIN book_user u ON u.book_id=b.id AND u.profile_id=?1
                 WHERE {scope}"
            ),
            [&p],
            |r| {
                let n = |i: usize| -> rusqlite::Result<u32> {
                    Ok(r.get::<_, Option<i64>>(i)?.unwrap_or(0) as u32)
                };
                Ok(Facets {
                    total: n(0)?,
                    want_to_read: n(1)?,
                    reading: n(2)?,
                    finished: n(3)?,
                    favorites: n(4)?,
                    audio: n(5)?,
                    missing: n(6)?,
                    ..Default::default()
                })
            },
        )?;

        let pairs = |sql: &str| -> Result<Vec<(String, u32)>> {
            let mut stmt = self.conn.prepare(sql)?;
            let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get::<_, i64>(1)? as u32)))?;
            Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
        };
        f.file_types = pairs(&format!(
            "SELECT file_type, COUNT(*) FROM books b WHERE {scope}
             GROUP BY file_type ORDER BY COUNT(*) DESC"
        ))?
        .into_iter()
        .filter_map(|(t, n)| FileType::parse(&t).map(|t| (t, n)))
        .collect();
        f.content_types = pairs(&format!(
            "SELECT content_type, COUNT(*) FROM books b WHERE {scope} GROUP BY content_type
             ORDER BY COUNT(*) DESC"
        ))?
        .into_iter()
        .filter_map(|(t, n)| ContentType::parse(&t).map(|t| (t, n)))
        .collect();
        f.tags = pairs(&format!(
            "SELECT t.name, COUNT(*) FROM tags t JOIN book_tags bt ON bt.tag_id = t.id
             JOIN books b ON b.id = bt.book_id WHERE {scope}
             GROUP BY t.id ORDER BY COUNT(*) DESC, t.name COLLATE NOCASE"
        ))?;
        f.categories = pairs(&format!(
            "SELECT c.path, COUNT(*) FROM categories c
             JOIN book_categories bc ON bc.category_id = c.id
             JOIN books b ON b.id = bc.book_id WHERE {scope}
             GROUP BY c.id ORDER BY c.path COLLATE NOCASE"
        ))?;
        Ok(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-09-27T10:00:00Z";

    fn hex(n: u8) -> BookId {
        BookId::from_hex(format!("{:064x}", n)).unwrap()
    }

    fn book(n: u8, path: &str, title: &str) -> Book {
        Book {
            id: hex(n),
            rel_path: path.into(),
            file_type: FileType::from_path(std::path::Path::new(path)).unwrap(),
            file_size: 1000 + u64::from(n),
            has_cover: false,
            missing: false,
            added_at: format!("2026-09-{:02}T00:00:00Z", n),
            modified_at: NOW.into(),
            metadata: BookMetadata {
                title: title.into(),
                ..Default::default()
            },
            user: Default::default(),
        }
    }

    fn setup() -> (Database, ProfileId) {
        let db = Database::open_in_memory().unwrap();
        let p = db.ensure_owner_profile("Owner", NOW).unwrap();
        (db, p)
    }

    #[test]
    fn insert_read_and_update_round_trip() {
        let (db, p) = setup();
        let mut b = book(1, "Books/Physics/qm.pdf", "Quantum Mechanics");
        b.metadata.authors = vec!["John Smith".into()];
        b.metadata.tags = vec!["physics".into(), "Quantum".into()];
        b.metadata.categories = vec!["Science/Physics".into()];
        db.insert_book(&b, 5).unwrap();

        let got = db.book(&b.id, &p).unwrap().unwrap();
        assert_eq!(got.metadata.authors, vec!["John Smith"]);
        assert_eq!(got.metadata.tags, vec!["physics", "Quantum"]);
        assert_eq!(got.folder(), "Physics");

        let mut m = got.metadata.clone();
        m.title = "Quantum Mechanics, 2nd ed.".into();
        m.tags = vec!["physics".into()];
        assert!(db.update_metadata(&b.id, &m, NOW).unwrap());
        let got = db.book(&b.id, &p).unwrap().unwrap();
        assert_eq!(got.metadata.tags, vec!["physics"]);
        assert_eq!(
            db.facets(&p, &[]).unwrap().tags,
            vec![("physics".into(), 1)]
        );
        assert!(!db.update_metadata(&hex(9), &m, NOW).unwrap());
    }

    #[test]
    fn folders_search_and_filters() {
        let (db, p) = setup();
        let mut a = book(1, "Books/a.pdf", "The Art of War");
        a.metadata.authors = vec!["Sun Tzu".into()];
        a.metadata.isbn13 = Some("9780306406157".into());
        let mut b = book(2, "Books/Physics/b.epub", "Brief History of Time");
        b.metadata.tags = vec!["cosmology".into()];
        b.metadata.categories = vec!["Science/Physics".into()];
        let c = book(3, "Books/Physics/Quantum/c.md", "Café notes");
        for x in [&a, &b, &c] {
            db.insert_book(x, 0).unwrap();
        }
        let titles = |q: BookQuery| -> Vec<String> {
            db.query_books(&q, &p)
                .unwrap()
                .into_iter()
                .map(|b| b.metadata.title)
                .collect()
        };

        assert_eq!(
            titles(BookQuery::default()),
            vec!["The Art of War", "Brief History of Time", "Café notes"]
        );
        assert_eq!(
            titles(BookQuery {
                folder: Some("".into()),
                ..Default::default()
            }),
            vec!["The Art of War"]
        );
        assert_eq!(
            titles(BookQuery {
                folder: Some("Physics".into()),
                include_subfolders: true,
                ..Default::default()
            })
            .len(),
            2
        );
        let search = |s: &str| {
            titles(BookQuery {
                search: Some(s.into()),
                ..Default::default()
            })
        };
        assert_eq!(search("hist"), vec!["Brief History of Time"]);
        assert_eq!(search("cafe"), vec!["Café notes"]);
        assert_eq!(search("tzu war"), vec!["The Art of War"]);
        assert_eq!(search("978-0-306"), vec!["The Art of War"]);
        assert_eq!(search("physics"), vec!["Brief History of Time"]);
        assert_eq!(
            titles(BookQuery {
                category: Some("Science".into()),
                ..Default::default()
            }),
            vec!["Brief History of Time"]
        );
        assert_eq!(
            titles(BookQuery {
                file_types: vec![FileType::Md, FileType::Epub],
                sort: SortKey::Added,
                descending: true,
                ..Default::default()
            }),
            vec!["Café notes", "Brief History of Time"]
        );
    }

    #[test]
    fn user_state_is_per_profile() {
        let (db, p) = setup();
        let b = book(1, "Books/a.pdf", "A");
        db.insert_book(&b, 0).unwrap();
        let state = BookUserState {
            status: ReadingStatus::Reading,
            rating: 4,
            favorite: true,
            ..Default::default()
        };
        db.set_user_state(&b.id, &p, &state).unwrap();
        assert_eq!(db.book(&b.id, &p).unwrap().unwrap().user.rating, 4);
        let other = ProfileId::new();
        assert_eq!(db.book(&b.id, &other).unwrap().unwrap().user.rating, 0);
        let f = db.facets(&p, &[]).unwrap();
        assert_eq!((f.total, f.reading, f.favorites), (1, 1, 1));
        let reading = db
            .query_books(
                &BookQuery {
                    status: Some(ReadingStatus::Reading),
                    ..Default::default()
                },
                &p,
            )
            .unwrap();
        assert_eq!(reading.len(), 1);
    }

    #[test]
    fn folder_prefixes_are_case_sensitive_and_literal() {
        let (db, p) = setup();
        db.insert_book(&book(1, "Books/Kids/a.pdf", "A"), 0)
            .unwrap();
        db.insert_book(&book(2, "Books/kids/b.pdf", "B"), 0)
            .unwrap();
        db.insert_book(&book(3, "Books/K_ds/c.pdf", "C"), 0)
            .unwrap();
        db.insert_book(&book(4, "Books/100%/d.pdf", "D"), 0)
            .unwrap();
        db.insert_book(&book(5, "Books/100x/e.pdf", "E"), 0)
            .unwrap();
        let titles = |q: BookQuery| -> Vec<String> {
            let mut t: Vec<String> = db
                .query_books(&q, &p)
                .unwrap()
                .into_iter()
                .map(|b| b.metadata.title)
                .collect();
            t.sort();
            t
        };
        let within = |f: &str| BookQuery {
            within_folders: vec![f.into()],
            ..Default::default()
        };
        assert_eq!(titles(within("Kids")), vec!["A"]);
        assert_eq!(titles(within("100%")), vec!["D"]);
        assert_eq!(titles(within("it's")), Vec::<String>::new());
        assert_eq!(
            titles(BookQuery {
                folder: Some("kids".into()),
                ..Default::default()
            }),
            vec!["B"]
        );
        assert_eq!(
            titles(BookQuery {
                folder: Some("K_ds".into()),
                ..Default::default()
            }),
            vec!["C"]
        );
        assert_eq!(
            db.move_folder_paths("Books/kids", "Books/young").unwrap(),
            1
        );
        assert_eq!(
            db.book(&hex(1), &p).unwrap().unwrap().rel_path,
            "Books/Kids/a.pdf"
        );
        assert_eq!(
            db.book(&hex(2), &p).unwrap().unwrap().rel_path,
            "Books/young/b.pdf"
        );
    }

    #[test]
    fn moving_folders_and_changing_ids_keep_everything() {
        let (db, p) = setup();
        let mut b = book(1, "Books/Old/x.pdf", "X");
        b.metadata.tags = vec!["t".into()];
        db.insert_book(&b, 0).unwrap();
        db.insert_book(&book(2, "Books/Older/y.pdf", "Y"), 0)
            .unwrap();
        assert_eq!(db.move_folder_paths("Books/Old", "Books/New").unwrap(), 1);
        assert_eq!(
            db.book(&b.id, &p).unwrap().unwrap().rel_path,
            "Books/New/x.pdf"
        );

        db.set_user_state(
            &b.id,
            &p,
            &BookUserState {
                rating: 5,
                ..Default::default()
            },
        )
        .unwrap();
        db.change_book_id(&b.id, &hex(7), 5, 6, NOW).unwrap();
        let moved = db.book(&hex(7), &p).unwrap().unwrap();
        assert_eq!(moved.user.rating, 5);
        assert_eq!(moved.metadata.tags, vec!["t"]);
        assert_eq!(db.resolve_book_id(&b.id).unwrap(), Some(hex(7)));
        assert_eq!(
            db.query_books(
                &BookQuery {
                    search: Some("x".into()),
                    ..Default::default()
                },
                &p
            )
            .unwrap()
            .len(),
            1
        );

        db.delete_book(&hex(7)).unwrap();
        assert_eq!(db.book_count().unwrap(), 1);
        assert!(db.facets(&p, &[]).unwrap().tags.is_empty());
    }
}
