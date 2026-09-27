//! Library health check, locating missing files, and the backup folder.

use crate::export::ExportReport;
use crate::import::ImportMode;
use crate::paths::{self, unique_path};
use crate::{covers, now, sidecar, Error, Library, Result};
use libreri_core::{AliasKind, BookId, FileType};
use libreri_export::archive::{ArchiveKind, ArchiveReader};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// A link in a note that points to no book in this library.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrokenLink {
    /// Library-relative path of the note.
    pub note: String,
    /// 1-based line number.
    pub line: u32,
    pub link: String,
}

/// What the health check found. Empty lists mean all is well.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HealthReport {
    pub checked_at: String,
    pub books: u32,
    /// Books whose file cannot be found (their notes are kept).
    pub missing_files: Vec<BookId>,
    /// Books with notes made in another copy or edition; those notes find
    /// their place by the quoted text. (book, number of notes)
    pub other_file_notes: Vec<(BookId, u32)>,
    /// Books that share an ISBN or DOI: ("ISBN 978…", books).
    pub duplicates: Vec<(String, Vec<BookId>)>,
    /// In the signed-in profile's notes.
    pub broken_links: Vec<BrokenLink>,
    /// Books without a JSON backup of their details (fixable).
    pub missing_sidecars: u32,
    /// Notebooks the catalogue points to that are gone (fixable).
    pub stale_notebooks: u32,
    /// Covers and thumbnails of books no longer in the library (fixable).
    pub unused_covers: u32,
    pub unused_cover_bytes: u64,
    /// Backups of personal data that cannot be read.
    pub unreadable_backups: Vec<String>,
    /// Problems in the catalogue file itself (fix: Rebuild library index).
    pub database: Vec<String>,
}

impl HealthReport {
    pub fn fixable(&self) -> bool {
        self.missing_sidecars + self.stale_notebooks + self.unused_covers > 0
    }
}

/// What "Locate file…" did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocateOutcome {
    /// The file is the book: it is back in the library.
    Linked,
    /// The file is a different file (another copy or edition). Nothing was
    /// changed; call again with `accept_other` to use it anyway.
    DifferentFile,
}

/// A backup in the backup folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupFile {
    pub path: PathBuf,
    pub created_at: String,
    pub size: u64,
    pub includes_book_files: bool,
    pub books: u32,
}

fn hex_at(s: &str) -> Option<&str> {
    let hex: usize = s
        .char_indices()
        .take_while(|(_, c)| c.is_ascii_hexdigit())
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);
    (hex == 64).then(|| &s[..64])
}

impl Library {
    /// Checks the library. Anyone who may change it can run the check;
    /// broken links are looked for in their own notes only.
    pub fn health_check(&self) -> Result<HealthReport> {
        self.require_edit()?;
        let me = self.profile()?;
        let mut r = HealthReport {
            checked_at: now(),
            ..Default::default()
        };
        let records = self.with_db(|db| db.file_records())?;
        r.books = records.len() as u32;
        let known: HashSet<String> = records.iter().map(|b| b.id.to_string()).collect();
        for rec in &records {
            if rec.missing {
                r.missing_files.push(rec.id.clone());
            }
            if !sidecar::path(self.layout(), &rec.id).is_file() {
                r.missing_sidecars += 1;
            }
        }
        r.other_file_notes = self.with_db(|db| db.books_with_other_file_notes())?;
        r.duplicates = self.with_db(|db| db.books_sharing_identifiers())?;

        for (_, _, rel) in self.with_db(|db| db.notebook_rows())? {
            let gone = self
                .layout()
                .resolve_relative(&rel)
                .is_none_or(|p| !p.is_file());
            if gone {
                r.stale_notebooks += 1;
            }
        }

        // Broken links in the signed-in profile's notes.
        if let Ok((_, files)) = self.note_files(&me) {
            for f in files.iter().filter(|f| f.inner.ends_with(".md")) {
                let Ok(text) = fs::read_to_string(&f.abs) else {
                    continue;
                };
                let rel = paths::rel_of(self.layout(), &f.abs).unwrap_or_default();
                for (n, line) in text.lines().enumerate() {
                    let mut rest = line;
                    while let Some(at) = rest.find("libreri://book/") {
                        let after = &rest[at + "libreri://book/".len()..];
                        let resolves = hex_at(after)
                            .and_then(|h| BookId::from_hex(h).ok())
                            .map(|id| self.with_db(|db| db.resolve_book_id(&id)))
                            .transpose()?
                            .flatten()
                            .is_some();
                        if !resolves {
                            let link: String = rest[at..]
                                .chars()
                                .take_while(|c| {
                                    !c.is_whitespace() && !matches!(c, ')' | ']' | '>' | '"')
                                })
                                .collect();
                            r.broken_links.push(BrokenLink {
                                note: rel.clone(),
                                line: n as u32 + 1,
                                link,
                            });
                        }
                        rest = after;
                    }
                }
            }
        }

        // Covers of books that are gone.
        for dir in ["covers", "thumbnails"] {
            let Ok(entries) = fs::read_dir(self.layout().data_dir().join(dir)) else {
                continue;
            };
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                let Some(id) = name.strip_suffix(".jpg") else {
                    continue;
                };
                if !known.contains(id) {
                    r.unused_covers += 1;
                    r.unused_cover_bytes += e.metadata().map_or(0, |m| m.len());
                }
            }
        }

