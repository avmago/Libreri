//! Path helpers: library-relative paths, safe names, unique names, moves.

use crate::{Error, Result};
use libreri_core::layout::BOOKS_DIR;
use libreri_core::LibraryLayout;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};

/// Writes a file via a temporary file and rename, so readers never see half
/// a file.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    // A name of its own, so two writers of the same file never share (and
    // tear) one temporary file.
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    let tmp = path.with_file_name(name);
    let written = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)
    })();
    if written.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    written
}

/// `abs` as a library-relative path with `/` separators, if inside `root`.
pub fn rel_of(layout: &LibraryLayout, abs: &Path) -> Option<String> {
    let rel = abs.strip_prefix(layout.root()).ok()?;
    let parts: Option<Vec<&str>> = rel
        .components()
        .map(|c| match c {
            Component::Normal(p) => p.to_str(),
            _ => None,
        })
        .collect();
    let parts = parts?;
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// Absolute path of a folder given relative to `Books/` ("" = `Books/`).
pub fn folder_abs(layout: &LibraryLayout, folder: &str) -> Result<PathBuf> {
    let folder = folder.trim_matches('/');
    let rel = if folder.is_empty() {
        BOOKS_DIR.to_owned()
    } else {
        format!("{BOOKS_DIR}/{folder}")
    };
    layout
        .resolve_relative(&rel)
        .ok_or_else(|| Error::InvalidInput("that folder is outside the library".into()))
}

/// Checks a file or folder name typed by the user. The rules are the union
/// of macOS, Windows and Linux so a library works on all three.
pub fn validate_name(name: &str) -> Result<String> {
    let name = name.trim();
    let bad = |why: &str| Err(Error::InvalidInput(why.to_owned()));
    if name.is_empty() {
        return bad("the name cannot be empty");
    }
    if name == "." || name == ".." {
        return bad("that name is reserved");
    }
    if name.starts_with('.') {
        return bad("names starting with a dot are hidden; choose another name");
    }
    if let Some(c) = name.chars().find(|c| {
        matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control()
    }) {
        return Err(Error::InvalidInput(format!("names cannot contain “{c}”")));
    }
    if name.ends_with('.') {
        return bad("names cannot end with a dot");
    }
    let upper = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    let numbered = (upper.starts_with("COM") || upper.starts_with("LPT"))
        && upper.len() == 4
        && upper.as_bytes()[3].is_ascii_digit();
    if reserved.contains(&upper.as_str()) || numbered {
        return bad("that name is reserved on Windows");
    }
    // File systems count bytes (255 on most); keep room for " (2)".
    if name.chars().count() > 200 || name.len() > 240 {
        return bad("the name is too long");
    }
    Ok(name.to_owned())
}

/// A path in `dir` for `file_name` that does not exist yet:
/// "a.pdf", then "a (2).pdf", "a (3).pdf", …
pub fn unique_path(dir: &Path, file_name: &str) -> PathBuf {
    let first = dir.join(file_name);
    if !first.exists() {
        return first;
    }
    let (stem, ext) = match file_name.rsplit_once('.') {
        Some((s, e)) if !s.is_empty() => (s, format!(".{e}")),
        _ => (file_name, String::new()),
    };
    (2..)
        .map(|n| dir.join(format!("{stem} ({n}){ext}")))
        .find(|p| !p.exists())
        .expect("an unused name exists")
}

/// Moves a file, copying and deleting when the destination is on another
/// drive.
pub fn move_file(from: &Path, to: &Path) -> std::io::Result<()> {
    match fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(_) if from.is_file() => {
            fs::copy(from, to)?;
            // Keep the original if the copy is incomplete.
            if fs::metadata(to)?.len() != fs::metadata(from)?.len() {
                let _ = fs::remove_file(to);
                return Err(std::io::Error::other("copy was incomplete"));
            }
            fs::remove_file(from)
        }
        Err(e) => Err(e),
    }
}

/// Modification time in whole seconds, used to skip re-hashing.
pub fn mtime_secs(meta: &fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Hidden and system files the scanner and importer skip.
pub fn is_hidden(name: &std::ffi::OsStr) -> bool {
    let n = name.to_string_lossy();
    n.starts_with('.') || n.starts_with("~$") || n == "Thumbs.db" || n == "desktop.ini"
}

/// BLAKE3 hash of a file's content as a book id.
pub fn hash_file(path: &Path) -> std::io::Result<libreri_core::BookId> {
    let mut hasher = blake3::Hasher::new();
    hasher.update_reader(fs::File::open(path)?)?;
    Ok(
        libreri_core::BookId::from_hex(hasher.finalize().to_hex().to_string())
            .expect("blake3 hex is a valid book id"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_valid_everywhere() {
        assert_eq!(validate_name(" Physics ").unwrap(), "Physics");
        for bad in ["", "..", ".hidden", "a/b", "a:b", "CON", "com1.txt", "end."] {
            assert!(validate_name(bad).is_err(), "{bad} should be refused");
        }
        assert!(validate_name("Comics & Manga").is_ok());
        assert!(validate_name("Contracts").is_ok());
        assert!(
            validate_name(&"漢".repeat(90)).is_err(),
            "270 bytes is too long"
        );
    }

    #[test]
    fn unique_names_and_relative_paths() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(unique_path(dir.path(), "a.pdf"), dir.path().join("a.pdf"));
        fs::write(dir.path().join("a.pdf"), "").unwrap();
        fs::write(dir.path().join("a (2).pdf"), "").unwrap();
        assert_eq!(
            unique_path(dir.path(), "a.pdf"),
            dir.path().join("a (3).pdf")
        );
        let layout = LibraryLayout::new(dir.path());
        assert_eq!(
            rel_of(&layout, &dir.path().join("Books").join("x y.pdf")).as_deref(),
            Some("Books/x y.pdf")
        );
        assert!(folder_abs(&layout, "../x").is_err());
    }

    #[test]
    fn atomic_writes_leave_no_temporary_files() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.json");
        write_atomic(&p, b"1").unwrap();
        write_atomic(&p, b"2").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"2");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 1);
        assert!(write_atomic(&dir.path().join("no/such/dir.json"), b"x").is_err());
    }

    #[test]
    fn hashes_are_stable() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a");
        fs::write(&p, "hello").unwrap();
        assert_eq!(
            hash_file(&p).unwrap().as_str(),
            "ea8f163db38682925e4491c5e58d4bb3506ef8c14eb78a86e908c5624a67200f"
        );
    }
}
