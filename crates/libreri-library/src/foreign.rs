//! Importing from other apps: Calibre, Zotero, BibTeX/RIS files (Mendeley,
//! JabRef…) and Goodreads/StoryGraph exports (docs/adr/0015).
//!
//! Book files are always **copied**; the other app's folders are only read.
//! One file per book: for Calibre the first format in the chosen order, for
//! Zotero the attachment with the most highlights. Details from the other
//! app win over what Libreri reads from the file; tags and categories are
//! added together. Personal data (status, rating, highlights, notes) goes to
//! the signed-in profile. Goodreads and StoryGraph only update books that
//! are already here (user, 2026-09-28).

use crate::paths::{self, folder_abs, unique_path};
use crate::{covers, now, sidecar, Error, Library, Progress, Result};
use libreri_core::{
    Annotation, AnnotationKind, BookId, BookMetadata, ContentType, FileType, ReadingStatus,
    TextQuote,
};
use libreri_export::foreign::{
    self, ForeignAnnotation, ForeignBook, ForeignError, ForeignMark, ForeignPersonal,
    ForeignSource, DEFAULT_FORMAT_ORDER,
};
use libreri_formats::PageBox;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// How to import.
#[derive(Debug, Clone, Default)]
pub struct ForeignImport {
    /// Preferred formats, best first (Calibre); empty = the default order.
    pub formats: Vec<FileType>,
    /// Folder under `Books/` to copy files into ("" = top level).
    pub folder: String,
    /// Replace reading status and rating already set here (otherwise only
    /// empty ones are filled).
    pub replace_personal: bool,
}

/// What a library from another app holds, before importing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForeignSummary {
    pub source: Option<ForeignSource>,
    pub books: u32,
    /// Books with a file Libreri can open.
    pub with_files: u32,
    pub highlights: u32,
    pub notes: u32,
    pub rated: u32,
    /// Files by format (Calibre can have several per book).
    pub formats: Vec<(FileType, u32)>,
    /// Reading logs: books found in this library.
    pub matched: u32,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ForeignReport {
    pub added: u32,
    pub added_ids: Vec<BookId>,
    /// Same file already in the library: details and notes were merged.
    pub already_here: u32,
    /// Books without a file whose details went to a matching book here.
    pub details_added: u32,
    /// Books without a file and no match here (titles, at most 50).
    pub without_file: Vec<String>,
    pub without_file_count: u32,
    pub highlights_added: u32,
    pub notes_added: u32,
    pub personal_updated: u32,
    /// Reading logs: books not in this library (titles, at most 50).
    pub unmatched: Vec<String>,
    pub unmatched_count: u32,
    pub failed: Vec<(String, String)>,
    pub warnings: Vec<String>,
}

fn foreign_err(e: ForeignError) -> Error {
    match e {
        ForeignError::Io(e) => Error::Io(e),
        other => Error::InvalidInput(other.to_string()),
    }
}

/// Details from the other app over details read from the file: non-empty
/// fields replace, tags and categories are added together.
fn overlay(base: &mut BookMetadata, src: &BookMetadata) {
    macro_rules! opt {
        ($($f:ident),*) => {$( if src.$f.is_some() { base.$f = src.$f.clone(); } )*};
    }
    if !src.title.trim().is_empty() {
        base.title = src.title.clone();
    }
    if !src.authors.is_empty() {
        base.authors = src.authors.clone();
    }
    if !src.contributors.is_empty() {
        base.contributors = src.contributors.clone();
    }
    opt!(
        subtitle,
        about,
        year,
        publisher,
        pages,
        isbn13,
        isbn10,
        edition,
        language,
        series,
        series_number,
        doi,
        arxiv_id,
        journal,
        volume,
        issue,
        url
    );
    if !matches!(src.content_type, ContentType::Book | ContentType::Other) {
        base.content_type = src.content_type;
    }
    union(&mut base.tags, &src.tags);
    union(&mut base.categories, &src.categories);
}

