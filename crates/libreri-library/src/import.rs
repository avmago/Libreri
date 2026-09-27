//! Importing files and folders into `Books/`.

use crate::paths::{self, folder_abs, is_hidden, unique_path};
use crate::{sidecar, Error, Library, Progress, Result};
use libreri_core::{BookId, FileType};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ImportMode {
    /// Move the files into the library (the default: no duplicates on disk).
    #[default]
    Move,
    /// Leave the originals where they are.
    Copy,
}

#[derive(Debug, Clone)]
pub struct ImportRequest {
    /// Files and folders to import. Folders keep their structure.
    pub sources: Vec<PathBuf>,
    /// Destination relative to `Books/` ("" = top level).
    pub folder: String,
    pub mode: ImportMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Duplicate {
    pub file: String,
    pub existing_title: String,
    pub existing_id: BookId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImportReport {
    pub added_ids: Vec<BookId>,
    /// Files whose book was already in the library (same content).
    pub duplicates: Vec<Duplicate>,
    /// Files that matched a book whose file had gone missing.
    pub relinked: u32,
    /// Files skipped because Libreri does not read their format.
    pub unsupported: u32,
    /// `(file name, reason)`
    pub failed: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

/// One file to import and the folder (relative to `Books/`) it goes to.
struct Item {
    source: PathBuf,
    file_type: FileType,
    folder: String,
    mode: ImportMode,
}

fn display_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.display().to_string())
}

fn join_folder(base: &str, sub: &str) -> String {
    match (base.is_empty(), sub.is_empty()) {
        (true, _) => sub.to_owned(),
        (_, true) => base.to_owned(),
        _ => format!("{base}/{sub}"),
    }
}

/// Lists every book file under the sources. A dropped folder "Physics"
/// becomes `<folder>/Physics/…`.
fn plan(req: &ImportRequest, report: &mut ImportReport) -> Vec<Item> {
    let mut items = Vec::new();
    for src in &req.sources {
        if src.is_dir() {
            let base_name = display_name(src);
            let walker = walkdir::WalkDir::new(src)
                .follow_links(false)
                .into_iter()
                .filter_entry(|e| e.depth() == 0 || !is_hidden(e.file_name()));
            for entry in walker.flatten() {
                if !entry.file_type().is_file() {
                    continue;
                }
                let Some(ft) = FileType::from_path(entry.path()) else {
                    report.unsupported += 1;
                    continue;
                };
                let sub = entry
                    .path()
                    .parent()
                    .and_then(|p| p.strip_prefix(src).ok())
                    .map(|p| {
                        p.components()
                            .map(|c| c.as_os_str().to_string_lossy().into_owned())
                            .collect::<Vec<_>>()
                            .join("/")
                    })
                    .unwrap_or_default();
                items.push(Item {
                    source: entry.path().to_path_buf(),
                    file_type: ft,
                    folder: join_folder(&req.folder, &join_folder(&base_name, &sub)),
                    mode: req.mode,
                });
            }
        } else if src.is_file() {
            match FileType::from_path(src) {
                Some(ft) => items.push(Item {
                    source: src.clone(),
                    file_type: ft,
                    folder: req.folder.clone(),
                    mode: req.mode,
                }),
                None => report.unsupported += 1,
            }
        } else {
            report
                .failed
                .push((display_name(src), "the file no longer exists".into()));
        }
    }
    items
}

impl Library {
    /// Imports files and folders. Runs one at a time with scans; reports
    /// progress per file and stops between files when cancelled.
    pub fn import(&self, req: &ImportRequest, progress: &dyn Progress) -> Result<ImportReport> {
        self.require_edit()?;
        let _busy = self.busy();
        let mut report = ImportReport::default();
        folder_abs(self.layout(), &req.folder)?;
        let items = plan(req, &mut report);
        let total = items.len() as u64;
        let books_dir = self.layout().books_dir();

        for (i, item) in items.iter().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            let name = display_name(&item.source);
            progress.report(i as u64, total, &name);
            match self.import_one(item, &books_dir, &mut report) {
                Ok(()) => {}
                Err(e) => report.failed.push((name, e.to_string())),
            }
        }
        progress.report(total, total, "");
        Ok(report)
    }

