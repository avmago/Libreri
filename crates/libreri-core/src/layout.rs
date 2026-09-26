//! The on-disk layout of a library folder.
//!
//! ```text
//! MyLibrary/
//!   Books/            the user's books, audiobooks and folders
//!   Notes/<profile>/  Markdown notebooks, canvases, attachments
//!   .library-data/    everything Libreri manages
//! ```
//!
//! Only relative paths are stored anywhere, so a library can be moved or
//! synced to another computer (see `docs/adr/0005-portable-links.md`).

use std::path::{Path, PathBuf};

/// Name of the folder that holds the user's book files.
pub const BOOKS_DIR: &str = "Books";
/// Name of the folder that holds per-profile notes.
pub const NOTES_DIR: &str = "Notes";
/// Name of the hidden folder that Libreri manages.
pub const DATA_DIR: &str = ".library-data";

/// Sub-folders created inside [`DATA_DIR`].
pub const DATA_SUBDIRS: &[&str] = &[
    "thumbnails",
    "covers",
    "metadata",
    "annotations",
    "page-cache",
    "versions",
    "users",
    "models",
];

/// Resolves the well-known paths of one library folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryLayout {
    root: PathBuf,
}

impl LibraryLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn books_dir(&self) -> PathBuf {
        self.root.join(BOOKS_DIR)
    }

    pub fn notes_dir(&self) -> PathBuf {
        self.root.join(NOTES_DIR)
    }

    pub fn data_dir(&self) -> PathBuf {
        self.root.join(DATA_DIR)
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir().join("library.db")
    }

    pub fn info_path(&self) -> PathBuf {
        self.data_dir().join("library.json")
    }

    pub fn lock_path(&self) -> PathBuf {
        self.data_dir().join("lock")
    }

    /// Every directory that must exist in a valid library.
    pub fn required_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.books_dir(), self.notes_dir(), self.data_dir()];
        dirs.extend(DATA_SUBDIRS.iter().map(|d| self.data_dir().join(d)));
        dirs
    }

    /// Joins a library-relative path onto the root, refusing anything that
    /// would escape the library folder (absolute paths or `..`).
    pub fn resolve_relative(&self, relative: &str) -> Option<PathBuf> {
        let rel = Path::new(relative);
        if rel.is_absolute() {
            return None;
        }
        let mut out = self.root.clone();
        for component in rel.components() {
            match component {
                std::path::Component::Normal(part) => out.push(part),
                std::path::Component::CurDir => {}
                _ => return None,
            }
        }
        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_are_inside_the_root() {
        let layout = LibraryLayout::new("/lib");
        assert_eq!(layout.books_dir(), PathBuf::from("/lib/Books"));
        assert_eq!(
            layout.database_path(),
            PathBuf::from("/lib/.library-data/library.db")
        );
        assert!(layout.required_dirs().iter().all(|d| d.starts_with("/lib")));
    }

    #[test]
    fn resolve_relative_blocks_escapes() {
        let layout = LibraryLayout::new("/lib");
        assert_eq!(
            layout.resolve_relative("Books/a/b.pdf"),
            Some(PathBuf::from("/lib/Books/a/b.pdf"))
        );
        assert_eq!(layout.resolve_relative("../etc/passwd"), None);
        assert_eq!(layout.resolve_relative("Books/../../x"), None);
        assert_eq!(layout.resolve_relative("/etc/passwd"), None);
    }
}
