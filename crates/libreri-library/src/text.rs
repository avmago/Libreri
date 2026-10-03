//! The words inside books: OCR results kept in the library, the search index
//! kept on each computer, and searching.
//!
//! OCR is slow and runs only when asked ("Make searchable"), so its results
//! are saved in the library (`.library-data/text/<bookId>.json`), where they
//! travel with the library, its backups and exports. The
//! search index is only a fast copy of text that can always be read again,
//! so it lives in this computer's cache and is filled in the background.

use crate::{now, Error, Library, Progress, Result};
use libreri_core::{Book, BookId, BookQuery, FileType, LibraryLayout};
use libreri_formats::ocr::OcrPage;
use libreri_formats::text::{self, BookText};
use libreri_search::{Entry, Hit, Piece, SearchIndex, TextState};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Mutex;

/// Bump when text extraction improves, so books are read again.
const EXTRACT_VERSION: u32 = 1;
const OCR_FORMAT: u32 = 1;

/// OCR results for one book, saved in the library.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrText {
    pub format_version: u32,
    /// "tesseract 5.3.4".
    pub engine: String,
    pub languages: Vec<String>,
    pub updated_at: String,
    /// Pages read so far, in page order.
    pub pages: Vec<OcrPage>,
}

impl OcrText {
    pub fn page(&self, page: u32) -> Option<&OcrPage> {
        self.pages.iter().find(|p| p.page == page)
    }
}

pub(crate) fn ocr_path(layout: &LibraryLayout, id: &BookId) -> PathBuf {
    layout.data_dir().join("text").join(format!("{id}.json"))
}

pub(crate) fn rename(layout: &LibraryLayout, old: &BookId, new: &BookId) {
    let _ = fs::rename(ocr_path(layout, old), ocr_path(layout, new));
}

/// What to OCR and how.
#[derive(Clone)]
pub struct OcrOptions {
    /// Tesseract language codes.
    pub languages: Vec<String>,
    /// Tesseract's data folder, when not its own.
    pub tessdata: Option<PathBuf>,
    /// Read pages again that were read before (other languages).
    pub redo: bool,
    /// Pages to read side by side.
    pub workers: usize,
    /// A downloaded model to read with instead of Tesseract (one page at
    /// a time; languages and Tesseract data are then not used).
    pub reader: Option<std::sync::Arc<dyn libreri_formats::ocr::PageReader>>,
}

impl std::fmt::Debug for OcrOptions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OcrOptions")
            .field("languages", &self.languages)
            .field("redo", &self.redo)
            .field("workers", &self.workers)
            .field("reader", &self.reader.as_ref().map(|r| r.engine()))
            .finish_non_exhaustive()
    }
}

/// What an OCR run did.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct OcrReport {
    pub pages_read: u32,
    pub pages_failed: u32,
    pub words: u32,
    pub errors: Vec<String>,
}

/// A book's text, as the details panel shows it.
#[derive(Debug, Clone, PartialEq)]
pub struct TextStatus {
    /// `None` while the book waits to be indexed.
    pub state: Option<TextState>,
    pub pages: u32,
    pub empty_pages: u32,
    pub ocr_pages: u32,
    pub ocr_languages: Vec<String>,
    pub can_ocr: bool,
    pub message: Option<String>,
}

/// One book's matches.
#[derive(Debug, Clone)]
pub struct TextResult {
    pub book: Book,
    pub total: u32,
    pub hits: Vec<Hit>,
}

/// What an indexing pass did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IndexReport {
    pub indexed: u32,
    pub removed: u32,
    pub remaining: u32,
}

fn file_stamp(path: &Path) -> String {
    fs::metadata(path)
        .ok()
        .map(|m| {
            let mtime = m
                .modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map_or(0, |d| d.as_secs());
            format!("{}-{mtime}", m.len())
        })
        .unwrap_or_else(|| "-".into())
}

/// Letters in a string (to tell pages with text from scans).
fn letters(s: &str) -> usize {
    s.chars().filter(|c| c.is_alphanumeric()).count()
}