    fn import_one(&self, item: &Item, books_dir: &Path, report: &mut ImportReport) -> Result<()> {
        let id = paths::hash_file(&item.source)?;
        let already_inside = item.source.starts_with(books_dir);

        if let Some(existing) = self.with_db(|db| db.resolve_book_id(&id))? {
            let book = self.record(&existing)?;
            let existing_path = self.layout().resolve_relative(&book.rel_path);
            let present = existing_path.as_ref().is_some_and(|p| p.is_file());
            if present || already_inside {
                report.duplicates.push(Duplicate {
                    file: display_name(&item.source),
                    existing_title: book.metadata.title,
                    existing_id: book.id,
                });
                return Ok(());
            }
            // The book's file had gone missing: this is it, put it back.
            let dest = self.place(item, books_dir)?;
            let rel = paths::rel_of(self.layout(), &dest).ok_or(Error::BookNotFound)?;
            self.with_db(|db| db.set_book_path(&book.id, &rel))?;
            sidecar::write(self, &self.record(&book.id)?)?;
            report.relinked += 1;
            return Ok(());
        }

        let dest = if already_inside {
            item.source.clone()
        } else {
            self.place(item, books_dir)?
        };
        let book = self.register(&dest, id, item.file_type, &mut report.warnings);
        match book {
            Ok(book) => {
                report.added_ids.push(book.id);
                Ok(())
            }
            Err(e) => {
                // Undo the copy so a failed import leaves no stray file.
                if !already_inside && dest != item.source {
                    let _ = match fs::metadata(&item.source) {
                        Ok(_) => fs::remove_file(&dest),
                        Err(_) => paths::move_file(&dest, &item.source),
                    };
                }
                Err(e)
            }
        }
    }

    /// Copies or moves the source into its destination folder, creating the
    /// folder and picking a free name.
    fn place(&self, item: &Item, books_dir: &Path) -> Result<PathBuf> {
        let dir = if item.folder.is_empty() {
            books_dir.to_path_buf()
        } else {
            folder_abs(self.layout(), &item.folder)?
        };
        fs::create_dir_all(&dir)?;
        let dest = unique_path(&dir, &display_name(&item.source));
        match item.mode {
            ImportMode::Copy => {
                fs::copy(&item.source, &dest)?;
            }
            ImportMode::Move => paths::move_file(&item.source, &dest)?,
        }
        Ok(dest)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::BookQuery;
    use std::fs;

    fn req(sources: Vec<std::path::PathBuf>, folder: &str, mode: ImportMode) -> ImportRequest {
        ImportRequest {
            sources,
            folder: folder.into(),
            mode,
        }
    }

    #[test]
    fn imports_files_and_folders_and_skips_duplicates() {
        let (dir, lib) = library();
        let outside = dir.path().join("outside");
        let a = md_book(&outside, "a.md", "Alpha");
        md_book(&outside, "Physics/b.md", "Beta");
        md_book(&outside, "Physics/Quantum/c.md", "Gamma");
        fs::write(outside.join("Physics/notes.docx"), "x").unwrap();
        fs::write(outside.join("Physics/.DS_Store"), "x").unwrap();

        let r = lib
            .import(&req(vec![a.clone()], "", ImportMode::Copy), &NoProgress)
            .unwrap();
        assert_eq!(r.added_ids.len(), 1);
        assert!(a.exists(), "copy keeps the original");

        let r = lib
            .import(
                &req(
                    vec![outside.join("Physics"), a.clone()],
                    "Science",
                    ImportMode::Move,
                ),
                &NoProgress,
            )
            .unwrap();
        assert_eq!(r.added_ids.len(), 2);
        assert_eq!(r.duplicates.len(), 1);
        assert_eq!(r.duplicates[0].existing_title, "Alpha");
        assert_eq!(r.unsupported, 1);
        assert!(
            !outside.join("Physics/b.md").exists(),
            "move removes originals"
        );
        let root = lib.layout().root();
        assert!(root.join("Books/Science/Physics/Quantum/c.md").is_file());

        let titles: Vec<String> = lib
            .books(&BookQuery::default())
            .unwrap()
            .into_iter()
            .map(|b| b.metadata.title)
            .collect();
        assert_eq!(titles, vec!["Alpha", "Beta", "Gamma"]);
    }

    #[test]
    fn name_clashes_get_a_number() {
        let (dir, lib) = library();
        let one = md_book(&dir.path().join("1"), "same.md", "One");
        let two = md_book(&dir.path().join("2"), "same.md", "Two");
        lib.import(&req(vec![one, two], "", ImportMode::Copy), &NoProgress)
            .unwrap();
        let root = lib.layout().root();
        assert!(root.join("Books/same.md").is_file());
        assert!(root.join("Books/same (2).md").is_file());
    }

    #[test]
    fn cancelling_stops_between_files() {
        struct Cancel;
        impl Progress for Cancel {
            fn report(&self, _: u64, _: u64, _: &str) {}
            fn cancelled(&self) -> bool {
                true
            }
        }
        let (dir, lib) = library();
        let a = md_book(dir.path(), "a.md", "A");
        assert!(matches!(
            lib.import(&req(vec![a], "", ImportMode::Copy), &Cancel),
            Err(Error::Cancelled)
        ));
    }
}
