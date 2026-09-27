//! Reading, editing, moving and deleting books.

use crate::paths::{self, folder_abs, mtime_secs, unique_path};
use crate::{covers, now, sidecar, Error, Library, Result};
use libreri_core::{Book, BookId, BookMetadata, BookQuery, BookUserState, FileType};
use libreri_db::Facets;
use std::fs;
use std::path::Path;

impl Library {
    pub fn books(&self, query: &BookQuery) -> Result<Vec<Book>> {
        let profile = self.profile();
        self.with_db(|db| db.query_books(query, &profile))
    }

    /// One book, following old ids of books whose file has changed.
    pub fn book(&self, id: &BookId) -> Result<Book> {
        let profile = self.profile();
        self.with_db(|db| {
            let Some(current) = db.resolve_book_id(id)? else {
                return Ok(None);
            };
            db.book(&current, &profile)
        })?
        .ok_or(Error::BookNotFound)
    }

    pub fn facets(&self) -> Result<Facets> {
        let profile = self.profile();
        self.with_db(|db| db.facets(&profile))
    }

    /// Saves edited details after tidying and validating them.
    pub fn update_metadata(&self, id: &BookId, metadata: BookMetadata) -> Result<Book> {
        let metadata = metadata
            .normalized()
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        if !self.with_db(|db| db.update_metadata(id, &metadata, &now()))? {
            return Err(Error::BookNotFound);
        }
        let book = self.book(id)?;
        sidecar::write(self.layout(), &book)?;
        Ok(book)
    }

    /// Saves personal state (status, rating, favourite) for the current profile.
    pub fn set_user_state(&self, id: &BookId, state: &BookUserState) -> Result<Book> {
        let profile = self.profile();
        self.book(id)?;
        self.with_db(|db| db.set_user_state(id, &profile, state))?;
        self.book(id)
    }

    /// Moves books into `folder` (relative to `Books/`). Files are renamed on
    /// disk; a name clash gets " (2)" added. Returns the moved books.
    pub fn move_books(&self, ids: &[BookId], folder: &str) -> Result<Vec<Book>> {
        let target = folder_abs(self.layout(), folder)?;
        if !target.is_dir() {
            return Err(Error::InvalidInput("that folder no longer exists".into()));
        }
        let mut moved = Vec::with_capacity(ids.len());
        for id in ids {
            let book = self.book(id)?;
            let from = self
                .layout()
                .resolve_relative(&book.rel_path)
                .ok_or(Error::BookNotFound)?;
            if from.parent() == Some(target.as_path()) {
                continue;
            }
            let name = from
                .file_name()
                .ok_or(Error::BookNotFound)?
                .to_string_lossy()
                .into_owned();
            let to = unique_path(&target, &name);
            paths::move_file(&from, &to)?;
            let rel = paths::rel_of(self.layout(), &to).ok_or(Error::BookNotFound)?;
            self.with_db(|db| db.set_book_path(&book.id, &rel))?;
            let book = self.book(&book.id)?;
            sidecar::write(self.layout(), &book)?;
            moved.push(book);
        }
        Ok(moved)
    }

    /// Moves the books' files to the system trash and removes them from the
    /// library. Their details stay in the sidecar, so restoring a file from
    /// the trash brings everything back.
    pub fn trash_books(&self, ids: &[BookId]) -> Result<usize> {
        let mut n = 0;
        for id in ids {
            let book = self.book(id)?;
            if let Some(path) = self.layout().resolve_relative(&book.rel_path) {
                if path.exists() {
                    trash::delete(&path).map_err(|e| Error::Trash(e.to_string()))?;
                }
            }
            self.with_db(|db| db.delete_book(&book.id))?;
            n += 1;
        }
        Ok(n)
    }

    /// Adds a file that is already inside `Books/` to the database: reads its
    /// details (or restores them from a sidecar), stores its cover and writes
    /// a sidecar. `warnings` collects problems for the report.
    pub(crate) fn register(
        &self,
        abs: &Path,
        id: BookId,
        file_type: FileType,
        warnings: &mut Vec<String>,
    ) -> Result<Book> {
        let meta = fs::metadata(abs)?;
        let rel = paths::rel_of(self.layout(), abs)
            .ok_or_else(|| Error::InvalidInput("the file is outside the library".into()))?;
        let now = now();

        let (metadata, added_at, cover) = match sidecar::read(self.layout(), &id) {
            Some(s) => (s.metadata, s.added_at, None),
            None => {
                let e = libreri_formats::extract(abs, file_type);
                warnings.extend(e.warnings);
                let m = e.metadata.clone().normalized().unwrap_or_else(|_| {
                    // Extracted ISBNs can be wrong; keep everything else.
                    BookMetadata {
                        isbn13: None,
                        isbn10: None,
                        ..e.metadata
                    }
                    .normalized()
                    .unwrap_or_else(|_| BookMetadata {
                        title: BookMetadata::title_from_file_name(abs),
                        content_type: file_type.default_content_type(),
                        ..Default::default()
                    })
                });
                (m, now.clone(), e.cover)
            }
        };

        let mut has_cover = covers::exists(self.layout(), &id);
        if !has_cover {
            if let Some(bytes) = cover {
                match covers::store(self.layout(), &id, &bytes) {
                    Ok(()) => has_cover = true,
                    Err(e) => warnings.push(format!("cover: {e}")),
                }
            }
        }

        let book = Book {
            id,
            rel_path: rel,
            file_type,
            file_size: meta.len(),
            has_cover,
            missing: false,
            added_at,
            modified_at: now,
            metadata,
            user: BookUserState::default(),
        };
        self.with_db(|db| db.insert_book(&book, mtime_secs(&meta)))?;
        sidecar::write(self.layout(), &book)?;
        self.restore_annotations(&book.id)?;
        Ok(book)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::{BookQuery, ReadingStatus};

    #[test]
    fn edit_move_and_trash() {
        let (dir, lib) = library();
        let src = md_book(dir.path(), "in/a.md", "Alpha");
        let report = lib
            .import(
                &ImportRequest {
                    sources: vec![src],
                    folder: String::new(),
                    mode: ImportMode::Copy,
                },
                &NoProgress,
            )
            .unwrap();
        let id = report.added_ids[0].clone();

        let mut m = lib.book(&id).unwrap().metadata;
        m.isbn13 = Some("978-0-306-40615-7".into());
        let b = lib.update_metadata(&id, m.clone()).unwrap();
        assert_eq!(b.metadata.isbn10.as_deref(), Some("0306406152"));
        m.isbn13 = Some("123".into());
        assert!(matches!(
            lib.update_metadata(&id, m),
            Err(Error::InvalidInput(_))
        ));

        let b = lib
            .set_user_state(
                &id,
                &libreri_core::BookUserState {
                    status: ReadingStatus::Finished,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(b.user.status, ReadingStatus::Finished);

        lib.create_folder("", "Maths").unwrap();
        let moved = lib.move_books(std::slice::from_ref(&id), "Maths").unwrap();
        assert_eq!(moved[0].rel_path, "Books/Maths/a.md");
        assert!(lib.layout().root().join("Books/Maths/a.md").is_file());
        let sc = sidecar::read(lib.layout(), &id).unwrap();
        assert_eq!(sc.rel_path, "Books/Maths/a.md");

        // Trashing needs a desktop trash; skip quietly where there is none.
        match lib.trash_books(&[id]) {
            Ok(n) => {
                assert_eq!(n, 1);
                assert!(lib.books(&BookQuery::default()).unwrap().is_empty());
            }
            Err(Error::Trash(_)) => {}
            Err(e) => panic!("{e}"),
        }
    }
}
