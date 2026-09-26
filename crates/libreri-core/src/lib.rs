//! Core domain for Libreri.
//!
//! This crate holds the plain types every other crate shares: identifiers,
//! the library folder layout, library information and settings. It has no
//! knowledge of Tauri, SQLite or the file system beyond path names.

pub mod book;
pub mod ids;
pub mod isbn;
pub mod layout;
pub mod library;
pub mod query;
pub mod settings;

pub use book::{Book, BookMetadata, BookUserState, ContentType, FileType, ReadingStatus};
pub use ids::{BookId, LibraryId, ProfileId};
pub use layout::LibraryLayout;
pub use library::{LibraryInfo, LIBRARY_FORMAT_VERSION};
pub use query::{BookQuery, SortKey};
pub use settings::{AppSettings, ThemePreference};