        // Personal backups that cannot be read.
        for e in walkdir::WalkDir::new(self.layout().data_dir().join("annotations"))
            .max_depth(2)
            .into_iter()
            .flatten()
        {
            if e.file_type().is_file() && e.path().extension().is_some_and(|x| x == "json") {
                let ok = fs::read_to_string(e.path())
                    .ok()
                    .and_then(|t| crate::reading::read_backup(&t))
                    .is_some();
                if !ok {
                    r.unreadable_backups
                        .push(paths::rel_of(self.layout(), e.path()).unwrap_or_default());
                }
            }
        }
        r.database = self.with_db(|db| db.check())?;
        Ok(r)
    }

    /// Fixes what the health check can fix by itself: writes missing
    /// sidecars, forgets notebooks that are gone, removes unused covers.
    /// Returns how many things were fixed.
    pub fn repair_health(&self) -> Result<u32> {
        self.require_edit()?;
        let mut fixed = 0;
        let records = self.with_db(|db| db.file_records())?;
        let known: HashSet<String> = records.iter().map(|b| b.id.to_string()).collect();
        for rec in &records {
            if !sidecar::path(self.layout(), &rec.id).is_file() {
                sidecar::write(self, &self.record(&rec.id)?)?;
                fixed += 1;
            }
        }
        for (book, profile, rel) in self.with_db(|db| db.notebook_rows())? {
            let gone = self
                .layout()
                .resolve_relative(&rel)
                .is_none_or(|p| !p.is_file());
            if gone {
                self.with_db(|db| db.delete_notebook_row(&book, &profile))?;
                fixed += 1;
            }
        }
        for dir in ["covers", "thumbnails"] {
            let Ok(entries) = fs::read_dir(self.layout().data_dir().join(dir)) else {
                continue;
            };
            for e in entries.flatten() {
                let name = e.file_name().to_string_lossy().into_owned();
                if name
                    .strip_suffix(".jpg")
                    .is_some_and(|id| !known.contains(id))
                    && fs::remove_file(e.path()).is_ok()
                {
                    fixed += 1;
                }
            }
        }
        Ok(fixed)
    }

    /// Puts a missing book's file back from `src`. When `src` is a
    /// different file (another copy or edition), nothing changes unless
    /// `accept_other`; then the book takes the new file and its notes find
    /// their place by the quoted text.
    pub fn locate_file(
        &self,
        id: &BookId,
        src: &Path,
        mode: ImportMode,
        accept_other: bool,
    ) -> Result<LocateOutcome> {
        self.require_edit()?;
        let _busy = self.busy();
        let book = self.book(id)?;
        let file_type = FileType::from_path(src)
            .ok_or_else(|| Error::InvalidInput("Libreri cannot open this kind of file".into()))?;
        let new_id = paths::hash_file(src)?;
        let same = self.with_db(|db| db.resolve_book_id(&new_id))?;
        match &same {
            Some(existing) if *existing == book.id => {}
            Some(existing) => {
                let other = self.record(existing)?;
                return Err(Error::InvalidInput(format!(
                    "that file is already in the library as “{}”",
                    other.metadata.title
                )));
            }
            None if !accept_other => return Ok(LocateOutcome::DifferentFile),
            None => {}
        }

        // Where it goes: the book's own place if free, else its folder.
        let wanted = self
            .layout()
            .resolve_relative(&book.rel_path)
            .filter(|_| book.rel_path.starts_with("Books/"))
            .unwrap_or_else(|| self.layout().books_dir().join("x"));
        let dir = wanted
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.layout().books_dir());
        fs::create_dir_all(&dir)?;
        let mut name = src
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "book".into());
        if same.is_some() {
            if let Some(n) = wanted.file_name() {
                name = n.to_string_lossy().into_owned();
            }
        }
        let dest = if src.starts_with(self.layout().books_dir()) {
            src.to_path_buf()
        } else {
            let dest = unique_path(&dir, &name);
            match mode {
                ImportMode::Copy => {
                    fs::copy(src, &dest)?;
                }
                ImportMode::Move => paths::move_file(src, &dest)?,
            }
            dest
        };
        let rel = paths::rel_of(self.layout(), &dest).ok_or(Error::BookNotFound)?;
        let meta = fs::metadata(&dest)?;
        let mtime = paths::mtime_secs(&meta);
        if same.is_some() {
            self.with_db(|db| {
                db.set_book_path(&book.id, &rel)?;
                db.set_file_stamp(&book.id, meta.len(), mtime)
            })?;
            sidecar::write(self, &self.record(&book.id)?)?;
        } else {
            self.with_db(|db| {
                db.change_book_id(&book.id, &new_id, meta.len(), mtime, &now())?;
                db.add_alias(&book.id, &new_id, AliasKind::OtherFile)?;
                db.set_book_path(&new_id, &rel)?;
                db.set_file_type(&new_id, file_type)
            })?;
            sidecar::rename(self.layout(), &book.id, &new_id);
            covers::rename(self.layout(), &book.id, &new_id);
            self.rename_annotation_backups(&book.id, &new_id);
            sidecar::write(self, &self.record(&new_id)?)?;
        }
        Ok(LocateOutcome::Linked)
    }

    /// A name for a new backup: "<library> 2026-09-28 1430.libreri".
    pub fn backup_file_name(&self) -> String {
        let when = chrono::Local::now().format("%Y-%m-%d %H%M");
        format!(
            "{} {when}.libreri",
            libreri_export::safe_file_name(&self.info().name)
        )
    }

    /// Writes a backup into `folder` and removes the oldest backups of this
    /// library beyond `keep`. No one needs to be signed in.
    pub fn back_up_into(
        &self,
        folder: &Path,
        keep: usize,
        book_files: bool,
        app_version: &str,
        progress: &dyn crate::Progress,
    ) -> Result<ExportReport> {
        fs::create_dir_all(folder)?;
        let dest = unique_path(folder, &self.backup_file_name());
        let report = self.backup_to(&dest, book_files, app_version, progress)?;
        let backups = list_backups(folder, &self.info().id.to_string());
        for old in backups.iter().skip(keep.max(1)) {
            let _ = fs::remove_file(&old.path);
        }
        Ok(report)
    }
}

