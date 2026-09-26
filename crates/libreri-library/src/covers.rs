//! Stored covers: `.library-data/covers/<id>.jpg` and
//! `.library-data/thumbnails/<id>.jpg`.

use crate::paths::write_atomic;
use crate::{Library, Result};
use libreri_core::{BookId, LibraryLayout};
use std::path::PathBuf;

pub fn cover_path(layout: &LibraryLayout, id: &BookId) -> PathBuf {
    layout.data_dir().join("covers").join(format!("{id}.jpg"))
}

pub fn thumbnail_path(layout: &LibraryLayout, id: &BookId) -> PathBuf {
    layout
        .data_dir()
        .join("thumbnails")
        .join(format!("{id}.jpg"))
}

/// Library-relative path of a thumbnail, for the `book://` protocol.
pub fn thumbnail_rel(id: &BookId) -> String {
    format!("{}/thumbnails/{id}.jpg", libreri_core::layout::DATA_DIR)
}

/// Library-relative path of a cover, for the `book://` protocol.
pub fn cover_rel(id: &BookId) -> String {
    format!("{}/covers/{id}.jpg", libreri_core::layout::DATA_DIR)
}

/// Makes and writes the cover and thumbnail. Returns an error if `bytes`
/// is not an image.
pub fn store(layout: &LibraryLayout, id: &BookId, bytes: &[u8]) -> Result<()> {
    let covers = libreri_thumbs::make_covers(bytes)
        .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;
    write_atomic(&cover_path(layout, id), &covers.cover)?;
    write_atomic(&thumbnail_path(layout, id), &covers.thumbnail)?;
    Ok(())
}

pub fn exists(layout: &LibraryLayout, id: &BookId) -> bool {
    thumbnail_path(layout, id).is_file()
}

pub fn rename(layout: &LibraryLayout, old: &BookId, new: &BookId) {
    let _ = std::fs::rename(cover_path(layout, old), cover_path(layout, new));
    let _ = std::fs::rename(thumbnail_path(layout, old), thumbnail_path(layout, new));
}

impl Library {
    /// Replaces a book's cover with `bytes` (any common image format). Used
    /// for PDF first pages rendered by the interface, and later for covers
    /// found online.
    pub fn set_cover(&self, id: &BookId, bytes: &[u8]) -> Result<()> {
        if self.with_db(|db| db.resolve_book_id(id))?.as_ref() != Some(id) {
            return Err(crate::Error::BookNotFound);
        }
        store(self.layout(), id, bytes)?;
        self.with_db(|db| db.set_has_cover(id, true, &crate::now()))
    }
}