impl Library {
    /// Saved OCR text of a book, if any.
    pub fn ocr_text(&self, id: &BookId) -> Result<Option<OcrText>> {
        let path = ocr_path(self.layout(), id);
        match fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|e| Error::InvalidInput(format!("saved OCR text is damaged: {e}"))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub(crate) fn save_ocr(&self, id: &BookId, ocr: &OcrText) -> Result<()> {
        let path = ocr_path(self.layout(), id);
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = path.with_extension("json.part");
        fs::write(
            &tmp,
            serde_json::to_vec(ocr).map_err(std::io::Error::other)?,
        )?;
        fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Saved OCR text, kept in memory for the few books being read (the
    /// reader asks for one page at a time).
    pub(crate) fn ocr_cached(&self, id: &BookId) -> Option<std::sync::Arc<OcrText>> {
        type Cache = Mutex<Vec<(PathBuf, String, std::sync::Arc<OcrText>)>>;
        static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
        let path = ocr_path(self.layout(), id);
        let stamp = file_stamp(&path);
        if stamp == "-" {
            return None;
        }
        let cache = CACHE.get_or_init(Default::default);
        let mut list = cache.lock().unwrap_or_else(|p| p.into_inner());
        if let Some((_, _, t)) = list.iter().find(|(p, s, _)| *p == path && *s == stamp) {
            return Some(t.clone());
        }
        let text = std::sync::Arc::new(self.ocr_text(id).ok().flatten()?);
        list.retain(|(p, _, _)| *p != path);
        list.insert(0, (path, stamp, text.clone()));
        list.truncate(4);
        Some(text)
    }

    /// OCR words of one page, in the reader's word shape.
    pub(crate) fn ocr_words(&self, id: &BookId, page: u32) -> Vec<libreri_formats::djvu::Word> {
        self.ocr_cached(id)
            .and_then(|t| {
                t.page(page).map(|p| {
                    p.words
                        .iter()
                        .map(|w| libreri_formats::djvu::Word {
                            text: w.text.clone(),
                            rect: w.rect.map(f64::from),
                        })
                        .collect()
                })
            })
            .unwrap_or_default()
    }

    /// Removes a book's OCR text (it falls back to the file's own text).
    pub fn forget_ocr(&self, id: &BookId, index: Option<&SearchIndex>) -> Result<()> {
        self.require_edit()?;
        let book = self.book(id)?;
        match fs::remove_file(ocr_path(self.layout(), &book.id)) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e.into()),
            _ => {}
        }
        if let Some(index) = index {
            self.index_book(index, &book)?;
        }
        Ok(())
    }

    /// Which version of a book's text the index should hold.
    fn text_stamp(&self, book: &Book) -> String {
        let ocr = file_stamp(&ocr_path(self.layout(), &book.id));
        let helper = match book.file_type {
            FileType::Djvu => {
                libreri_helpers::status(libreri_helpers::Helper::DjVuLibre).installed as u8
            }
            _ => 1,
        };
        format!("v{EXTRACT_VERSION}|{ocr}|{helper}|{}", book.missing as u8)
    }

    /// Reads a book's text (with saved OCR text for pages that have none)
    /// and stores it in the index.
    pub fn index_book(&self, index: &SearchIndex, book: &Book) -> Result<Entry> {
        let stamp = self.text_stamp(book);
        let failed = |message: String| Entry {
            stamp: stamp.clone(),
            state: TextState::Failed,
            empty_pages: 0,
            pages: 0,
            ocr: false,
            message: Some(message),
        };
        let entry_and_pieces = if !text::has_text(book.file_type) {
            (
                Entry {
                    stamp: stamp.clone(),
                    state: TextState::NoWords,
                    empty_pages: 0,
                    pages: 0,
                    ocr: false,
                    message: None,
                },
                Vec::new(),
            )
        } else if book.missing {
            (failed("the file is missing".into()), Vec::new())
        } else {
            let path = self
                .layout()
                .resolve_relative(&book.rel_path)
                .ok_or(Error::BookNotFound)?;
            match text::book_text(&path, book.file_type) {
                Ok(t) => {
                    let ocr = self.ocr_text(&book.id).ok().flatten();
                    combine(stamp.clone(), t, ocr.as_ref())
                }
                Err(e) => (failed(e), Vec::new()),
            }
        };
        let (entry, pieces) = entry_and_pieces;
        index
            .put(&book.id.to_string(), &entry, &pieces)
            .map_err(Error::InvalidInput)?;
        Ok(entry)
    }

    /// Brings the index up to date: reads books that are new or changed and
    /// forgets books that are gone. Stops early when cancelled.
    pub fn update_index(
        &self,
        index: &SearchIndex,
        progress: &dyn Progress,
    ) -> Result<IndexReport> {
        let records = self.with_db(|db| db.file_records())?;
        let entries = index.entries().map_err(Error::InvalidInput)?;
        let keep: HashSet<String> = records.iter().map(|r| r.id.to_string()).collect();
        let mut report = IndexReport {
            removed: index.retain(&keep).map_err(Error::InvalidInput)?,
            ..Default::default()
        };
        let mut todo = Vec::new();
        for r in &records {
            let Ok(book) = self.record(&r.id) else {
                continue;
            };
            let current = entries.get(&r.id.to_string()).map(|e| e.stamp.as_str());
            if current != Some(self.text_stamp(&book).as_str()) {
                todo.push(book);
            }
        }
        let total = todo.len() as u64;
        for (i, book) in todo.iter().enumerate() {
            if progress.cancelled() {
                report.remaining = (todo.len() - i) as u32;
                return Ok(report);
            }
            progress.report(i as u64, total, &book.metadata.title);
            // One unreadable book must not stop the rest.
            if self.index_book(index, book).is_ok() {
                report.indexed += 1;
            }
        }
        progress.report(total, total, "");
        Ok(report)
    }

    /// What the index knows about a book's text.
    pub fn text_status(&self, index: &SearchIndex, id: &BookId) -> Result<TextStatus> {
        let book = self.book(id)?;
        let entry = index
            .entry(&book.id.to_string())
            .map_err(Error::InvalidInput)?
            .filter(|e| e.stamp == self.text_stamp(&book));
        let ocr = self.ocr_text(&book.id).ok().flatten();
        Ok(TextStatus {
            state: entry.as_ref().map(|e| e.state),
            pages: entry.as_ref().map_or(0, |e| e.pages),
            empty_pages: entry.as_ref().map_or(0, |e| e.empty_pages),
            ocr_pages: ocr.as_ref().map_or(0, |o| o.pages.len() as u32),
            ocr_languages: ocr.map(|o| o.languages).unwrap_or_default(),
            can_ocr: text::can_ocr(book.file_type) && !book.missing,
            message: entry.and_then(|e| e.message),
        })
    }

    /// Books whose text matches `query`, best first, limited to what the
    /// signed-in profile may see.
    pub fn search_text(
        &self,
        index: &SearchIndex,
        query: &str,
        limit: usize,
    ) -> Result<Vec<TextResult>> {
        let visible: HashMap<String, Book> = self
            .books(&BookQuery::default())?
            .into_iter()
            .map(|b| (b.id.to_string(), b))
            .collect();
        let found = index
            .search(query, &|id| visible.contains_key(id), limit, 3)
            .map_err(Error::InvalidInput)?;
        Ok(found
            .into_iter()
            .filter_map(|h| {
                Some(TextResult {
                    book: visible.get(&h.book_id)?.clone(),
                    total: h.total,
                    hits: h.hits,
                })
            })
            .collect())
    }

    /// Books the signed-in profile can see whose pages are scans without
    /// text (all or some), for marking them in the library.
    pub fn books_without_text(&self, index: &SearchIndex) -> Result<Vec<(BookId, TextState)>> {
        let entries = index.entries().map_err(Error::InvalidInput)?;
        Ok(self
            .books(&BookQuery::default())?
            .into_iter()
            .filter_map(|b| {
                let e = entries.get(&b.id.to_string())?;
                matches!(e.state, TextState::NoText | TextState::Partial)
                    .then(|| (b.id.clone(), e.state))
            })
            .collect())
    }

    /// Every match in one book, in reading order.
    pub fn search_in_book(
        &self,
        index: &SearchIndex,
        id: &BookId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<Hit>> {
        let book = self.book(id)?;
        index
            .book_hits(query, &book.id.to_string(), limit)
            .map_err(Error::InvalidInput)
    }

    /// Reads the pages of a scanned PDF or DjVu book with Tesseract and
    /// saves the text in the library. Pages that already have text are
    /// left alone; pages read before are skipped unless `redo`. Saves as it
    /// goes, so a cancelled run keeps what it read.
    pub fn make_searchable(
        &self,
        id: &BookId,
        options: &OcrOptions,
        scratch: &Path,
        index: Option<&SearchIndex>,
        progress: &dyn Progress,
    ) -> Result<OcrReport> {
        self.require_edit()?;
        let book = self.book(id)?;
        if !text::can_ocr(book.file_type) {
            return Err(Error::InvalidInput(
                "only PDF and DjVu books can be made searchable".into(),
            ));
        }
        if book.missing {
            return Err(Error::InvalidInput("the book's file is missing".into()));
        }
        if options.reader.is_none() && libreri_helpers::find_program("tesseract").is_none() {
            return Err(Error::InvalidInput(
                libreri_formats::ocr::NOT_INSTALLED.into(),
            ));
        }
        let path = self
            .layout()
            .resolve_relative(&book.rel_path)
            .ok_or(Error::BookNotFound)?;
        progress.report(0, 1, "Looking for pages without text");
        let own = text::book_text(&path, book.file_type).map_err(Error::InvalidInput)?;
        let mut saved = self.ocr_text(&book.id).ok().flatten().unwrap_or_default();
        if options.redo {
            saved.pages.clear();
        }
        let todo: Vec<u32> = own
            .empty_pages
            .iter()
            .copied()
            .filter(|p| saved.page(*p).is_none())
            .collect();
        let mut report = OcrReport::default();
        if todo.is_empty() {
            if let Some(index) = index {
                self.index_book(index, &book)?;
            }
            return Ok(report);
        }
        saved.format_version = OCR_FORMAT;
        saved.engine = match &options.reader {
            Some(r) => r.engine(),
            None => libreri_helpers::status(libreri_helpers::Helper::Tesseract)
                .version
                .map(|v| format!("tesseract {v}"))
                .unwrap_or_else(|| "tesseract".into()),
        };
        saved.languages = if options.reader.is_some() {
            Vec::new()
        } else {
            options.languages.clone()
        };

        let work = scratch.join(format!("ocr-{}", book.id));
        fs::create_dir_all(&work)?;
        let source = match book.file_type {
            FileType::Pdf => Source::Pdf(
                libreri_formats::pdftext::PdfDoc::open(&path).map_err(Error::InvalidInput)?,
            ),
            _ => Source::Djvu {
                path: path.clone(),
                sizes: libreri_formats::djvu::info(&path)
                    .map_err(Error::InvalidInput)?
                    .sizes,
            },
        };
        let total = todo.len() as u64;
        let next = AtomicU32::new(0);
        let stop = AtomicBool::new(false);
        // A model uses the whole computer for one page.
        let workers = if options.reader.is_some() {
            1
        } else {
            options.workers.clamp(1, 8).min(todo.len())
        };
        progress.report(0, total, "Reading pages");
        let (tx, rx) = std::sync::mpsc::channel::<(u32, std::result::Result<OcrPage, String>)>();
        std::thread::scope(|s| {
            for _ in 0..workers {
                let tx = tx.clone();
                let (next, stop, todo, source, work) = (&next, &stop, &todo, &source, &work);
                s.spawn(move || loop {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let i = next.fetch_add(1, Ordering::Relaxed) as usize;
                    let Some(&page) = todo.get(i) else { return };
                    let read = source.read_page(page, work, options);
                    if tx.send((page, read)).is_err() {
                        return;
                    }
                });
            }
            drop(tx);
            // Results come back here, where progress is reported and the
            // text is saved every few pages so a stop loses little.
            let mut done = 0u64;
            for (page, read) in rx {
                done += 1;
                match read {
                    Ok(p) => {
                        report.pages_read += 1;
                        report.words += p.words.len() as u32;
                        saved.pages.push(p);
                    }
                    Err(e) => {
                        report.pages_failed += 1;
                        // Tesseract (or the model) itself is broken: no
                        // point going on.
                        if e.contains("could not start")
                            || e.contains("language")
                            || e.contains("could not load")
                        {
                            stop.store(true, Ordering::Relaxed);
                        }
                        if report.errors.len() < 5 && !report.errors.contains(&e) {
                            report.errors.push(e);
                        }
                    }
                }
                if done.is_multiple_of(8) {
                    saved.pages.sort_by_key(|p| p.page);
                    saved.updated_at = now();
                    let _ = self.save_ocr(&book.id, &saved);
                }
                progress.report(done, total, &format!("Page {page}"));
                if progress.cancelled() {
                    stop.store(true, Ordering::Relaxed);
                }
            }
        });
        let _ = fs::remove_dir_all(&work);
        saved.pages.sort_by_key(|p| p.page);
        saved.updated_at = now();
        if !saved.pages.is_empty() {
            self.save_ocr(&book.id, &saved)?;
        }
        if let Some(index) = index {
            self.index_book(index, &book)?;
        }
        if report.pages_read == 0 && report.pages_failed > 0 {
            return Err(Error::InvalidInput(
                report
                    .errors
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "no page could be read".into()),
            ));
        }
        if progress.cancelled() {
            return Err(Error::Cancelled);
        }
        Ok(report)
    }
}

enum Source {
    Pdf(libreri_formats::pdftext::PdfDoc),
    Djvu {
        path: PathBuf,
        sizes: Vec<(u32, u32)>,
    },
}

impl Source {
    fn read_page(
        &self,
        page: u32,
        work: &Path,
        options: &OcrOptions,
    ) -> std::result::Result<OcrPage, String> {
        let (file, dpi) = match self {
            Source::Pdf(doc) => {
                let (png, dpi) = doc.render_png(page, 300.0, 5000)?;
                let file = work.join(format!("{page}.png"));
                fs::write(&file, png).map_err(|e| e.to_string())?;
                (file, Some(dpi))
            }
            Source::Djvu { path, sizes } => {
                let native = sizes
                    .get(page.saturating_sub(1) as usize)
                    .map_or(2400, |s| s.0);
                let width = native.clamp(1200, 3600);
                let pnm = libreri_formats::djvu::render_page(path, page, width)?;
                let file = work.join(format!("{page}.pnm"));
                fs::write(&file, pnm).map_err(|e| e.to_string())?;
                (file, None)
            }
        };
        let read = match &options.reader {
            Some(reader) => fs::read(&file)
                .map_err(|e| e.to_string())
                .and_then(|bytes| reader.read_page(&bytes, page)),
            None => libreri_formats::ocr::recognize(
                &file,
                page,
                &options.languages,
                options.tessdata.as_deref(),
                dpi,
            ),
        };
        let _ = fs::remove_file(&file);
        read
    }
}

/// Puts a book's own text and its OCR text together for the index.
fn combine(stamp: String, own: BookText, ocr: Option<&OcrText>) -> (Entry, Vec<Piece>) {
    let mut pieces: Vec<Piece> = own
        .chunks
        .into_iter()
        .map(|c| Piece {
            page: c.page,
            section: c.section,
            label: c.label,
            text: c.text,
        })
        .collect();
    if own.pages == 0 {
        let state = if pieces.is_empty() {
            TextState::NoText
        } else {
            TextState::Text
        };
        let entry = Entry {
            stamp,
            state,
            empty_pages: 0,
            pages: 0,
            ocr: false,
            message: None,
        };
        return (entry, pieces);
    }
    let mut empty = 0u32;
    let mut used_ocr = false;
    for p in &own.empty_pages {
        match ocr
            .and_then(|o| o.page(*p))
            .filter(|o| letters(&o.text) > 0)
        {
            Some(o) => {
                used_ocr = true;
                // A page with a few letters of its own gets the OCR text
                // instead.
                pieces.retain(|x| x.page != Some(*p));
                pieces.push(Piece {
                    page: Some(*p),
                    text: o.text.clone(),
                    ..Default::default()
                });
            }
            None => empty += 1,
        }
    }
    pieces.sort_by_key(|p| p.page);
    let pages = own.pages;
    let with_text = pages - empty.min(pages);
    let state = if with_text == 0 || with_text * 10 < pages {
        TextState::NoText
    } else if empty >= 3 && empty * 10 > pages {
        TextState::Partial
    } else {
        TextState::Text
    };
    let entry = Entry {
        stamp,
        state,
        empty_pages: empty,
        pages,
        ocr: used_ocr,
        message: None,
    };
    (entry, pieces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::library;
    use crate::{ImportMode, ImportRequest, NoProgress};

    fn import(lib: &Library, src: &Path) -> BookId {
        let r = lib
            .import(
                &ImportRequest {
                    sources: vec![src.to_path_buf()],
                    folder: String::new(),
                    mode: ImportMode::Copy,
                },
                &NoProgress,
            )
            .unwrap();
        r.added_ids[0].clone()
    }

    #[test]
    fn indexes_and_searches_books() {
        let (dir, lib) = library();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        let pdf = src.join("keeper.pdf");
        libreri_formats::test_text_pdf(
            &pdf,
            &["The lighthouse keeper wrote every night", "Storms came"],
        );
        let pdf_id = import(&lib, &pdf);
        let md = crate::testutil::md_book(&src, "notes.md", "Harbour Notes");
        fs::write(&md, "# Harbour Notes\n\nThe keeper's daughter rowed out.").unwrap();
        let md_id = import(&lib, &md);

        let index = SearchIndex::open(&dir.path().join("cache/index.sqlite")).unwrap();
        let r = lib.update_index(&index, &NoProgress).unwrap();
        assert_eq!(r.indexed, 2);
        assert_eq!(lib.update_index(&index, &NoProgress).unwrap().indexed, 0);

        let found = lib.search_text(&index, "keeper", 10).unwrap();
        assert_eq!(found.len(), 2);
        let p = found.iter().find(|f| f.book.id == pdf_id).unwrap();
        assert_eq!(p.hits[0].page, Some(1));
        assert!(lib
            .search_text(&index, "daughter", 10)
            .unwrap()
            .iter()
            .all(|f| f.book.id == md_id));
        let status = lib.text_status(&index, &pdf_id).unwrap();
        assert_eq!(status.state, Some(TextState::Text));
        assert!(status.can_ocr);
    }

    #[test]
    fn scanned_pages_and_saved_ocr_text() {
        let (dir, lib) = library();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        let pdf = src.join("scan.pdf");
        libreri_formats::test_text_pdf(&pdf, &["", "", "", ""]);
        let id = import(&lib, &pdf);
        let index = SearchIndex::open(&dir.path().join("cache/index.sqlite")).unwrap();
        lib.update_index(&index, &NoProgress).unwrap();
        let s = lib.text_status(&index, &id).unwrap();
        assert_eq!(s.state, Some(TextState::NoText));
        assert_eq!(s.empty_pages, 4);

        // Saved OCR text (as if Tesseract had read pages 1–3).
        let ocr = OcrText {
            format_version: 1,
            engine: "tesseract 5".into(),
            languages: vec!["eng".into()],
            updated_at: now(),
            pages: (1..=3)
                .map(|p| OcrPage {
                    page: p,
                    text: format!("Scanned page {p} about tides"),
                    ..Default::default()
                })
                .collect(),
        };
        lib.save_ocr(&id, &ocr).unwrap();
        assert_eq!(lib.update_index(&index, &NoProgress).unwrap().indexed, 1);
        let s = lib.text_status(&index, &id).unwrap();
        assert_eq!(s.state, Some(TextState::Text));
        assert_eq!(s.empty_pages, 1);
        assert_eq!(s.ocr_pages, 3);
        let hits = lib.search_in_book(&index, &id, "tides", 10).unwrap();
        assert_eq!(
            hits.iter().map(|h| h.page).collect::<Vec<_>>(),
            vec![Some(1), Some(2), Some(3)]
        );

        lib.forget_ocr(&id, Some(&index)).unwrap();
        assert_eq!(
            lib.text_status(&index, &id).unwrap().state,
            Some(TextState::NoText)
        );
    }

    #[test]
    fn ocr_reads_a_scanned_pdf() {
        if libreri_helpers::find_program("tesseract").is_none() {
            eprintln!("Tesseract is not installed; skipped");
            return;
        }
        let (dir, lib) = library();
        let src = dir.path().join("src");
        fs::create_dir_all(&src).unwrap();
        // A "scan": the text is drawn as a picture, so the PDF has no text.
        let png_pdf = src.join("scan.pdf");
        make_scan_pdf(
            &png_pdf,
            &[
                "The lighthouse keeper wrote",
                "Waves broke on the harbour wall",
            ],
        );
        let id = import(&lib, &png_pdf);
        let index = SearchIndex::open(&dir.path().join("cache/index.sqlite")).unwrap();
        lib.update_index(&index, &NoProgress).unwrap();
        assert_eq!(
            lib.text_status(&index, &id).unwrap().state,
            Some(TextState::NoText)
        );
        let options = OcrOptions {
            languages: vec!["eng".into()],
            tessdata: None,
            redo: false,
            workers: 2,
            reader: None,
        };
        let r = lib
            .make_searchable(
                &id,
                &options,
                &dir.path().join("cache"),
                Some(&index),
                &NoProgress,
            )
            .unwrap();
        assert_eq!(r.pages_read, 2, "{r:?}");
        assert!(ocr_path(lib.layout(), &id).is_file());
        let found = lib.search_text(&index, "harbour wall", 10).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].hits[0].page, Some(2));
        // Nothing left to read.
        let again = lib
            .make_searchable(
                &id,
                &options,
                &dir.path().join("cache"),
                Some(&index),
                &NoProgress,
            )
            .unwrap();
        assert_eq!(again.pages_read, 0);
    }

