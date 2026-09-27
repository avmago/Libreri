//! Reading data: positions, annotations, notebooks.

use crate::{Database, Result};
use libreri_core::{Annotation, AnnotationKind, BookId, HighlightColor, ProfileId, TextQuote};
use rusqlite::{params, OptionalExtension, Row};

fn row_to_annotation(r: &Row<'_>) -> rusqlite::Result<Annotation> {
    let book: String = r.get(1)?;
    let kind: String = r.get(2)?;
    let color: Option<String> = r.get(3)?;
    let exact: Option<String> = r.get(5)?;
    Ok(Annotation {
        id: r.get(0)?,
        book_id: BookId::from_hex(book).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, e.into())
        })?,
        kind: AnnotationKind::parse(&kind).unwrap_or(AnnotationKind::Highlight),
        color: color.as_deref().and_then(HighlightColor::parse),
        locator: r.get(4)?,
        quote: exact.map(|exact| TextQuote {
            exact,
            prefix: r
                .get::<_, Option<String>>(6)
                .ok()
                .flatten()
                .unwrap_or_default(),
            suffix: r
                .get::<_, Option<String>>(7)
                .ok()
                .flatten()
                .unwrap_or_default(),
        }),
        note: r.get(8)?,
        label: r.get(9)?,
        position: r.get(10)?,
        created_at: r.get(11)?,
        modified_at: r.get(12)?,
    })
}

const ANNOTATION_COLUMNS: &str = "id, book_id, kind, color, locator, quote, prefix, suffix, \
     note, label, position, created_at, modified_at";

impl Database {
    /// Saves where the profile is in a book, and how far through (0–1).
    /// Also marks the book as opened now.
    pub fn set_position(
        &self,
        book: &BookId,
        profile: &ProfileId,
        locator: &str,
        progress: f32,
        now: &str,
    ) -> Result<()> {
        self.conn.execute(
            "INSERT INTO book_user(book_id, profile_id, position, progress, last_opened)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(book_id, profile_id) DO UPDATE SET position=excluded.position,
                progress=excluded.progress, last_opened=excluded.last_opened",
            params![
                book.as_str(),
                profile.to_string(),
                locator,
                f64::from(progress.clamp(0.0, 1.0)),
                now
            ],
        )?;
        Ok(())
    }

