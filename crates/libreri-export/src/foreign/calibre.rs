//! Calibre libraries: `metadata.db` plus one folder per book holding its
//! formats and `cover.jpg`. The database is opened read-only.

use super::{
    foreign_file, ForeignBook, ForeignError, ForeignLibrary, ForeignPersonal, ForeignSource,
};
use libreri_core::{BookMetadata, ContentType, FileType};
use libreri_metadata::normalise;
use rusqlite::{Connection, OpenFlags};
use std::collections::HashMap;
use std::path::Path;

/// Formats in the order a book's file is chosen when Calibre has several.
pub const DEFAULT_FORMAT_ORDER: &[FileType] = &[
    FileType::Epub,
    FileType::Pdf,
    FileType::Azw3,
    FileType::Mobi,
    FileType::Fb2,
    FileType::Djvu,
    FileType::Cbz,
    FileType::Cbr,
    FileType::Cb7,
    FileType::Cbt,
    FileType::Md,
    FileType::Txt,
    FileType::M4b,
    FileType::Mp3,
];

fn open(dir: &Path) -> Result<Connection, ForeignError> {
    let db = dir.join("metadata.db");
    if !db.is_file() {
        return Err(ForeignError::NotRecognised(
            "this folder is not a Calibre library (there is no metadata.db)".into(),
        ));
    }
    Ok(Connection::open_with_flags(
        &db,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?)
}

/// id → list of values from a `books_<x>_link` table.
fn linked(conn: &Connection, sql: &str) -> Result<HashMap<i64, Vec<String>>, ForeignError> {
    let mut out: HashMap<i64, Vec<String>> = HashMap::new();
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
    for row in rows {
        let (id, v) = row?;
        out.entry(id).or_default().push(v);
    }
    Ok(out)
}

fn content_type(tags: &[String], file_types: &[FileType]) -> ContentType {
    let has = |w: &str| tags.iter().any(|t| t.eq_ignore_ascii_case(w));
    let all = |f: fn(FileType) -> bool| !file_types.is_empty() && file_types.iter().all(|t| f(*t));
    if has("comic") || has("comics") || has("manga") || all(FileType::is_comic) {
        ContentType::Comic
    } else if has("textbook") {
        ContentType::Textbook
    } else if has("paper") || has("research paper") {
        ContentType::ResearchPaper
    } else if all(FileType::is_audio) {
        ContentType::Audiobook
    } else {
        ContentType::Book
    }
}

pub fn read(dir: &Path) -> Result<ForeignLibrary, ForeignError> {
    let conn = open(dir)?;
    let authors = linked(
        &conn,
        "SELECT l.book, a.name FROM books_authors_link l JOIN authors a ON a.id = l.author ORDER BY l.id",
    )?;
    let tags = linked(
        &conn,
        "SELECT l.book, t.name FROM books_tags_link l JOIN tags t ON t.id = l.tag ORDER BY t.name",
    )?;
    let series = linked(
        &conn,
        "SELECT l.book, s.name FROM books_series_link l JOIN series s ON s.id = l.series",
    )?;
    let publishers = linked(
        &conn,
        "SELECT l.book, p.name FROM books_publishers_link l JOIN publishers p ON p.id = l.publisher",
    )?;
    let languages = linked(
        &conn,
        "SELECT l.book, g.lang_code FROM books_languages_link l JOIN languages g ON g.id = l.lang_code ORDER BY l.item_order",
    )?;
    let ratings = linked(
        &conn,
        "SELECT l.book, CAST(r.rating AS TEXT) FROM books_ratings_link l JOIN ratings r ON r.id = l.rating",
    )?;
    let comments = linked(&conn, "SELECT book, text FROM comments")?;
    let mut identifiers: HashMap<i64, Vec<(String, String)>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT book, type, val FROM identifiers")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (book, t, v) = row?;
            identifiers
                .entry(book)
                .or_default()
                .push((t.to_lowercase(), v));
        }
    }
    let mut formats: HashMap<i64, Vec<(String, String)>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT book, format, name FROM data")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (book, f, name) = row?;
            formats.entry(book).or_default().push((f, name));
        }
    }

    let mut books = Vec::new();
    let mut warnings = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT id, title, pubdate, series_index, path, has_cover, timestamp, isbn FROM books ORDER BY sort",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, String>(1)?,
            r.get::<_, Option<String>>(2)?,
            r.get::<_, Option<f64>>(3)?,
            r.get::<_, String>(4)?,
            r.get::<_, Option<bool>>(5)?,
            r.get::<_, Option<String>>(6)?,
            r.get::<_, Option<String>>(7)?,
        ))
    })?;
    for row in rows {
        let (id, title, pubdate, series_index, path, has_cover, timestamp, isbn) = row?;
        let folder = dir.join(&path);
        let get = |m: &HashMap<i64, Vec<String>>| m.get(&id).cloned().unwrap_or_default();
        let ids = identifiers.get(&id).cloned().unwrap_or_default();
        let ident = |t: &str| ids.iter().find(|(k, _)| k == t).map(|(_, v)| v.clone());
        let isbn = ident("isbn").or(isbn).filter(|s| !s.trim().is_empty());
        let (isbn13, isbn10) = match isbn.as_deref().map(|s| s.replace(['-', ' '], "")) {
            Some(s) if s.len() == 10 => (None, Some(s)),
            Some(s) => (Some(s), None),
            None => (None, None),
        };
        let files: Vec<_> = formats
            .get(&id)
            .into_iter()
            .flatten()
            .filter_map(|(f, name)| {
                foreign_file(folder.join(format!("{name}.{}", f.to_lowercase())))
            })
            .collect();
        let file_types: Vec<FileType> = files.iter().map(|f| f.file_type).collect();
        let tag_list = get(&tags);
        // Calibre's "undefined" date is the year 101.
        let year = pubdate
            .as_deref()
            .and_then(normalise::year)
            .filter(|y| *y > 1000);
        let metadata = BookMetadata {
            title,
            authors: get(&authors),
            tags: tag_list.clone(),
            year,
            publisher: get(&publishers).into_iter().next(),
            isbn13,
            isbn10,
            language: get(&languages)
                .into_iter()
                .next()
                .and_then(|l| normalise::language(&l)),
            series: get(&series).into_iter().next(),
            series_number: series_index.filter(|_| series.contains_key(&id)),
            about: get(&comments)
                .into_iter()
                .next()
                .and_then(|c| normalise::about(&c)),
            doi: ident("doi"),
            arxiv_id: ident("arxiv"),
            content_type: content_type(&tag_list, &file_types),
            ..Default::default()
        };
        let mut book = ForeignBook::new(format!("calibre:{id}"), metadata);
        if files.is_empty() {
            warnings.push(format!("{}: no file Libreri can open", book.metadata.title));
        }
        book.files = files;
        let cover = folder.join("cover.jpg");
        if has_cover.unwrap_or(false) && cover.is_file() {
            book.cover = Some(cover);
        }
        let rating = get(&ratings)
            .first()
            .and_then(|r| r.parse::<f64>().ok())
            .map_or(0, |r| ((r / 2.0).round() as u8).min(5));
        if rating > 0 {
            book.personal = Some(ForeignPersonal {
                rating,
                ..Default::default()
            });
        }
        book.added_at = timestamp.as_deref().and_then(super::sql_time);
        books.push(book);
    }
    Ok(ForeignLibrary {
        source: ForeignSource::Calibre,
        books,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_details_formats_and_ratings() {
        let dir = tempfile::tempdir().unwrap();
        crate::foreign::calibre_fixture(dir.path());
        let lib = crate::foreign::read(dir.path(), ForeignSource::Calibre).unwrap();
        assert_eq!(lib.books.len(), 2);
        let dune = &lib.books[0];
        let m = &dune.metadata;
        assert_eq!(m.title, "Dune");
        assert_eq!(m.authors, ["Frank Herbert"]);
        assert_eq!(m.tags, ["Classics", "Science Fiction"]);
        assert_eq!(m.year, Some(1965));
        assert_eq!(m.publisher.as_deref(), Some("Chilton"));
        assert_eq!(m.language.as_deref(), Some("en"));
        assert_eq!(m.series.as_deref(), Some("Dune"));
        assert_eq!(m.series_number, Some(1.0));
        assert_eq!(m.isbn13.as_deref(), Some("9780441013593"));
        assert_eq!(
            m.about.as_deref(),
            Some("A desert planet, spice and a young duke.")
        );
        assert_eq!(dune.files.len(), 2);
        assert!(dune.cover.is_some());
        assert_eq!(dune.personal.as_ref().unwrap().rating, 4);
        assert_eq!(dune.added_at.as_deref(), Some("2021-03-04T05:06:07Z"));
        let notes = &lib.books[1];
        assert_eq!(notes.metadata.year, None, "Calibre's empty date");
        assert!(notes.cover.is_none());
    }

    #[test]
    fn a_plain_folder_is_not_calibre() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            read(dir.path()),
            Err(ForeignError::NotRecognised(_))
        ));
    }
}
