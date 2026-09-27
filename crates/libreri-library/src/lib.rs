//! Library folders and everything that changes files inside them.
//!
//! A library is an ordinary folder the user picks. [`Library::create`] sets up
//! the layout described in `libreri_core::layout`; [`Library::open`] checks
//! it, repairs missing folders, takes the lock and opens the database.
//!
//! The book files under `Books/` are the source of truth: importing, moving
//! and deleting change the files first and then the database; the scanner
//! ([`Library::scan`]) brings the database back in line with whatever the
//! user did in their file manager.
//!
//! A `Library` is shared between threads (`Arc<Library>`): the database sits
//! behind a mutex, and long operations (import, scan) run one at a time.

mod books;
mod covers;
mod folders;
mod import;
mod lock;
mod paths;
mod reading;
mod scan;
mod sidecar;
mod watcher;

use libreri_core::{LibraryInfo, LibraryLayout, ProfileId, LIBRARY_FORMAT_VERSION};
use libreri_db::Database;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

pub use covers::{cover_rel, thumbnail_rel};
pub use folders::FolderNode;
pub use import::{Duplicate, ImportMode, ImportReport, ImportRequest};
pub use libreri_db::Facets;
pub use lock::{LibraryLock, LockOwner};
pub use reading::Notebook;
pub use scan::ScanReport;
pub use watcher::LibraryWatcher;

/// Errors from library operations. Messages are written for people.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the folder {0} already contains a Libreri library")]
    AlreadyALibrary(PathBuf),
    #[error(
        "the folder {0} is not empty; choose an empty folder or open it as an existing library"
    )]
    FolderNotEmpty(PathBuf),
    #[error("no Libreri library was found in {0}")]
    NotALibrary(PathBuf),
    #[error("this library needs a newer version of Libreri (library format {found}, this app supports {supported})")]
    FormatTooNew { found: u32, supported: u32 },
    #[error(
        "the library is open on another computer ({host}). Close it there first, or open it anyway"
    )]
    LockedElsewhere { host: String },
    #[error("the library is already open in another Libreri window on this computer")]
    AlreadyOpenHere,
    #[error("library information file is damaged: {0}")]
    BadInfo(#[from] serde_json::Error),
    #[error("the library has been closed")]
    Closed,
    #[error("book not found")]
    BookNotFound,
    #[error("{0}")]
    InvalidInput(String),
    #[error("a file or folder called “{0}” already exists there")]
    NameTaken(String),
    #[error("could not move to the trash: {0}")]
    Trash(String),
    #[error("cancelled")]
    Cancelled,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Db(#[from] libreri_db::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Options for [`Library::open`].
#[derive(Debug, Clone, Copy, Default)]
pub struct OpenOptions {
    /// Take over a lock left by another computer (after the user confirms).
    pub force: bool,
}

/// Progress reporting and cancellation for long operations.
pub trait Progress {
    fn report(&self, done: u64, total: u64, message: &str);
    fn cancelled(&self) -> bool;
}

/// A [`Progress`] that ignores everything, for tests and quick calls.
pub struct NoProgress;

impl Progress for NoProgress {
    fn report(&self, _: u64, _: u64, _: &str) {}
    fn cancelled(&self) -> bool {
        false
    }
}

/// An open library: its layout, information, database and lock.
pub struct Library {
    layout: LibraryLayout,
    info: LibraryInfo,
    profile: ProfileId,
    db: Mutex<Option<Database>>,
    lock: Mutex<Option<LibraryLock>>,
    /// Held by imports and scans so they never run at the same time.
    busy: Mutex<()>,
}

/// Current time as an RFC 3339 string in UTC.
pub(crate) fn now() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
}

fn default_owner_name() -> String {
    std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .ok()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| "Owner".to_owned())
}

impl Library {
    /// Creates a new library in `root` (the folder may not exist yet, or must
    /// be empty) and opens it.
    pub fn create(root: &Path, name: Option<&str>, app_version: &str) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        if layout.info_path().exists() {
            return Err(Error::AlreadyALibrary(root.to_path_buf()));
        }
        if root.exists() && fs::read_dir(root)?.next().is_some() {
            return Err(Error::FolderNotEmpty(root.to_path_buf()));
        }
        for dir in layout.required_dirs() {
            fs::create_dir_all(dir)?;
        }
        let name = name
            .map(str::to_owned)
            .or_else(|| root.file_name().map(|n| n.to_string_lossy().into_owned()))
            .unwrap_or_else(|| "Library".to_owned());
        let info = LibraryInfo::new(name, app_version);
        write_info(&layout, &info)?;
        Self::open(root, OpenOptions::default())
    }

    /// Opens an existing library.
    pub fn open(root: &Path, options: OpenOptions) -> Result<Self> {
        let layout = LibraryLayout::new(root);
        let info = read_info(&layout)?;
        if info.format_version > LIBRARY_FORMAT_VERSION {
            return Err(Error::FormatTooNew {
                found: info.format_version,
                supported: LIBRARY_FORMAT_VERSION,
            });
        }
        // Recreate any folder the user deleted by accident.
        for dir in layout.required_dirs() {
            fs::create_dir_all(dir)?;
        }
        let lock = LibraryLock::acquire(&layout.lock_path(), options.force)?;
        let db = Database::open(&layout.database_path())?;
        db.meta_set("library_id", &info.id.to_string())?;
        db.meta_set("name", &info.name)?;
        let profile = db.ensure_owner_profile(&default_owner_name(), &now())?;
        Ok(Self {
            layout,
            info,
            profile,
            db: Mutex::new(Some(db)),
            lock: Mutex::new(Some(lock)),
            busy: Mutex::new(()),
        })
    }

    /// Returns `true` if `root` looks like a Libreri library.
    pub fn is_library(root: &Path) -> bool {
        LibraryLayout::new(root).info_path().is_file()
    }

    pub fn layout(&self) -> &LibraryLayout {
        &self.layout
    }

    pub fn info(&self) -> &LibraryInfo {
        &self.info
    }

    /// The profile personal data is read and written for. Phase 3 lets the
    /// user pick; until then it is the library owner.
    pub fn profile(&self) -> ProfileId {
        self.profile
    }

    /// Runs `f` with the database.
    pub fn with_db<T>(&self, f: impl FnOnce(&Database) -> libreri_db::Result<T>) -> Result<T> {
        let guard = self.db_guard()?;
        let db = guard.as_ref().ok_or(Error::Closed)?;
        Ok(f(db)?)
    }

    fn db_guard(&self) -> Result<MutexGuard<'_, Option<Database>>> {
        // A panic while holding the lock leaves the data intact (every write
        // is a transaction), so keep going.
        Ok(self.db.lock().unwrap_or_else(|p| p.into_inner()))
    }

    pub(crate) fn busy(&self) -> MutexGuard<'_, ()> {
        self.busy.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Closes the database cleanly and releases the lock. Safe to call more
    /// than once; also runs when the last reference is dropped.
    pub fn shutdown(&self) -> Result<()> {
        let db = self.db_guard()?.take();
        let lock = self.lock.lock().unwrap_or_else(|p| p.into_inner()).take();
        if let Some(db) = db {
            db.close()?;
        }
        if let Some(lock) = lock {
            lock.release()?;
        }
        Ok(())
    }

    /// Closes the library (see [`Library::shutdown`]).
    pub fn close(self) -> Result<()> {
        self.shutdown()
    }
}