/// Only empty fields are filled (a book already in the library).
fn fill_empty(base: &mut BookMetadata, src: &BookMetadata) {
    macro_rules! opt {
        ($($f:ident),*) => {$( if base.$f.is_none() { base.$f = src.$f.clone(); } )*};
    }
    if base.authors.is_empty() {
        base.authors = src.authors.clone();
    }
    opt!(
        subtitle,
        about,
        year,
        publisher,
        pages,
        isbn13,
        isbn10,
        edition,
        language,
        series,
        series_number,
        doi,
        arxiv_id,
        journal,
        volume,
        issue,
        url
    );
    union(&mut base.tags, &src.tags);
    union(&mut base.categories, &src.categories);
}

fn union(a: &mut Vec<String>, b: &[String]) {
    for x in b {
        if !a.iter().any(|y| y.eq_ignore_ascii_case(x)) {
            a.push(x.clone());
        }
    }
}

/// A Zotero rectangle (PDF points, bottom-left origin) as a fraction of
/// the page as it is shown (top-left origin, rotation applied).
fn to_page_fraction(r: &[f64; 4], b: &PageBox) -> Option<[f64; 4]> {
    let w = b.x1 - b.x0;
    let h = b.y1 - b.y0;
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let u0 = (r[0].min(r[2]) - b.x0) / w;
    let u1 = (r[0].max(r[2]) - b.x0) / w;
    let v0 = (b.y1 - r[1].max(r[3])) / h;
    let v1 = (b.y1 - r[1].min(r[3])) / h;
    let rot = |u: f64, v: f64| match b.rotate {
        90 => (1.0 - v, u),
        180 => (1.0 - u, 1.0 - v),
        270 => (v, 1.0 - u),
        _ => (u, v),
    };
    let (a, c) = (rot(u0, v0), rot(u1, v1));
    let x = a.0.min(c.0).clamp(0.0, 1.0);
    let y = a.1.min(c.1).clamp(0.0, 1.0);
    let x2 = a.0.max(c.0).clamp(0.0, 1.0);
    let y2 = a.1.max(c.1).clamp(0.0, 1.0);
    (x2 > x && y2 > y).then_some([x, y, x2 - x, y2 - y])
}

/// A highlight or note from Zotero as a Libreri annotation.
fn convert(a: &ForeignAnnotation, book: &BookId, boxes: &[PageBox]) -> Option<Annotation> {
    let page_box = boxes.get(a.page_index as usize)?;
    let pages = boxes.len().max(1) as f64;
    let rects: Vec<[f64; 4]> = a
        .rects
        .iter()
        .filter_map(|r| to_page_fraction(r, page_box))
        .collect();
    let top = rects.iter().map(|r| r[1]).fold(f64::INFINITY, f64::min);
    let top = if top.is_finite() { top } else { 0.0 };
    let page = a.page_index + 1;
    let label = Some(format!(
        "p. {}",
        a.page_label.clone().unwrap_or_else(|| page.to_string())
    ));
    let (kind, locator, quote, note) = match a.mark {
        ForeignMark::Highlight => {
            let text = a.text.clone()?;
            if rects.is_empty() {
                return None;
            }
            let locator =
                serde_json::json!({ "type": "pdf-highlight", "page": page, "rects": rects });
            (
                AnnotationKind::Highlight,
                locator,
                Some(TextQuote {
                    exact: text,
                    ..Default::default()
                }),
                a.comment.clone(),
            )
        }
        ForeignMark::Note => {
            let locator = serde_json::json!({ "type": "pdf", "page": page, "top": top });
            (
                AnnotationKind::Bookmark,
                locator,
                None,
                a.comment.clone().or_else(|| a.text.clone()),
            )
        }
    };
    Annotation {
        id: a.id.clone(),
        book_id: book.clone(),
        kind,
        color: (kind == AnnotationKind::Highlight).then_some(a.color),
        locator: locator.to_string(),
        quote,
        note,
        label,
        position: ((f64::from(a.page_index) + top) / pages).clamp(0.0, 1.0),
        created_at: a.created_at.clone(),
        modified_at: a.modified_at.clone(),
    }
    .validated()
    .ok()
}