    pub fn position(&self, book: &BookId, profile: &ProfileId) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT position FROM book_user WHERE book_id=?1 AND profile_id=?2",
                params![book.as_str(), profile.to_string()],
                |r| r.get(0),
            )
            .optional()?
            .flatten())
    }

    /// All annotations of one profile in one book, in reading order.
    pub fn annotations(&self, book: &BookId, profile: &ProfileId) -> Result<Vec<Annotation>> {
        let mut stmt = self.conn.prepare_cached(&format!(
            "SELECT {ANNOTATION_COLUMNS} FROM annotations
             WHERE book_id=?1 AND profile_id=?2 ORDER BY position, created_at"
        ))?;
        let rows = stmt.query_map(
            params![book.as_str(), profile.to_string()],
            row_to_annotation,
        )?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn annotation(&self, id: &str) -> Result<Option<(Annotation, ProfileId)>> {
        Ok(self
            .conn
            .query_row(
                &format!("SELECT {ANNOTATION_COLUMNS}, profile_id FROM annotations WHERE id=?1"),
                [id],
                |r| {
                    let profile: String = r.get(13)?;
                    Ok((row_to_annotation(r)?, profile))
                },
            )
            .optional()?
            .and_then(|(a, p)| p.parse().ok().map(|p| (a, p))))
    }

    /// Adds or replaces an annotation.
    pub fn save_annotation(&self, a: &Annotation, profile: &ProfileId) -> Result<()> {
        let (exact, prefix, suffix) = match &a.quote {
            Some(q) => (Some(&q.exact), Some(&q.prefix), Some(&q.suffix)),
            None => (None, None, None),
        };
        self.conn.execute(
            &format!(
                "INSERT INTO annotations({ANNOTATION_COLUMNS}, profile_id)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                 ON CONFLICT(id) DO UPDATE SET kind=excluded.kind, color=excluded.color,
                    locator=excluded.locator, quote=excluded.quote, prefix=excluded.prefix,
                    suffix=excluded.suffix, note=excluded.note, label=excluded.label,
                    position=excluded.position, modified_at=excluded.modified_at"
            ),
            params![
                a.id,
                a.book_id.as_str(),
                a.kind.as_str(),
                a.color.map(HighlightColor::as_str),
                a.locator,
                exact,
                prefix,
                suffix,
                a.note,
                a.label,
                a.position,
                a.created_at,
                a.modified_at,
                profile.to_string(),
            ],
        )?;
        Ok(())
    }

    pub fn delete_annotation(&self, id: &str) -> Result<bool> {
        Ok(self
            .conn
            .execute("DELETE FROM annotations WHERE id=?1", [id])?
            > 0)
    }

    pub fn notebook_path(&self, book: &BookId, profile: &ProfileId) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT rel_path FROM notebooks WHERE book_id=?1 AND profile_id=?2",
                params![book.as_str(), profile.to_string()],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_notebook_path(&self, book: &BookId, profile: &ProfileId, rel: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO notebooks(book_id, profile_id, rel_path) VALUES (?1, ?2, ?3)
             ON CONFLICT(book_id, profile_id) DO UPDATE SET rel_path=excluded.rel_path",
            params![book.as_str(), profile.to_string(), rel],
        )?;
        Ok(())
    }

    pub fn profile_name(&self, profile: &ProfileId) -> Result<Option<String>> {
        Ok(self
            .conn
            .query_row(
                "SELECT name FROM profiles WHERE id=?1",
                [profile.to_string()],
                |r| r.get(0),
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use libreri_core::{Book, BookMetadata, FileType};

    const NOW: &str = "2026-09-27T10:00:00Z";

    fn setup() -> (Database, ProfileId, BookId) {
        let db = Database::open_in_memory().unwrap();
        let p = db.ensure_owner_profile("Owner", NOW).unwrap();
        let id = BookId::from_hex("b".repeat(64)).unwrap();
        db.insert_book(
            &Book {
                id: id.clone(),
                rel_path: "Books/a.pdf".into(),
                file_type: FileType::Pdf,
                file_size: 1,
                has_cover: false,
                missing: false,
                added_at: NOW.into(),
                modified_at: NOW.into(),
                metadata: BookMetadata {
                    title: "A".into(),
                    ..Default::default()
                },
                user: Default::default(),
            },
            0,
        )
        .unwrap();
        (db, p, id)
    }

    fn highlight(book: &BookId, id: &str, position: f64) -> Annotation {
        Annotation {
            id: id.into(),
            book_id: book.clone(),
            kind: AnnotationKind::Highlight,
            color: Some(HighlightColor::Green),
            locator: r#"{"type":"pdf","page":2}"#.into(),
            quote: Some(TextQuote {
                exact: "words".into(),
                prefix: "some ".into(),
                suffix: " here".into(),
            }),
            note: Some("why?".into()),
            label: Some("p. 2".into()),
            position,
            created_at: NOW.into(),
            modified_at: NOW.into(),
        }
    }

    #[test]
    fn annotations_round_trip_in_reading_order() {
        let (db, p, book) = setup();
        db.save_annotation(&highlight(&book, "b", 0.5), &p).unwrap();
        db.save_annotation(&highlight(&book, "a", 0.1), &p).unwrap();
        let mut edited = highlight(&book, "b", 0.5);
        edited.note = None;
        db.save_annotation(&edited, &p).unwrap();

        let list = db.annotations(&book, &p).unwrap();
        assert_eq!(
            list.iter().map(|a| a.id.as_str()).collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        assert_eq!(list[1].note, None);
        assert_eq!(list[0].quote.as_ref().unwrap().prefix, "some ");
        assert_eq!(db.annotation("a").unwrap().unwrap().1, p);
        assert!(db.delete_annotation("a").unwrap());
        assert_eq!(db.annotations(&book, &p).unwrap().len(), 1);
        assert!(db.annotations(&book, &ProfileId::new()).unwrap().is_empty());
    }

    #[test]
    fn positions_and_notebooks() {
        let (db, p, book) = setup();
        assert_eq!(db.position(&book, &p).unwrap(), None);
        db.set_position(&book, &p, r#"{"page":3}"#, 0.3, NOW)
            .unwrap();
        assert_eq!(
            db.position(&book, &p).unwrap().as_deref(),
            Some(r#"{"page":3}"#)
        );
        let b = db.book(&book, &p).unwrap().unwrap();
        assert_eq!(b.user.last_opened.as_deref(), Some(NOW));
        assert!((b.user.progress - 0.3).abs() < 1e-6);

        db.set_notebook_path(&book, &p, "Notes/Owner/A.md").unwrap();
        assert_eq!(
            db.notebook_path(&book, &p).unwrap().as_deref(),
            Some("Notes/Owner/A.md")
        );
        assert_eq!(db.profile_name(&p).unwrap().as_deref(), Some("Owner"));
    }
}
