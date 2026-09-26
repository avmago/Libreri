//! The library lock.
//!
//! Two layers:
//! * an operating-system file lock on `.library-data/lock`, which stops two
//!   Libreri windows on the same computer from opening the library and is
//!   released automatically if the app crashes;
//! * the lock file's contents (computer name and time), which synced folders
//!   such as iCloud Drive or Dropbox carry to other computers, so a second
//!   computer can warn that the library is already open elsewhere.

use crate::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// Who holds the lock, written into the lock file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockOwner {
    pub host: String,
    pub pid: u32,
    pub since: DateTime<Utc>,
}

impl LockOwner {
    fn current() -> Self {
        Self {
            host: gethostname::gethostname().to_string_lossy().into_owned(),
            pid: std::process::id(),
            since: Utc::now(),
        }
    }
}

/// A held library lock. Dropping it releases the OS lock; call
/// [`LibraryLock::release`] to also clear the owner information.
pub struct LibraryLock {
    file: File,
    path: PathBuf,
}

impl LibraryLock {
    pub fn acquire(path: &Path, force: bool) -> Result<Self> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)?;

        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => return Err(Error::AlreadyOpenHere),
            Err(std::fs::TryLockError::Error(e)) => return Err(e.into()),
        }

        let me = LockOwner::current();
        if !force {
            if let Some(previous) = read_owner(&mut file) {
                if previous.host != me.host {
                    // Released below when `file` is dropped.
                    return Err(Error::LockedElsewhere {
                        host: previous.host,
                    });
                }
            }
        }

        write_owner(&mut file, Some(&me))?;
        Ok(Self {
            file,
            path: path.to_path_buf(),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Clears the owner information and releases the OS lock.
    pub fn release(mut self) -> Result<()> {
        write_owner(&mut self.file, None)?;
        self.file.unlock()?;
        Ok(())
    }
}

fn read_owner(file: &mut File) -> Option<LockOwner> {
    let mut text = String::new();
    file.seek(SeekFrom::Start(0)).ok()?;
    file.read_to_string(&mut text).ok()?;
    serde_json::from_str(text.trim()).ok()
}

fn write_owner(file: &mut File, owner: Option<&LockOwner>) -> Result<()> {
    let text = match owner {
        Some(o) => serde_json::to_string(o)?,
        None => String::new(),
    };
    file.set_len(0)?;
    file.seek(SeekFrom::Start(0))?;
    file.write_all(text.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lock_from_another_computer_blocks_unless_forced() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let other = LockOwner {
            host: "someone-elses-laptop".into(),
            pid: 1,
            since: Utc::now(),
        };
        std::fs::write(&path, serde_json::to_string(&other).unwrap()).unwrap();

        match LibraryLock::acquire(&path, false) {
            Err(Error::LockedElsewhere { host }) => assert_eq!(host, "someone-elses-laptop"),
            other => panic!("expected LockedElsewhere, got {:?}", other.map(|_| ())),
        }
        let lock = LibraryLock::acquire(&path, true).unwrap();
        lock.release().unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "");
    }

    #[test]
    fn a_stale_lock_from_this_computer_is_taken_over() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("lock");
        let crashed = LockOwner {
            pid: 999_999,
            ..LockOwner::current()
        };
        std::fs::write(&path, serde_json::to_string(&crashed).unwrap()).unwrap();
        LibraryLock::acquire(&path, false)
            .unwrap()
            .release()
            .unwrap();
    }
}
