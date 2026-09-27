//! Exporting the library: collects books and the signed-in reader's notes
//! and hands them to `libreri-export` for the chosen format.
//!
//! Personal data in an export is always the signed-in profile's own. Only
//! the owner can put everyone's data in a Libreri archive or database copy
//! (the owner can already reset anyone's PIN). Kids and guests cannot export.

use crate::archive::ArchiveOptions;
use crate::paths::write_atomic;
use crate::{now, Error, Library, Progress, Result};
use libreri_core::{Book, BookId, BookQuery, ProfileId, ProfileKind};
use libreri_export::archive::ArchiveKind;
use libreri_export::citation::{self, Citation};
use libreri_export::table::{self, TableOptions};
use libreri_export::{notes, opf, safe_file_name, unique_name, CitationStyle, Entry, ExportFormat};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// What to export and where.
#[derive(Debug, Clone)]
pub struct ExportRequest {
    pub format: ExportFormat,
    /// The books to export; `None` means every book the profile can see.
    pub books: Option<Vec<BookId>>,
    /// The file to write, or for folder formats (Obsidian, Calibre) a new
    /// or empty folder.
    pub dest: PathBuf,
    /// Status, rating, favourite and progress (tables, JSON, Obsidian,
    /// Calibre ratings).
    pub personal: bool,
    /// Highlights, bookmarks and notebooks (JSON, Obsidian, archive).
    pub notes: bool,
    /// Book files (archive, Calibre).
    pub book_files: bool,
    /// Everyone's personal data (archive, database copy). Owner only.
    pub everyone: bool,
    /// Version of the app, recorded in archives.
    pub app_version: String,
}

/// What an export wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ExportReport {
    pub path: PathBuf,
    pub books: u32,
    pub notes: u32,
    pub files: u32,
    pub bytes: u64,
    pub warnings: Vec<String>,
}

/// A Markdown file in a profile's notes folder and the book it is about.
pub(crate) struct NoteFile {
    pub abs: PathBuf,
    /// Path inside the profile's notes folder, with `/` separators.
    pub inner: String,
    pub book: Option<BookId>,
}