impl Library {
    /// A book here with the same ISBN, DOI or arXiv id, or title and first
    /// author (see `same_work`), that the signed-in profile can see.
    fn find_same_book(&self, m: &BookMetadata) -> Result<Option<BookId>> {
        let by_id = self.with_db(|db| {
            db.find_book_by_identifiers(
                m.isbn13.as_deref(),
                m.isbn10.as_deref(),
                m.doi.as_deref(),
                m.arxiv_id.as_deref(),
            )
        })?;
        let found = match by_id {
            Some(id) => Some(id),
            // Title alone is not enough: the first author must match too.
            None if !m.title.trim().is_empty() => {
                let candidates = self.with_db(|db| db.books_titled(&m.title))?;
                crate::archive::same_work(&m.authors, m.pages, &candidates)
            }
            None => None,
        };
        Ok(found.filter(|id| self.book(id).is_ok()))
    }

    /// What importing from another app would bring in.
    pub fn inspect_foreign(&self, path: &Path) -> Result<ForeignSummary> {
        self.require_edit()?;
        let source = foreign::detect(path).ok_or_else(|| {
            Error::InvalidInput(
                "Libreri does not recognise this. Choose a Calibre library folder, Zotero's data folder, a .bib or .ris file, or a Goodreads or StoryGraph export (.csv)".into(),
            )
        })?;
        let lib = foreign::read(path, source).map_err(foreign_err)?;
        let mut s = ForeignSummary {
            source: Some(source),
            books: lib.books.len() as u32,
            warnings: lib.warnings.iter().take(20).cloned().collect(),
            ..Default::default()
        };
        let mut formats: HashMap<FileType, u32> = HashMap::new();
        for b in &lib.books {
            let existing: Vec<_> = b.files.iter().filter(|f| f.path.is_file()).collect();
            if !existing.is_empty() {
                s.with_files += 1;
            }
            for f in existing {
                *formats.entry(f.file_type).or_default() += 1;
            }
            s.highlights += b.annotations.len() as u32;
            s.notes += b.notes.len() as u32;
            if b.personal.as_ref().is_some_and(|p| p.rating > 0) {
                s.rated += 1;
            }
            if source.is_reading_log() && self.find_same_book(&b.metadata)?.is_some() {
                s.matched += 1;
            }
        }
        let mut f: Vec<_> = formats.into_iter().collect();
        f.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.as_str().cmp(b.0.as_str())));
        s.formats = f;
        Ok(s)
    }

    /// Imports from another app. Files are copied; the source is not changed.
    pub fn import_foreign(
        &self,
        path: &Path,
        opts: &ForeignImport,
        progress: &dyn Progress,
    ) -> Result<ForeignReport> {
        self.require_edit()?;
        let source = foreign::detect(path)
            .ok_or_else(|| Error::InvalidInput("Libreri does not recognise this".into()))?;
        let lib = foreign::read(path, source).map_err(foreign_err)?;
        let dest_dir = folder_abs(self.layout(), &opts.folder)?;
        let order: Vec<FileType> = if opts.formats.is_empty() {
            DEFAULT_FORMAT_ORDER.to_vec()
        } else {
            opts.formats.clone()
        };
        let mut report = ForeignReport {
            warnings: lib.warnings.clone(),
            ..Default::default()
        };
        let _busy = self.busy();
        let total = lib.books.len() as u64;
        for (i, book) in lib.books.iter().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            progress.report(i as u64, total, &book.metadata.title);
            let result = if source.is_reading_log() {
                self.import_log_entry(book, opts, &mut report)
            } else {
                self.import_foreign_book(book, source, &order, &dest_dir, opts, &mut report)
            };
            if let Err(e) = result {
                report
                    .failed
                    .push((book.metadata.title.clone(), e.to_string()));
            }
        }
        progress.report(total, total, "");
        Ok(report)
    }

    fn import_log_entry(
        &self,
        b: &ForeignBook,
        opts: &ForeignImport,
        report: &mut ForeignReport,
    ) -> Result<()> {
        match self.find_same_book(&b.metadata)? {
            Some(id) => {
                if let Some(p) = &b.personal {
                    if self.apply_personal(&id, p, opts.replace_personal)? {
                        report.personal_updated += 1;
                    }
                }
            }
            None => {
                report.unmatched_count += 1;
                if report.unmatched.len() < 50 {
                    report.unmatched.push(b.metadata.title.clone());
                }
            }
        }
        Ok(())
    }

    fn import_foreign_book(
        &self,
        b: &ForeignBook,
        source: ForeignSource,
        order: &[FileType],
        dest_dir: &Path,
        opts: &ForeignImport,
        report: &mut ForeignReport,
    ) -> Result<()> {
        // One file per book.
        let existing: Vec<_> = b.files.iter().filter(|f| f.path.is_file()).collect();
        let chosen = if source == ForeignSource::Calibre {
            order
                .iter()
                .find_map(|t| existing.iter().find(|f| f.file_type == *t))
                .or_else(|| existing.first())
        } else {
            existing.first()
        }
        .copied();

        let Some(file) = chosen else {
            // No file: its details can still help a book that is here.
            if let Some(id) = self.find_same_book(&b.metadata)? {
                let current = self.record(&id)?;
                let mut m = current.metadata.clone();
                fill_empty(&mut m, &b.metadata);
                if m != current.metadata {
                    self.update_metadata(&id, m)?;
                    report.details_added += 1;
                }
                self.merge_personal_foreign(&id, b, opts, &[], report)?;
            } else {
                report.without_file_count += 1;
                if report.without_file.len() < 50 {
                    report.without_file.push(b.metadata.title.clone());
                }
            }
            return Ok(());
        };

        let hash = paths::hash_file(&file.path)?;
        let id = match self.with_db(|db| db.resolve_book_id(&hash))? {
            Some(id) => {
                report.already_here += 1;
                let current = self.record(&id)?;
                let mut m = current.metadata.clone();
                fill_empty(&mut m, &b.metadata);
                if m != current.metadata {
                    self.update_metadata(&id, m)?;
                }
                if current.missing {
                    // The file was missing here: put this copy back.
                    fs::create_dir_all(dest_dir)?;
                    let dest = unique_path(dest_dir, &file_name(&file.path));
                    fs::copy(&file.path, &dest)?;
                    let rel = paths::rel_of(self.layout(), &dest).ok_or(Error::BookNotFound)?;
                    self.with_db(|db| db.set_book_path(&id, &rel))?;
                    sidecar::write(self, &self.record(&id)?)?;
                }
                id
            }
            None => {
                fs::create_dir_all(dest_dir)?;
                let dest = unique_path(dest_dir, &file_name(&file.path));
                fs::copy(&file.path, &dest)?;
                let registered =
                    self.register(&dest, hash.clone(), file.file_type, &mut report.warnings);
                let book = match registered {
                    Ok(book) => book,
                    Err(e) => {
                        let _ = fs::remove_file(&dest);
                        return Err(e);
                    }
                };
                let mut m = book.metadata.clone();
                overlay(&mut m, &b.metadata);
                // Keep the other app's details even if an ISBN is not valid.
                let fallback = BookMetadata {
                    isbn13: None,
                    isbn10: None,
                    ..m.clone()
                };
                let m = m.normalized().unwrap_or(fallback);
                if m != book.metadata {
                    self.with_db(|db| db.update_metadata(&book.id, &m, &now()))?;
                }
                if let Some(cover) = &b.cover {
                    if let Ok(bytes) = fs::read(cover) {
                        if covers::store(self.layout(), &book.id, &bytes).is_ok() {
                            self.with_db(|db| db.set_has_cover(&book.id, true, &now()))?;
                        }
                    }
                }
                sidecar::write(self, &self.record(&book.id)?)?;
                report.added += 1;
                report.added_ids.push(book.id.clone());
                book.id
            }
        };

        // Highlights made in the file that was copied.
        let marks: Vec<&ForeignAnnotation> = b
            .annotations
            .iter()
            .filter(|a| a.file == file.path)
            .collect();
        let boxes = if marks.is_empty() || file.file_type != FileType::Pdf {
            Vec::new()
        } else {
            libreri_formats::page_boxes(&file.path).unwrap_or_default()
        };
        if !marks.is_empty() && boxes.is_empty() {
            report.warnings.push(format!(
                "{}: the highlights could not be placed (the PDF could not be read)",
                b.metadata.title
            ));
        }
        let converted: Vec<Annotation> = marks
            .iter()
            .filter_map(|a| convert(a, &id, &boxes))
            .collect();
        self.merge_personal_foreign(&id, b, opts, &converted, report)
    }

    /// Status, rating, highlights and notes for the signed-in profile.
    fn merge_personal_foreign(
        &self,
        id: &BookId,
        b: &ForeignBook,
        opts: &ForeignImport,
        annotations: &[Annotation],
        report: &mut ForeignReport,
    ) -> Result<()> {
        let session = self.session_info().ok_or(Error::SignedOut)?;
        if !session.kind.keeps_data() {
            return Ok(());
        }
        if let Some(p) = &b.personal {
            if self.apply_personal(id, p, opts.replace_personal)? {
                report.personal_updated += 1;
            }
        }
        let mut changed = false;
        for a in annotations {
            match self.with_db(|db| db.annotation(&a.id))? {
                None => {
                    self.with_db(|db| db.save_annotation(a, &session.id))?;
                    report.highlights_added += 1;
                    changed = true;
                }
                Some((old, owner)) if owner == session.id && a.modified_at > old.modified_at => {
                    self.with_db(|db| db.save_annotation(a, &session.id))?;
                    changed = true;
                }
                Some(_) => {}
            }
        }
        if changed {
            self.backup_personal(id)?;
        }
        if !b.notes.is_empty() {
            let nb = self.notebook(id)?;
            let mut content = nb.content.clone();
            let mut added = 0;
            for note in &b.notes {
                if !content.contains(note.trim()) {
                    if !content.contains("## From Zotero") {
                        content.push_str("\n## From Zotero\n");
                    }
                    content.push('\n');
                    content.push_str(note.trim());
                    content.push('\n');
                    added += 1;
                }
            }
            if added > 0 {
                self.save_notebook(id, &content)?;
                report.notes_added += added;
            }
        }
        Ok(())
    }

    /// Returns `true` if anything changed.
    fn apply_personal(&self, id: &BookId, p: &ForeignPersonal, replace: bool) -> Result<bool> {
        let Some(session) = self.session_info() else {
            return Ok(false);
        };
        if !session.kind.keeps_data() {
            return Ok(false);
        }
        let current = self.book(id)?.user;
        let mut s = current.clone();
        if let Some(status) = p.status {
            if replace || s.status == ReadingStatus::None {
                s.status = status;
                if status == ReadingStatus::Finished {
                    s.progress = 1.0;
                }
            }
        }
        if p.rating > 0 && (replace || s.rating == 0) {
            s.rating = p.rating.min(5);
        }
        s.favorite |= p.favorite;
        if s.last_opened.is_none() {
            s.last_opened = p.last_read.clone();
        }
        if s == current {
            return Ok(false);
        }
        self.with_db(|db| db.set_user_state(id, &session.id, &s))?;
        self.backup_personal(id)?;
        Ok(true)
    }
}

