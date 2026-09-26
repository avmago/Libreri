//! JSON sidecars: a plain-file backup of every book's details.
//!
//! `.library-data/metadata/<book id>.json` holds the shared metadata of one
//! book. If the database is lost or damaged, a scan rebuilds it from these
//! files (and from the book files themselves), so nothing typed by hand is
//! lost. Sidecars are kept when a book goes to the trash, so restoring the
//! file from the trash brings its details back too.

use crate::paths::write_atomic;
use libreri_core::{Book, BookId, BookMetadata, FileType, LibraryLayout};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SIDECAR_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Sidecar {
    pub format_version: u32,
    pub id: BookId,
    pub rel_path: String,
    pub file_type: FileType,
    pub file_size: u64,
    pub added_at: String,
    pub modified_at: String,
    pub metadata: BookMetadata,
}

pub fn path(layout: &LibraryLayout, id: &BookId) -> PathBuf {
    layout
        .data_dir()
        .join("metadata")
        .join(format!("{id}.json"))
}

pub fn write(layout: &LibraryLayout, book: &Book) -> std::io::Result<()> {
    let sidecar = Sidecar {
        format_version: SIDECAR_VERSION,
        id: book.id.clone(),
        rel_path: book.rel_path.clone(),
        file_type: book.file_type,
        file_size: book.file_size,
        added_at: book.added_at.clone(),
        modified_at: book.modified_at.clone(),
        metadata: book.metadata.clone(),
    };
    let json = serde_json::to_vec_pretty(&sidecar).map_err(std::io::Error::other)?;
    write_atomic(&path(layout, &book.id), &json)
}

/// Reads a sidecar; `None` if missing, damaged or from a newer app.
pub fn read(layout: &LibraryLayout, id: &BookId) -> Option<Sidecar> {
    let text = std::fs::read_to_string(path(layout, id)).ok()?;
    let s: Sidecar = serde_json::from_str(&text).ok()?;
    (s.format_version <= SIDECAR_VERSION && &s.id == id).then_some(s)
}

/// Moves a sidecar when a book's id changes (its file was edited).
pub fn rename(layout: &LibraryLayout, old: &BookId, new: &BookId) {
    let _ = std::fs::rename(path(layout, old), path(layout, new));
}