impl Drop for Library {
    fn drop(&mut self) {
        let _ = self.shutdown();
    }
}

fn write_info(layout: &LibraryLayout, info: &LibraryInfo) -> Result<()> {
    let json = serde_json::to_string_pretty(info)?;
    paths::write_atomic(&layout.info_path(), json.as_bytes())?;
    Ok(())
}

fn read_info(layout: &LibraryLayout) -> Result<LibraryInfo> {
    let path = layout.info_path();
    if !path.is_file() {
        return Err(Error::NotALibrary(layout.root().to_path_buf()));
    }
    Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;

    pub const V: &str = "0.1.0-test";

    pub fn library() -> (tempfile::TempDir, Library) {
        let dir = tempfile::tempdir().unwrap();
        let lib = Library::create(&dir.path().join("lib"), None, V).unwrap();
        (dir, lib)
    }

    /// Writes a small Markdown "book" with a title in its front matter.
    pub fn md_book(dir: &Path, name: &str, title: &str) -> PathBuf {
        let p = dir.join(name);
        if let Some(parent) = p.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&p, format!("---\ntitle: {title}\n---\nBody of {title}\n")).unwrap();
        p
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::V;
    use super::*;

    #[test]
    fn create_makes_the_full_layout() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("MyLibrary");
        let lib = Library::create(&root, None, V).unwrap();
        assert_eq!(lib.info().name, "MyLibrary");
        for d in lib.layout().required_dirs() {
            assert!(d.is_dir(), "missing {}", d.display());
        }
        assert!(lib.layout().database_path().is_file());
        assert_eq!(
            lib.with_db(|db| db.meta_get("name")).unwrap().as_deref(),
            Some("MyLibrary")
        );
        lib.close().unwrap();
        assert!(Library::is_library(&root));
    }

    #[test]
    fn create_refuses_non_empty_folders_and_existing_libraries() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("stray.txt"), "x").unwrap();
        assert!(matches!(
            Library::create(dir.path(), None, V),
            Err(Error::FolderNotEmpty(_))
        ));

        let root = dir.path().join("lib");
        Library::create(&root, None, V).unwrap().close().unwrap();
        assert!(matches!(
            Library::create(&root, None, V),
            Err(Error::AlreadyALibrary(_))
        ));
    }

    #[test]
    fn open_repairs_missing_folders_and_keeps_identity() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        let lib = Library::create(&root, Some("Research"), V).unwrap();
        let id = lib.info().id;
        let profile = lib.profile();
        lib.close().unwrap();

        fs::remove_dir_all(root.join("Notes")).unwrap();
        let lib = Library::open(&root, OpenOptions::default()).unwrap();
        assert_eq!(lib.info().id, id);
        assert_eq!(lib.info().name, "Research");
        assert_eq!(lib.profile(), profile, "owner profile is kept");
        assert!(root.join("Notes").is_dir());
        lib.close().unwrap();
    }

    #[test]
    fn open_rejects_plain_folders() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            Library::open(dir.path(), OpenOptions::default()),
            Err(Error::NotALibrary(_))
        ));
    }

    #[test]
    fn a_library_can_be_moved_to_another_path() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let lib = Library::create(&a, None, V).unwrap();
        let id = lib.info().id;
        lib.close().unwrap();

        let b = dir.path().join("moved");
        fs::rename(&a, &b).unwrap();
        let lib = Library::open(&b, OpenOptions::default()).unwrap();
        assert_eq!(lib.info().id, id);
        lib.close().unwrap();
    }

    #[test]
    fn a_second_open_on_the_same_computer_is_refused_until_dropped() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        let first = Library::create(&root, None, V).unwrap();
        assert!(matches!(
            Library::open(&root, OpenOptions::default()),
            Err(Error::AlreadyOpenHere)
        ));
        drop(first); // dropping closes cleanly too
        let lib = Library::open(&root, OpenOptions::default()).unwrap();
        lib.shutdown().unwrap();
        assert!(matches!(
            lib.with_db(|db| db.book_count()),
            Err(Error::Closed)
        ));
    }
}
