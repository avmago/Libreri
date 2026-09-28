//! Bringing the database in line with the files under `Books/`.
//!
//! Runs when a library opens, when the watcher sees changes, and on request
//! ("Rebuild library index"). Unchanged files (same size and modification
//! time) are not re-read, so a scan of a large library is quick.

use crate::paths::{self, is_hidden, mtime_secs};
use crate::{covers, now, sidecar, Error, Library, Progress, Result};
use libreri_core::{BookId, FileType};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ScanReport {
    /// New files found and added.
    pub added: u32,
    /// Known books found at a new path (moved or renamed outside Libreri).
    pub moved: u32,
    /// Books whose file content changed.
    pub changed: u32,
    /// Books whose file can no longer be found.
    pub missing: u32,
    /// Books that were missing and are back.
    pub restored: u32,
    /// Files with the same content as another book (left alone).
    pub duplicates: u32,
    pub failed: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

impl ScanReport {
    pub fn changed_anything(&self) -> bool {
        self.added + self.moved + self.changed + self.missing + self.restored > 0
    }
}

struct Found {
    abs: PathBuf,
    rel: String,
    file_type: FileType,
    size: u64,
    mtime: i64,
}

impl Library {
    /// Scans `Books/` and updates the database.
    pub fn scan(&self, progress: &dyn Progress) -> Result<ScanReport> {
        let _busy = self.busy();
        let mut report = ScanReport::default();

        // 1. What is on disk.
        let mut found = Vec::new();
        let walker = walkdir::WalkDir::new(self.layout().books_dir())
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| e.depth() == 0 || !is_hidden(e.file_name()));
        for entry in walker.flatten() {
            if !entry.file_type().is_file() {
                continue;
            }
            let Some(file_type) = FileType::from_path(entry.path()) else {
                continue;
            };
            let (Some(rel), Ok(meta)) =
                (paths::rel_of(self.layout(), entry.path()), entry.metadata())
            else {
                continue;
            };
            found.push(Found {
                abs: entry.path().to_path_buf(),
                rel,
                file_type,
                size: meta.len(),
                mtime: mtime_secs(&meta),
            });
        }

        // 2. What the database knows.
        let records = self.with_db(|db| db.file_records())?;
        let by_path: HashMap<&str, _> = records.iter().map(|r| (r.rel_path.as_str(), r)).collect();
        let mut seen_ids: HashSet<BookId> = HashSet::new();
        let mut unmatched: Vec<&Found> = Vec::new();

        // 3. Files at a known path: unchanged, or edited.
        for f in &found {
            let Some(rec) = by_path.get(f.rel.as_str()) else {
                unmatched.push(f);
                continue;
            };
            if rec.file_size == f.size && rec.file_mtime == f.mtime {
                if rec.missing {
                    self.with_db(|db| db.set_missing(&rec.id, false))?;
                    report.restored += 1;
                }
                seen_ids.insert(rec.id.clone());
                continue;
            }
            let id = match paths::hash_file(&f.abs) {
                Ok(id) => id,
                Err(e) => {
                    report.failed.push((f.rel.clone(), e.to_string()));
                    seen_ids.insert(rec.id.clone());
                    continue;
                }
            };
            if id == rec.id {
                // Touched but not changed.
                self.with_db(|db| db.set_file_stamp(&rec.id, f.size, f.mtime))?;
            } else if self.with_db(|db| db.resolve_book_id(&id))?.is_none() {
                self.with_db(|db| db.change_book_id(&rec.id, &id, f.size, f.mtime, &now()))?;
                sidecar::rename(self.layout(), &rec.id, &id);
                covers::rename(self.layout(), &rec.id, &id);
                crate::text::rename(self.layout(), &rec.id, &id);
                crate::listening::rename(self.layout(), &rec.id, &id);
                self.rename_annotation_backups(&rec.id, &id);
                if let Ok(book) = self.record(&id) {
                    sidecar::write(self, &book)?;
                }
                report.changed += 1;
                seen_ids.insert(id);
                continue;
            } else {
                // Now identical to another book: treat as that book's copy.
                report.duplicates += 1;
            }
            seen_ids.insert(rec.id.clone());
        }

        // 4. Files at unknown paths: moved books, restored books, or new.
        let total = unmatched.len() as u64;
        for (i, f) in unmatched.into_iter().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            progress.report(i as u64, total, &f.rel);
            let id = match paths::hash_file(&f.abs) {
                Ok(id) => id,
                Err(e) => {
                    report.failed.push((f.rel.clone(), e.to_string()));
                    continue;
                }
            };
            match self.with_db(|db| db.resolve_book_id(&id))? {
                Some(existing) if !seen_ids.contains(&existing) => {
                    let was_missing = records
                        .iter()
                        .find(|r| r.id == existing)
                        .is_some_and(|r| r.missing);
                    self.with_db(|db| {
                        db.set_book_path(&existing, &f.rel)?;
                        db.set_file_stamp(&existing, f.size, f.mtime)
                    })?;
                    if let Ok(book) = self.record(&existing) {
                        sidecar::write(self, &book)?;
                    }
                    if was_missing {
                        report.restored += 1;
                    } else {
                        report.moved += 1;
                    }
                    seen_ids.insert(existing);
                }
                Some(_) => report.duplicates += 1,
                None => {
                    match self.register(&f.abs, id.clone(), f.file_type, &mut report.warnings) {
                        Ok(_) => {
                            report.added += 1;
                            seen_ids.insert(id);
                        }
                        Err(e) => report.failed.push((f.rel.clone(), e.to_string())),
                    }
                }
            }
        }

        // 5. Books whose file is gone.
        for rec in &records {
            if !seen_ids.contains(&rec.id) && !rec.missing {
                let still_there = self
                    .layout()
                    .resolve_relative(&rec.rel_path)
                    .is_some_and(|p| fs::metadata(p).is_ok());
                if !still_there {
                    self.with_db(|db| db.set_missing(&rec.id, true))?;
                    report.missing += 1;
                }
            }
        }
        progress.report(total, total, "");
        Ok(report)
    }

    /// Rebuilds the database from the book files and JSON sidecars. The old
    /// database is kept next to it as `library.db.bak`.
    pub fn rebuild_index(&self, progress: &dyn Progress) -> Result<ScanReport> {
        self.require_edit()?;
        {
            let _busy = self.busy();
            let mut guard = self.db.lock().unwrap_or_else(|p| p.into_inner());
            if let Some(db) = guard.take() {
                db.close()?;
            }
            let path = self.layout().database_path();
            let backup = path.with_extension("db.bak");
            let _ = fs::remove_file(&backup);
            fs::rename(&path, &backup)?;
            for ext in ["db-wal", "db-shm"] {
                let _ = fs::remove_file(path.with_extension(ext));
            }
            let db = libreri_db::Database::open(&path)?;
            db.meta_set("library_id", &self.info().id.to_string())?;
            db.meta_set("name", &self.info().name)?;
            // Bring back profiles (and their PINs) from their backups; keep
            // whoever is signed in even if their backup is missing.
            self.restore_profiles(&db)?;
            if let Some(s) = self.session_info() {
                db.insert_profile(&s.id, "Owner", &now())?;
            }
            db.ensure_owner_profile("Owner", &now())?;
            *guard = Some(db);
        }
        self.scan(progress)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::BookQuery;
    use std::fs;

    fn titles(lib: &Library) -> Vec<String> {
        lib.books(&BookQuery::default())
            .unwrap()
            .into_iter()
            .map(|b| b.metadata.title)
            .collect()
    }

    #[test]
    fn picks_up_changes_made_outside_libreri() {
        let (_dir, lib) = library();
        let books = lib.layout().books_dir();
        md_book(&books, "a.md", "Alpha");
        md_book(&books, "Sub/b.md", "Beta");
        fs::write(books.join("ignored.docx"), "x").unwrap();

        let r = lib.scan(&NoProgress).unwrap();
        assert_eq!(r.added, 2);
        assert_eq!(titles(&lib), vec!["Alpha", "Beta"]);

        // Nothing changed: nothing to do.
        assert!(!lib.scan(&NoProgress).unwrap().changed_anything());

        // Rename in the file manager: same book, new path, details kept.
        let beta = lib.books(&BookQuery::default()).unwrap()[1].clone();
        let mut m = beta.metadata.clone();
        m.tags = vec!["kept".into()];
        lib.update_metadata(&beta.id, m).unwrap();
        fs::create_dir(books.join("Other")).unwrap();
        fs::rename(books.join("Sub/b.md"), books.join("Other/beta.md")).unwrap();
        let r = lib.scan(&NoProgress).unwrap();
        assert_eq!(r.moved, 1);
        let b = lib.book(&beta.id).unwrap();
        assert_eq!(b.rel_path, "Books/Other/beta.md");
        assert_eq!(b.metadata.tags, vec!["kept"]);

        // Deleted: marked missing, and restored when it comes back.
        let content = fs::read(books.join("a.md")).unwrap();
        fs::remove_file(books.join("a.md")).unwrap();
        assert_eq!(lib.scan(&NoProgress).unwrap().missing, 1);
        fs::write(books.join("a.md"), content).unwrap();
        assert_eq!(lib.scan(&NoProgress).unwrap().restored, 1);

        // Edited: new id, details and old links kept.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(books.join("Other/beta.md"), "---\ntitle: Changed\n---\n").unwrap();
        let r = lib.scan(&NoProgress).unwrap();
        assert_eq!(r.changed, 1);
        let b = lib.book(&beta.id).unwrap();
        assert_ne!(b.id, beta.id);
        assert_eq!(b.metadata.tags, vec!["kept"]);
    }

    #[test]
    fn rebuild_restores_hand_edited_details_from_sidecars() {
        let (_dir, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "Alpha");
        lib.scan(&NoProgress).unwrap();
        let a = lib.books(&BookQuery::default()).unwrap()[0].clone();
        let mut m = a.metadata.clone();
        m.title = "Alpha, edited by hand".into();
        m.authors = vec!["Jane Smith".into()];
        lib.update_metadata(&a.id, m).unwrap();

        let r = lib.rebuild_index(&NoProgress).unwrap();
        assert_eq!(r.added, 1);
        let b = lib.book(&a.id).unwrap();
        assert_eq!(b.metadata.title, "Alpha, edited by hand");
        assert_eq!(b.metadata.authors, vec!["Jane Smith"]);
        assert_eq!(b.added_at, a.added_at);
        assert!(lib
            .layout()
            .database_path()
            .with_extension("db.bak")
            .is_file());
    }
}