fn file_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "book".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use crate::*;
    use libreri_core::BookQuery;

    #[test]
    fn rectangles_become_page_fractions() {
        let b = PageBox {
            x0: 0.0,
            y0: 0.0,
            x1: 600.0,
            y1: 800.0,
            rotate: 0,
        };
        let r = to_page_fraction(&[60.0, 700.0, 300.0, 720.0], &b).unwrap();
        assert!((r[0] - 0.1).abs() < 1e-9 && (r[1] - 0.1).abs() < 1e-9);
        assert!((r[2] - 0.4).abs() < 1e-9 && (r[3] - 0.025).abs() < 1e-9);
        let turned =
            to_page_fraction(&[60.0, 700.0, 300.0, 720.0], &PageBox { rotate: 90, ..b }).unwrap();
        // Top-left strip of an upright page is the top-right strip when turned.
        assert!((turned[0] - 0.875).abs() < 1e-9 && (turned[1] - 0.1).abs() < 1e-9);
        assert!(to_page_fraction(&[0.0, 0.0, 0.0, 0.0], &b).is_none());
    }

    #[test]
    fn imports_a_calibre_library_once() {
        let (dir, lib) = library();
        let cal = dir.path().join("Calibre Library");
        libreri_export::foreign::calibre_fixture(&cal);
        let s = lib.inspect_foreign(&cal).unwrap();
        assert_eq!((s.books, s.with_files, s.rated), (2, 2, 1));

        let r = lib
            .import_foreign(
                &cal,
                &ForeignImport {
                    folder: "Calibre".into(),
                    ..Default::default()
                },
                &NoProgress,
            )
            .unwrap();
        assert_eq!(r.added, 2, "{r:?}");
        let books = lib.books(&BookQuery::default()).unwrap();
        let dune = books.iter().find(|b| b.metadata.title == "Dune").unwrap();
        assert_eq!(
            dune.rel_path, "Books/Calibre/Dune - Frank Herbert.epub",
            "EPUB before MOBI"
        );
        assert_eq!(dune.metadata.authors, ["Frank Herbert"]);
        assert_eq!(dune.metadata.series.as_deref(), Some("Dune"));
        assert!(dune.metadata.tags.contains(&"Science Fiction".to_owned()));
        assert_eq!(dune.user.rating, 4);
        assert!(
            cal.join("Frank Herbert/Dune (1)/Dune - Frank Herbert.epub")
                .is_file(),
            "copied, not moved"
        );

        let again = lib
            .import_foreign(&cal, &ForeignImport::default(), &NoProgress)
            .unwrap();
        assert_eq!((again.added, again.already_here), (0, 2));
        assert_eq!(lib.books(&BookQuery::default()).unwrap().len(), 2);

        // MOBI first when asked.
        let (_d2, lib2) = library();
        lib2.import_foreign(
            &cal,
            &ForeignImport {
                formats: vec![FileType::Mobi, FileType::Epub],
                ..Default::default()
            },
            &NoProgress,
        )
        .unwrap();
        assert!(lib2
            .books(&BookQuery::default())
            .unwrap()
            .iter()
            .any(|b| b.rel_path.ends_with(".mobi")));
    }

    #[test]
    fn zotero_highlights_and_notes_come_along() {
        let (dir, lib) = library();
        let z = dir.path().join("Zotero");
        libreri_export::foreign::zotero_fixture(&z);
        // Make the attachment a real three-page PDF.
        let pdf = z.join("storage/ATTKEY01/Vaswani - 2017 - Attention.pdf");
        libreri_formats::test_pdf(&pdf, 3);
        let r = lib
            .import_foreign(&z, &ForeignImport::default(), &NoProgress)
            .unwrap();
        assert_eq!(r.added, 1, "{r:?}");
        assert_eq!(r.without_file_count, 1);
        assert_eq!(r.highlights_added, 1);
        assert_eq!(r.notes_added, 1);
        let paper = lib.books(&BookQuery::default()).unwrap().remove(0);
        assert_eq!(paper.metadata.categories, ["ML/NLP"]);
        let notes = lib.annotations(&paper.id).unwrap();
        assert_eq!(notes.len(), 1);
        assert!(notes[0].locator.contains("\"page\":3"));
        assert_eq!(notes[0].label.as_deref(), Some("p. 3"));
        assert_eq!(notes[0].note.as_deref(), Some("Key idea"));
        assert!(lib
            .notebook(&paper.id)
            .unwrap()
            .content
            .contains("Read section 3 again."));

        // Again: nothing is added twice.
        let again = lib
            .import_foreign(&z, &ForeignImport::default(), &NoProgress)
            .unwrap();
        assert_eq!((again.highlights_added, again.notes_added), (0, 0));
        assert_eq!(lib.annotations(&paper.id).unwrap().len(), 1);
    }

    #[test]
    fn goodreads_only_updates_books_that_are_here() {
        let (dir, lib) = library();
        md_book(&lib.layout().books_dir(), "dune.md", "Dune");
        lib.scan(&NoProgress).unwrap();
        let dune = lib.books(&BookQuery::default()).unwrap().remove(0);
        let mut m = dune.metadata.clone();
        m.authors = vec!["Frank Herbert".into()];
        lib.update_metadata(&dune.id, m).unwrap();
        let csv = dir.path().join("goodreads.csv");
        std::fs::write(
            &csv,
            "Book Id,Title,Author,ISBN,ISBN13,My Rating,Exclusive Shelf,Date Read\n\
             1,Dune,Frank Herbert,,,5,read,2021/03/04\n\
             2,Emma,Jane Austen,,,3,to-read,\n",
        )
        .unwrap();
        let s = lib.inspect_foreign(&csv).unwrap();
        assert_eq!((s.books, s.matched), (2, 1));
        let r = lib
            .import_foreign(&csv, &ForeignImport::default(), &NoProgress)
            .unwrap();
        assert_eq!(r.personal_updated, 1);
        assert_eq!(r.unmatched, ["Emma"]);
        assert_eq!(r.added, 0);
        let d = lib.book(&dune.id).unwrap();
        assert_eq!(d.user.status, ReadingStatus::Finished);
        assert_eq!(d.user.rating, 5);

        // A rating set here is kept unless replacing was asked for.
        lib.set_user_state(
            &dune.id,
            &libreri_core::BookUserState {
                rating: 2,
                ..d.user.clone()
            },
        )
        .unwrap();
        lib.import_foreign(&csv, &ForeignImport::default(), &NoProgress)
            .unwrap();
        assert_eq!(lib.book(&dune.id).unwrap().user.rating, 2);
        lib.import_foreign(
            &csv,
            &ForeignImport {
                replace_personal: true,
                ..Default::default()
            },
            &NoProgress,
        )
        .unwrap();
        assert_eq!(lib.book(&dune.id).unwrap().user.rating, 5);
    }

    #[test]
    fn bibtex_details_reach_books_already_here() {
        let (dir, lib) = library();
        md_book(
            &lib.layout().books_dir(),
            "att.md",
            "Attention Is All You Need",
        );
        lib.scan(&NoProgress).unwrap();
        let bib = dir.path().join("refs.bib");
        std::fs::write(
            &bib,
            "@inproceedings{v, title={Attention Is All You Need}, author={Vaswani, Ashish}, year=2017, doi={10.1/x}, keywords={nlp}}\n",
        )
        .unwrap();
        let r = lib
            .import_foreign(&bib, &ForeignImport::default(), &NoProgress)
            .unwrap();
        assert_eq!(r.details_added, 1, "{r:?}");
        let b = lib.books(&BookQuery::default()).unwrap().remove(0);
        assert_eq!(b.metadata.year, Some(2017));
        assert_eq!(b.metadata.doi.as_deref(), Some("10.1/x"));
        assert_eq!(b.metadata.tags, ["nlp"]);
    }
}
