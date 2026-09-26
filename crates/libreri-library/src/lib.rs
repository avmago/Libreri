//! Creating, opening and locking library folders.
//!
//! A library is an ordinary folder the user picks. [`Library::create`] sets up
//! the layout described in `libreri_core::layout`; [`Library::open`] checks
//! it, repairs missing folders, takes the lock and opens the database.

mod lock;

use libreri_core::{LibraryInfo, LibraryLayout, LIBRARY_FORMAT_VERSION};
use libreri_db::Database;
use std::fs;
use std::path::{Path, PathBuf};

pub use lock::{LibraryLock, LockOwner};

/// Errors when creating or opening a library.
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

/// An open library: its layout, information, database and lock.
pub struct Library {
    layout: LibraryLayout,
    info: LibraryInfo,
    db: Database,
    lock: LibraryLock,
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
        Ok(Self {
            layout,
            info,
            db,
            lock,
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

    pub fn db(&self) -> &Database {
        &self.db
    }

    /// Closes the database cleanly and releases the lock.
    pub fn close(self) -> Result<()> {
        self.db.close()?;
        self.lock.release()?;
        Ok(())
    }
}

fn write_info(layout: &LibraryLayout, info: &LibraryInfo) -> Result<()> {
    let json = serde_json::to_string_pretty(info)?;
    let tmp = layout.info_path().with_extension("json.tmp");
    fs::write(&tmp, json)?;
    fs::rename(tmp, layout.info_path())?;
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
mod tests {
    use super::*;

    const V: &str = "0.1.0-test";

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
            lib.db().meta_get("name").unwrap().as_deref(),
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
        lib.close().unwrap();

        fs::remove_dir_all(root.join("Notes")).unwrap();
        let lib = Library::open(&root, OpenOptions::default()).unwrap();
        assert_eq!(lib.info().id, id);
        assert_eq!(lib.info().name, "Research");
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
    fn a_second_open_on_the_same_computer_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("lib");
        let first = Library::create(&root, None, V).unwrap();
        assert!(matches!(
            Library::open(&root, OpenOptions::default()),
            Err(Error::AlreadyOpenHere)
        ));
        first.close().unwrap();
        Library::open(&root, OpenOptions::default())
            .unwrap()
            .close()
            .unwrap();
    }
}