    /// A PDF whose pages are images of text (like a scanner makes).
    fn make_scan_pdf(path: &Path, lines: &[&str]) {
        use lopdf::{dictionary, Document, Object, Stream};
        // Render each line with the text PDF and hayro, then embed it as an
        // image-only page.
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let mut kids = Vec::new();
        let tmp = tempfile::tempdir().unwrap();
        for line in lines {
            let t = tmp.path().join("t.pdf");
            libreri_formats::test_text_pdf(&t, &[line]);
            let png = libreri_formats::pdftext::render_page_png(&t, 1, 150.0, 4000).unwrap();
            let img = image::load_from_memory(&png).unwrap().to_luma8();
            let (w, h) = img.dimensions();
            let image = doc.add_object(Stream::new(
                dictionary! {
                    "Type" => "XObject", "Subtype" => "Image",
                    "Width" => w as i64, "Height" => h as i64,
                    "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
                },
                img.into_raw(),
            ));
            let content = doc.add_object(Stream::new(
                dictionary! {},
                b"q 612 0 0 792 0 0 cm /Im1 Do Q".to_vec(),
            ));
            let page = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
                "Contents" => content,
                "Resources" => dictionary! { "XObject" => dictionary! { "Im1" => image } },
            });
            kids.push(Object::Reference(page));
        }
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Count" => kids.len() as i64, "Kids" => kids,
            }),
        );
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog);
        doc.save(path).unwrap();
    }
}