/// This library's backups in `folder`, newest first. Other files, and
/// backups of other libraries, are left out (and never removed).
pub fn list_backups(folder: &Path, library_id: &str) -> Vec<BackupFile> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut out: Vec<BackupFile> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x == "libreri"))
        .filter_map(|e| {
            let r = ArchiveReader::open(&e.path()).ok()?;
            let m = &r.manifest;
            (m.kind == ArchiveKind::Backup && m.library.id == library_id).then(|| BackupFile {
                path: e.path(),
                created_at: m.created_at.clone(),
                size: e.metadata().map_or(0, |m| m.len()),
                includes_book_files: m.includes_book_files,
                books: m.books.len() as u32,
            })
        })
        .collect();
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(b.path.cmp(&a.path)));
    out
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::BookQuery;
    use std::fs;

    #[test]
    fn health_check_finds_and_fixes_problems() {
        let (dir, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "Alpha");
        md_book(&lib.layout().books_dir(), "b.md", "Beta");
        lib.scan(&NoProgress).unwrap();
        let books = lib.books(&BookQuery::default()).unwrap();
        let a = books.iter().find(|b| b.metadata.title == "Alpha").unwrap();

        let clean = lib.health_check().unwrap();
        assert_eq!(clean.books, 2);
        assert!(clean.missing_files.is_empty() && !clean.fixable());
        assert!(clean.database.is_empty());

        // Break things.
        fs::remove_file(lib.layout().books_dir().join("b.md")).unwrap();
        lib.scan(&NoProgress).unwrap();
        fs::remove_file(sidecar::path(lib.layout(), &a.id)).unwrap();
        fs::write(lib.layout().data_dir().join("covers/0000.jpg"), b"x").unwrap();
        let nb = lib.notebook(&a.id).unwrap();
        let note_path = lib.layout().resolve_relative(&nb.rel_path).unwrap();
        fs::write(
            &note_path,
            format!(
                "{}See [x](libreri://book/{}) and [y](libreri://book/{}#annotation=1)\n",
                nb.content,
                a.id,
                "f".repeat(64)
            ),
        )
        .unwrap();

        let r = lib.health_check().unwrap();
        assert_eq!(r.missing_files.len(), 1);
        assert_eq!(r.missing_sidecars, 1);
        assert_eq!(r.unused_covers, 1);
        assert_eq!(r.broken_links.len(), 1);
        assert!(r.broken_links[0].link.starts_with("libreri://book/ffff"));
        assert!(r.broken_links[0].link.ends_with("#annotation=1"));

        fs::remove_file(&note_path).unwrap();
        let r = lib.health_check().unwrap();
        assert_eq!(r.stale_notebooks, 1);
        assert_eq!(lib.repair_health().unwrap(), 3);
        let r = lib.health_check().unwrap();
        assert!(!r.fixable(), "{r:?}");
        drop(dir);
    }

    #[test]
    fn locating_a_missing_file() {
        let (dir, lib) = library();
        md_book(&lib.layout().books_dir(), "Physics/a.md", "Alpha");
        lib.scan(&NoProgress).unwrap();
        let a = lib.books(&BookQuery::default()).unwrap().remove(0);
        let outside = dir.path().join("found.md");
        fs::rename(lib.layout().books_dir().join("Physics/a.md"), &outside).unwrap();
        lib.scan(&NoProgress).unwrap();
        assert!(lib.book(&a.id).unwrap().missing);

        // The same file comes back to its old place.
        assert_eq!(
            lib.locate_file(&a.id, &outside, ImportMode::Copy, false)
                .unwrap(),
            LocateOutcome::Linked
        );
        let back = lib.book(&a.id).unwrap();
        assert!(!back.missing);
        assert_eq!(back.rel_path, "Books/Physics/a.md");

        // Another edition is only used when accepted.
        fs::remove_file(lib.layout().books_dir().join("Physics/a.md")).unwrap();
        lib.scan(&NoProgress).unwrap();
        let other = dir.path().join("alpha-2e.md");
        fs::write(&other, "---\ntitle: Alpha\n---\nSecond edition\n").unwrap();
        assert_eq!(
            lib.locate_file(&a.id, &other, ImportMode::Move, false)
                .unwrap(),
            LocateOutcome::DifferentFile
        );
        assert!(other.exists(), "nothing moved yet");
        lib.locate_file(&a.id, &other, ImportMode::Move, true)
            .unwrap();
        let new_id = lib
            .with_db(|db| db.resolve_book_id(&a.id))
            .unwrap()
            .unwrap();
        assert_ne!(new_id, a.id);
        let b = lib.book(&new_id).unwrap();
        assert!(!b.missing);
        assert_eq!(b.metadata.title, "Alpha");
        assert_eq!(b.rel_path, "Books/Physics/alpha-2e.md");
    }

    #[test]
    fn backups_rotate() {
        let (dir, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "Alpha");
        lib.scan(&NoProgress).unwrap();
        let folder = dir.path().join("Backups");
        fs::create_dir_all(&folder).unwrap();
        fs::write(folder.join("keep me.libreri"), "not a backup").unwrap();
        for _ in 0..4 {
            lib.back_up_into(&folder, 2, false, V, &NoProgress).unwrap();
        }
        let list = list_backups(&folder, &lib.info().id.to_string());
        assert_eq!(list.len(), 2);
        assert!(folder.join("keep me.libreri").exists());
        assert_eq!(list[0].books, 1);
    }
}