fn folder_size(dir: &Path) -> u64 {
    walkdir::WalkDir::new(dir)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// A folder export writes into a new folder, or an empty one.
fn prepare_folder(dest: &Path) -> Result<()> {
    if dest.exists() {
        let empty = fs::read_dir(dest)?.next().is_none();
        if !dest.is_dir() || !empty {
            return Err(Error::InvalidInput(format!(
                "choose a new or empty folder; {} already has files in it",
                dest.display()
            )));
        }
    }
    fs::create_dir_all(dest)?;
    Ok(())
}

impl Library {
    /// Refuses unless the signed-in profile may export.
    pub fn require_export(&self) -> Result<()> {
        self.export_session().map(|_| ())
    }

    fn export_session(&self) -> Result<crate::Session> {
        let s = self.session_info().ok_or(Error::SignedOut)?;
        if matches!(s.kind, ProfileKind::Kids | ProfileKind::Guest) {
            return Err(Error::NotAllowed(
                "this profile can copy citations but not export; ask the owner".into(),
            ));
        }
        Ok(s)
    }

    /// Every file in a profile's notes folder (`Notes/<name>/`), with the
    /// book each Markdown note is linked to.
    pub(crate) fn note_files(&self, profile: &ProfileId) -> Result<(PathBuf, Vec<NoteFile>)> {
        let name = self
            .with_db(|db| db.profile_name(profile))?
            .unwrap_or_else(|| "Me".into());
        let dir = self
            .layout()
            .notes_dir()
            .join(crate::reading::notes_folder_name(&name));
        let mut out = Vec::new();
        for e in walkdir::WalkDir::new(&dir)
            .follow_links(false)
            .into_iter()
            .flatten()
        {
            if !e.file_type().is_file() || crate::paths::is_hidden(e.file_name()) {
                continue;
            }
            let Ok(inner) = e.path().strip_prefix(&dir) else {
                continue;
            };
            let inner = inner
                .components()
                .map(|c| c.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            let book = if e.path().extension().is_some_and(|x| x == "md") {
                fs::read_to_string(e.path())
                    .ok()
                    .and_then(|c| crate::reading::linked_book(&c))
                    .and_then(|id| self.with_db(|db| db.resolve_book_id(&id)).ok().flatten())
            } else {
                None
            };
            out.push(NoteFile {
                abs: e.path().to_path_buf(),
                inner,
                book,
            });
        }
        Ok((dir, out))
    }

    /// The books an export covers, in title order, limited to what the
    /// signed-in profile can see.
    fn export_books(&self, ids: Option<&[BookId]>) -> Result<Vec<Book>> {
        let all = self.books(&BookQuery::default())?;
        Ok(match ids {
            None => all,
            Some(ids) => {
                let wanted: HashSet<&BookId> = ids.iter().collect();
                all.into_iter().filter(|b| wanted.contains(&b.id)).collect()
            }
        })
    }

    /// Books with the signed-in profile's notes and notebooks.
    fn entries(&self, books: Vec<Book>, profile: &ProfileId, notes: bool) -> Result<Vec<Entry>> {
        let notebooks: HashMap<BookId, NoteFile> = if notes {
            self.note_files(profile)?
                .1
                .into_iter()
                .filter(|n| n.inner.ends_with(".md"))
                .filter_map(|n| n.book.clone().map(|b| (b, n)))
                .collect()
        } else {
            HashMap::new()
        };
        let mut out = Vec::with_capacity(books.len());
        for book in books {
            let annotations = self.with_db(|db| db.annotations(&book.id, profile))?;
            let notebook = notebooks.get(&book.id).and_then(|n| {
                let content = fs::read_to_string(&n.abs).ok()?;
                let rel = crate::paths::rel_of(self.layout(), &n.abs)?;
                Some((rel, content))
            });
            out.push(Entry {
                book,
                annotations,
                notebook,
            });
        }
        Ok(out)
    }

    /// Writes an export. Long exports report progress and can be cancelled.
    pub fn export(&self, req: &ExportRequest, progress: &dyn Progress) -> Result<ExportReport> {
        let session = self.export_session()?;
        if req.everyone && session.kind != ProfileKind::Owner {
            return Err(Error::NotAllowed(
                "only the owner can export everyone's notes".into(),
            ));
        }
        match req.format {
            ExportFormat::Archive => {
                let books = self
                    .export_books(req.books.as_deref())?
                    .into_iter()
                    .map(|b| b.id)
                    .collect::<Vec<_>>();
                let whole = req.books.is_none();
                let opts = ArchiveOptions {
                    books: if whole { None } else { Some(books) },
                    profiles: if req.everyone {
                        None
                    } else {
                        Some(vec![session.id])
                    },
                    notes: req.notes,
                    book_files: req.book_files,
                    kind: ArchiveKind::Export,
                    app_version: req.app_version.clone(),
                };
                return self.write_archive(&req.dest, &opts, progress);
            }
            ExportFormat::Sqlite => {
                if req.dest.exists() {
                    fs::remove_file(&req.dest)?;
                }
                let scope = libreri_db::SnapshotScope {
                    only_profile: (!req.everyone).then_some(session.id),
                    strip_pins: true,
                };
                self.with_db(|db| db.snapshot(&req.dest, &scope))?;
                return Ok(ExportReport {
                    path: req.dest.clone(),
                    books: self.with_db(|db| db.book_count())? as u32,
                    bytes: fs::metadata(&req.dest)?.len(),
                    ..Default::default()
                });
            }
            _ => {}
        }

        let books = self.export_books(req.books.as_deref())?;
        let with_notes = req.notes
            || (req.personal && matches!(req.format, ExportFormat::Csv | ExportFormat::Xlsx));
        let entries = self.entries(books, &session.id, with_notes)?;
        let opts = TableOptions {
            personal: req.personal,
            notes: req.notes,
        };
        let mut report = ExportReport {
            path: req.dest.clone(),
            books: entries.len() as u32,
            notes: if req.notes {
                entries.iter().map(|e| e.annotations.len() as u32).sum()
            } else {
                0
            },
            ..Default::default()
        };
        let plain: Vec<Book> = entries.iter().map(|e| e.book.clone()).collect();
        let profile_name = self.with_db(|db| db.profile_name(&session.id))?;
        let bytes: Vec<u8> = match req.format {
            ExportFormat::Csv => table::csv(&entries, opts).map_err(Error::InvalidInput)?,
            ExportFormat::Xlsx => table::xlsx(&entries, opts).map_err(Error::InvalidInput)?,
            ExportFormat::Json => table::json(
                &entries,
                opts,
                &self.info().name,
                profile_name
                    .as_deref()
                    .filter(|_| req.personal || req.notes),
                &now(),
            )
            .into_bytes(),
            ExportFormat::Bibtex => citation::bibtex(&plain).into_bytes(),
            ExportFormat::Ris => citation::ris(&plain).into_bytes(),
            ExportFormat::CslJson => citation::csl_json(&plain).into_bytes(),
            ExportFormat::Obsidian => {
                self.write_obsidian(req, &entries, &session.id, &mut report, progress)?;
                report.bytes = folder_size(&req.dest);
                return Ok(report);
            }
            ExportFormat::Calibre => {
                self.write_calibre(req, &entries, &mut report, progress)?;
                report.bytes = folder_size(&req.dest);
                return Ok(report);
            }
            ExportFormat::Archive | ExportFormat::Sqlite => unreachable!("handled above"),
        };
        if let Some(dir) = req.dest.parent() {
            fs::create_dir_all(dir)?;
        }
        write_atomic(&req.dest, &bytes)?;
        report.bytes = bytes.len() as u64;
        Ok(report)
    }

    fn write_obsidian(
        &self,
        req: &ExportRequest,
        entries: &[Entry],
        profile: &ProfileId,
        report: &mut ExportReport,
        progress: &dyn Progress,
    ) -> Result<()> {
        prepare_folder(&req.dest)?;
        let books_dir = req.dest.join("Books");
        fs::create_dir_all(&books_dir)?;

        // Notes: everything in the notes folder for a whole-library export,
        // otherwise only the notebooks of the chosen books.
        let mut notebook_names: HashMap<BookId, String> = HashMap::new();
        if req.notes {
            let (_, files) = self.note_files(profile)?;
            let chosen: HashSet<&BookId> = entries.iter().map(|e| &e.book.id).collect();
            for f in files {
                let keep =
                    req.books.is_none() || f.book.as_ref().is_some_and(|b| chosen.contains(b));
                if !keep {
                    continue;
                }
                let Some(to) = libreri_core::LibraryLayout::new(req.dest.join("Notes"))
                    .resolve_relative(&f.inner)
                else {
                    continue;
                };
                if let Some(dir) = to.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::copy(&f.abs, &to)?;
                report.files += 1;
                if let Some(b) = f.book {
                    if let Some(stem) = f.inner.strip_suffix(".md") {
                        notebook_names
                            .entry(b)
                            .or_insert_with(|| format!("Notes/{stem}"));
                    }
                }
            }
        }

        let mut taken = HashSet::new();
        let mut index = Vec::new();
        let total = entries.len() as u64;
        for (i, e) in entries.iter().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            progress.report(i as u64, total, &e.book.metadata.title);
            let stem = safe_file_name(&libreri_export::full_title(&e.book));
            let name = unique_name(&mut taken, &stem, "md");
            let mut entry = e.clone();
            if !req.notes {
                entry.annotations.clear();
            }
            if !req.personal {
                entry.book.user = Default::default();
            }
            let text = notes::book_note(&entry, notebook_names.get(&e.book.id).map(String::as_str));
            write_atomic(&books_dir.join(&name), text.as_bytes())?;
            index.push((
                libreri_export::full_title(&e.book),
                format!("Books/{}", name.trim_end_matches(".md")),
            ));
        }
        let index_name = format!("{}.md", safe_file_name(&self.info().name));
        write_atomic(
            &req.dest.join(index_name),
            notes::index_note(&self.info().name, &index).as_bytes(),
        )?;
        Ok(())
    }

    fn write_calibre(
        &self,
        req: &ExportRequest,
        entries: &[Entry],
        report: &mut ExportReport,
        progress: &dyn Progress,
    ) -> Result<()> {
        prepare_folder(&req.dest)?;
        let mut authors_taken = HashSet::new();
        let mut author_dirs: HashMap<String, (String, HashSet<String>)> = HashMap::new();
        let total = entries.len() as u64;
        for (i, e) in entries.iter().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            let b = &e.book;
            progress.report(i as u64, total, &b.metadata.title);
            let author = b
                .metadata
                .authors
                .first()
                .map(|a| safe_file_name(a))
                .unwrap_or_else(|| "Unknown".into());
            let (author_dir, titles) = author_dirs
                .entry(author.to_lowercase())
                .or_insert_with(|| (unique_name(&mut authors_taken, &author, ""), HashSet::new()));
            let title = safe_file_name(&b.metadata.title);
            let book_dir_name = unique_name(titles, &title, "");
            let dir = req.dest.join(&*author_dir).join(&book_dir_name);
            fs::create_dir_all(&dir)?;

            let cover_src = crate::covers::cover_path(self.layout(), &b.id);
            let cover = if b.has_cover && cover_src.is_file() {
                fs::copy(&cover_src, dir.join("cover.jpg"))?;
                Some("cover.jpg")
            } else {
                None
            };
            write_atomic(
                &dir.join("metadata.opf"),
                opf::opf(b, req.personal, cover).as_bytes(),
            )?;
            if req.book_files {
                match self.layout().resolve_relative(&b.rel_path) {
                    Some(src) if !b.missing && src.is_file() => {
                        let ext = b.file_type.as_str();
                        let name = format!(
                            "{} - {}.{ext}",
                            title.chars().take(60).collect::<String>().trim(),
                            author.chars().take(40).collect::<String>().trim()
                        );
                        fs::copy(&src, dir.join(safe_file_name(&name)))?;
                        report.files += 1;
                    }
                    _ => report
                        .warnings
                        .push(format!("{}: the file is missing", b.metadata.title)),
                }
            }
        }
        Ok(())
    }

    /// Writes the whole catalogue (details only, never personal data) as
    /// BibTeX, RIS, CSL-JSON, CSV or JSON, for "keep a file up to date".
    /// Needs no one signed in. Returns `true` if the file changed.
    pub fn write_catalogue(&self, format: ExportFormat, dest: &Path) -> Result<bool> {
        let nobody = ProfileId(uuid::Uuid::nil());
        let books = self.with_db(|db| db.query_books(&BookQuery::default(), &nobody))?;
        let entries: Vec<Entry> = books.iter().cloned().map(Entry::new).collect();
        let bytes = match format {
            ExportFormat::Bibtex => citation::bibtex(&books).into_bytes(),
            ExportFormat::Ris => citation::ris(&books).into_bytes(),
            ExportFormat::CslJson => citation::csl_json(&books).into_bytes(),
            ExportFormat::Csv => {
                table::csv(&entries, TableOptions::default()).map_err(Error::InvalidInput)?
            }
            ExportFormat::Json => {
                // No time stamp, so an unchanged library leaves the file alone.
                table::json(
                    &entries,
                    TableOptions::default(),
                    &self.info().name,
                    None,
                    "",
                )
                .into_bytes()
            }
            _ => {
                return Err(Error::InvalidInput(
                    "only BibTeX, RIS, CSL-JSON, CSV and JSON files can be kept up to date".into(),
                ))
            }
        };
        if fs::read(dest).is_ok_and(|old| old == bytes) {
            return Ok(false);
        }
        if let Some(dir) = dest.parent() {
            fs::create_dir_all(dir)?;
        }
        write_atomic(dest, &bytes)?;
        Ok(true)
    }

    /// Formatted references for books, sorted as the style asks. Anyone
    /// signed in can copy citations of books they can see.
    pub fn citations(&self, ids: &[BookId], style: CitationStyle) -> Result<Citation> {
        self.profile()?;
        let mut books = Vec::with_capacity(ids.len());
        for id in ids {
            books.push(self.book(id)?);
        }
        Ok(citation::format_list(&books, style))
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::{Annotation, AnnotationKind, BookQuery, TextQuote};
    use libreri_export::{CitationStyle, ExportFormat};

    fn setup() -> (tempfile::TempDir, Library, libreri_core::Book) {
        let (dir, lib) = library();
        md_book(&lib.layout().books_dir(), "Physics/optics.md", "Optics");
        md_book(&lib.layout().books_dir(), "waves.md", "Waves");
        lib.scan(&NoProgress).unwrap();
        let b = lib
            .books(&BookQuery::default())
            .unwrap()
            .into_iter()
            .find(|b| b.metadata.title == "Optics")
            .unwrap();
        let mut m = b.metadata.clone();
        m.authors = vec!["Jane Smith".into()];
        m.year = Some(2019);
        m.tags = vec!["Light".into()];
        lib.update_metadata(&b.id, m).unwrap();
        lib.save_annotation(Annotation {
            id: "0b7f5a3e-1f7e-4d4c-9d34-5d0a8d8f2c10".into(),
            book_id: b.id.clone(),
            kind: AnnotationKind::Highlight,
            color: None,
            locator: r#"{"type":"text","start":1,"end":5}"#.into(),
            quote: Some(TextQuote {
                exact: "Body".into(),
                ..Default::default()
            }),
            note: Some("Key idea".into()),
            label: Some("p. 1".into()),
            position: 0.1,
            created_at: String::new(),
            modified_at: String::new(),
        })
        .unwrap();
        let nb = lib.notebook(&b.id).unwrap();
        lib.save_notebook(&b.id, &format!("{}My thoughts\n", nb.content))
            .unwrap();
        let b = lib.book(&b.id).unwrap();
        (dir, lib, b)
    }

    fn req(format: ExportFormat, dest: std::path::PathBuf) -> ExportRequest {
        ExportRequest {
            format,
            books: None,
            dest,
            personal: true,
            notes: true,
            book_files: true,
            everyone: false,
            app_version: V.into(),
        }
    }

    #[test]
    fn exports_every_simple_format() {
        let (dir, lib, b) = setup();
        for (format, name, needle) in [
            (ExportFormat::Csv, "books.csv", "Optics"),
            (ExportFormat::Json, "books.json", "Key idea"),
            (ExportFormat::Bibtex, "books.bib", "smith2019optics"),
            (ExportFormat::Ris, "books.ris", "AU  - Smith, Jane"),
            (
                ExportFormat::CslJson,
                "books.csl.json",
                "\"family\": \"Smith\"",
            ),
        ] {
            let dest = dir.path().join(name);
            let r = lib.export(&req(format, dest.clone()), &NoProgress).unwrap();
            assert_eq!(r.books, 2, "{format:?}");
            let text = std::fs::read_to_string(&dest).unwrap();
            assert!(text.contains(needle), "{format:?}: {text}");
        }
        let dest = dir.path().join("books.xlsx");
        lib.export(&req(ExportFormat::Xlsx, dest.clone()), &NoProgress)
            .unwrap();
        assert!(std::fs::read(&dest).unwrap().starts_with(b"PK"));

        let dest = dir.path().join("copy.db");
        lib.export(&req(ExportFormat::Sqlite, dest.clone()), &NoProgress)
            .unwrap();
        assert!(dest.is_file());

        let kept = dir.path().join("library.bib");
        assert!(lib.write_catalogue(ExportFormat::Bibtex, &kept).unwrap());
        assert!(!lib.write_catalogue(ExportFormat::Bibtex, &kept).unwrap());
        lib.sign_out().unwrap();
        assert!(
            !lib.write_catalogue(ExportFormat::Bibtex, &kept).unwrap(),
            "no one signed in"
        );
        assert!(lib.write_catalogue(ExportFormat::Obsidian, &kept).is_err());
        let owner = lib.profiles().unwrap().remove(0);
        lib.sign_in(&owner.id, None).unwrap();

        let c = lib
            .citations(std::slice::from_ref(&b.id), CitationStyle::Apa)
            .unwrap();
        assert_eq!(c.text, "Smith, J. (2019). Optics.");
    }

    #[test]
    fn obsidian_and_calibre_write_folders() {
        let (dir, lib, b) = setup();
        let vault = dir.path().join("vault");
        let r = lib
            .export(&req(ExportFormat::Obsidian, vault.clone()), &NoProgress)
            .unwrap();
        assert_eq!(r.books, 2);
        let note = std::fs::read_to_string(vault.join("Books/Optics.md")).unwrap();
        assert!(note.contains(&format!("libreri://book/{}#annotation=", b.id)));
        assert!(note.contains("Notebook: [[Notes/Optics]]"), "{note}");
        let nb = std::fs::read_to_string(vault.join("Notes/Optics.md")).unwrap();
        assert!(nb.contains("My thoughts"));
        assert!(vault.join("lib.md").is_file());

        // A folder with files in it is refused.
        assert!(lib
            .export(&req(ExportFormat::Obsidian, vault), &NoProgress)
            .is_err());

        let cal = dir.path().join("calibre");
        let r = lib
            .export(&req(ExportFormat::Calibre, cal.clone()), &NoProgress)
            .unwrap();
        assert_eq!(r.files, 2);
        let opf = std::fs::read_to_string(cal.join("Jane Smith/Optics/metadata.opf")).unwrap();
        assert!(opf.contains("<dc:title>Optics</dc:title>"));
        assert!(cal
            .join("Jane Smith/Optics/Optics - Jane Smith.md")
            .is_file());
    }

    #[test]
    fn kids_cannot_export_and_standard_cannot_export_everyone() {
        let (dir, lib, _) = setup();
        let sam = lib
            .create_profile("Sam", "blue", libreri_core::ProfileKind::Standard, None)
            .unwrap();
        lib.sign_in(&sam.id, None).unwrap();
        let mut r = req(ExportFormat::Archive, dir.path().join("a.libreri"));
        r.everyone = true;
        assert!(matches!(
            lib.export(&r, &NoProgress),
            Err(Error::NotAllowed(_))
        ));
        let kid = {
            lib.sign_out().unwrap();
            let owner = lib.profiles().unwrap().remove(0);
            lib.sign_in(&owner.id, None).unwrap();
            lib.create_profile("Kid", "red", libreri_core::ProfileKind::Kids, None)
                .unwrap()
        };
        lib.sign_in(&kid.id, None).unwrap();
        assert!(matches!(
            lib.export(
                &req(ExportFormat::Csv, dir.path().join("k.csv")),
                &NoProgress
            ),
            Err(Error::NotAllowed(_))
        ));
        // …but can still copy citations.
        assert!(lib.citations(&[], CitationStyle::Mla).is_ok());
    }
}
