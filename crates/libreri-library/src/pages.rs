//! Page images for the comic and DjVu readers.
//!
//! Comics: CBZ pages are read straight from the ZIP; CBR, CB7, CBT and CBA
//! are unpacked once into the page cache because they cannot be read page
//! by page quickly. DjVu pages are rendered by DjVuLibre at a few widths and
//! kept as JPEG. The cache belongs to this computer (it is not in the
//! library, so it is never synced or backed up) and is given by the caller;
//! [`prune_page_cache`] keeps it under a size.

use crate::{Error, Library, Result};
use libreri_core::{BookId, FileType};
use libreri_formats::djvu::{self, DjvuInfo, OutlineItem, Word};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// A book shown as page images.
#[derive(Debug, Clone, PartialEq)]
pub struct PageBook {
    pub kind: PageKind,
    pub pages: u32,
    /// Page sizes in pixels (DjVu); comics are measured when shown.
    pub sizes: Vec<(u32, u32)>,
    /// ComicInfo says right to left.
    pub right_to_left: bool,
    pub outline: Vec<OutlineItem>,
    /// DjVu: the book has a text layer (so it can be selected and searched).
    pub has_text: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageKind {
    Comic,
    Djvu,
}

/// Widths DjVu pages are rendered at (the reader asks for the next one up).
const WIDTHS: &[u32] = &[480, 800, 1200, 1600, 2000, 2600, 3200];

fn bucket(width: u32) -> u32 {
    WIDTHS
        .iter()
        .copied()
        .find(|w| *w >= width)
        .unwrap_or(*WIDTHS.last().expect("not empty"))
}

/// One lock per book, so two requests never unpack or render the same thing
/// at once.
fn book_lock(id: &BookId) -> Arc<Mutex<()>> {
    static LOCKS: OnceLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> = OnceLock::new();
    let mut map = LOCKS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|p| p.into_inner());
    map.entry(id.to_string()).or_default().clone()
}

fn mime(path: &str) -> &'static str {
    match path
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("png") => "image/png",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        _ => "image/jpeg",
    }
}

fn is_comic(t: FileType) -> bool {
    matches!(
        t,
        FileType::Cbz | FileType::Cbr | FileType::Cb7 | FileType::Cbt | FileType::Cba
    )
}

impl Library {
    /// The book's file, if the signed-in profile may open it.
    fn page_source(&self, id: &BookId) -> Result<(PathBuf, FileType)> {
        let book = self.book(id)?;
        if !self.may_open(&book.rel_path) || book.missing {
            return Err(Error::BookNotFound);
        }
        let path = self
            .layout()
            .resolve_relative(&book.rel_path)
            .ok_or(Error::BookNotFound)?;
        Ok((path, book.file_type))
    }

    fn djvu_info(&self, path: &Path, dir: &Path) -> Result<DjvuInfo> {
        let cached = dir.join("djvu.json");
        if let Ok(text) = fs::read_to_string(&cached) {
            if let Ok(info) = serde_json::from_str(&text) {
                return Ok(info);
            }
        }
        let info = djvu::info(path).map_err(Error::InvalidInput)?;
        fs::create_dir_all(dir)?;
        let _ = fs::write(
            &cached,
            serde_json::to_vec(&info).map_err(std::io::Error::other)?,
        );
        Ok(info)
    }

    /// Opens a comic or DjVu book for the reader. `cache` is this
    /// computer's page cache folder.
    pub fn open_pages(&self, id: &BookId, cache: &Path) -> Result<PageBook> {
        let (path, kind) = self.page_source(id)?;
        let dir = cache.join(id.to_string());
        let lock = book_lock(id);
        let _held = lock.lock().unwrap_or_else(|p| p.into_inner());
        if kind == FileType::Djvu {
            let info = self.djvu_info(&path, &dir)?;
            // A quick look at the first pages tells if there is a text layer.
            let has_text = (1..=info.sizes.len().min(5) as u32)
                .any(|p| djvu::page_words(&path, p).is_ok_and(|w| !w.is_empty()))
                || self.ocr_cached(&self.book(id)?.id).is_some();
            return Ok(PageBook {
                kind: PageKind::Djvu,
                pages: info.sizes.len() as u32,
                sizes: info.sizes,
                right_to_left: false,
                outline: info.outline,
                has_text,
            });
        }
        if !is_comic(kind) {
            return Err(Error::InvalidInput(
                "this book is not made of page images".into(),
            ));
        }
        let pages = if kind == FileType::Cbz {
            libreri_formats::list_pages(&path, kind).map_err(Error::InvalidInput)?
        } else {
            libreri_formats::extract_pages(&path, kind, &dir)
                .map_err(Error::InvalidInput)?
                .0
        };
        if pages.pages.is_empty() {
            return Err(Error::InvalidInput(
                "no pages were found in this comic".into(),
            ));
        }
        Ok(PageBook {
            kind: PageKind::Comic,
            pages: pages.pages.len() as u32,
            sizes: Vec::new(),
            right_to_left: pages.right_to_left,
            outline: Vec::new(),
            has_text: false,
        })
    }

    /// One page (1-based) as image bytes and their type. `width` is what the
    /// reader needs on screen (DjVu renders to fit it; comic pages are
    /// served as they are).
    pub fn page_image(
        &self,
        id: &BookId,
        page: u32,
        width: u32,
        cache: &Path,
    ) -> Result<(Vec<u8>, &'static str)> {
        let (path, kind) = self.page_source(id)?;
        let dir = cache.join(id.to_string());
        if page == 0 {
            return Err(Error::BookNotFound);
        }
        if kind == FileType::Djvu {
            let w = bucket(width);
            let file = dir.join(format!("djvu-{page}-{w}.jpg"));
            if let Ok(bytes) = fs::read(&file) {
                return Ok((bytes, "image/jpeg"));
            }
            let lock = book_lock(id);
            let _held = lock.lock().unwrap_or_else(|p| p.into_inner());
            let info = self.djvu_info(&path, &dir)?;
            if page as usize > info.sizes.len() {
                return Err(Error::BookNotFound);
            }
            let pnm = djvu::render_page(&path, page, w).map_err(Error::InvalidInput)?;
            let jpg = libreri_thumbs::to_jpeg(&pnm, 85)
                .map_err(|e| Error::InvalidInput(e.to_string()))?;
            fs::create_dir_all(&dir)?;
            let tmp = file.with_extension("part");
            fs::write(&tmp, &jpg)?;
            fs::rename(&tmp, &file)?;
            return Ok((jpg, "image/jpeg"));
        }
        if kind == FileType::Cbz {
            let pages = libreri_formats::list_pages(&path, kind).map_err(Error::InvalidInput)?;
            let name = pages
                .pages
                .get(page as usize - 1)
                .ok_or(Error::BookNotFound)?;
            let bytes = libreri_formats::read_zip_page(&path, name).map_err(Error::InvalidInput)?;
            return Ok((bytes, mime(name)));
        }
        if !is_comic(kind) {
            return Err(Error::BookNotFound);
        }
        let lock = book_lock(id);
        let files = {
            let _held = lock.lock().unwrap_or_else(|p| p.into_inner());
            libreri_formats::extract_pages(&path, kind, &dir)
                .map_err(Error::InvalidInput)?
                .1
        };
        let file = files.get(page as usize - 1).ok_or(Error::BookNotFound)?;
        let name = file.to_string_lossy().into_owned();
        Ok((fs::read(file)?, mime(&name)))
    }

    /// The words of a page with their boxes: a DjVu page's text layer, or
    /// saved OCR text (DjVu and PDF). Empty when there is none.
    pub fn page_words(&self, id: &BookId, page: u32) -> Result<Vec<Word>> {
        let (path, kind) = self.page_source(id)?;
        let id = self.book(id)?.id;
        match kind {
            FileType::Djvu => {
                let words = djvu::page_words(&path, page).map_err(Error::InvalidInput)?;
                if !words.is_empty() {
                    return Ok(words);
                }
            }
            FileType::Pdf => {}
            _ => return Ok(Vec::new()),
        }
        Ok(self.ocr_words(&id, page))
    }

    /// Pages of a PDF that have saved OCR text (the reader adds a hidden
    /// text layer to them).
    pub fn ocr_pages(&self, id: &BookId) -> Result<Vec<u32>> {
        let id = self.book(id)?.id;
        Ok(self
            .ocr_cached(&id)
            .map(|t| {
                t.pages
                    .iter()
                    .filter(|p| !p.words.is_empty())
                    .map(|p| p.page)
                    .collect()
            })
            .unwrap_or_default())
    }
}

impl Library {
    /// The plain text of every DjVu page (for finding), cached per computer;
    /// for a PDF, the text of its OCR-read pages.
    pub fn page_texts(&self, id: &BookId, cache: &Path) -> Result<Vec<String>> {
        let (path, kind) = self.page_source(id)?;
        if kind == FileType::Pdf {
            // Scanned PDFs: the OCR text, by page (PDF.js finds the rest).
            let Some(ocr) = self.ocr_cached(&self.book(id)?.id) else {
                return Ok(Vec::new());
            };
            let last = ocr.pages.iter().map(|p| p.page).max().unwrap_or(0) as usize;
            let mut list = vec![String::new(); last];
            for p in ocr.pages.iter().filter(|p| p.page > 0) {
                list[p.page as usize - 1] = p.text.clone();
            }
            return Ok(list);
        }
        if kind != FileType::Djvu {
            return Ok(Vec::new());
        }
        let dir = cache.join(id.to_string());
        let file = dir.join("text.json");
        if let Ok(text) = fs::read_to_string(&file) {
            if let Ok(list) = serde_json::from_str(&text) {
                return Ok(self.with_ocr(id, list));
            }
        }
        let info = self.djvu_info(&path, &dir)?;
        let list = djvu::page_texts(&path, info.sizes.len()).map_err(Error::InvalidInput)?;
        fs::create_dir_all(&dir)?;
        let _ = fs::write(
            &file,
            serde_json::to_vec(&list).map_err(std::io::Error::other)?,
        );
        Ok(self.with_ocr(id, list))
    }

    /// Fills pages without a text layer from saved OCR text.
    fn with_ocr(&self, id: &BookId, mut list: Vec<String>) -> Vec<String> {
        let Some(ocr) = self.book(id).ok().and_then(|b| self.ocr_cached(&b.id)) else {
            return list;
        };
        for p in &ocr.pages {
            if let Some(slot) = list.get_mut(p.page.saturating_sub(1) as usize) {
                if slot.chars().filter(|c| c.is_alphanumeric()).count() < 16 {
                    *slot = p.text.clone();
                }
            }
        }
        list
    }
}

/// Size of the page cache in bytes.
pub fn page_cache_size(cache: &Path) -> u64 {
    walkdir::WalkDir::new(cache)
        .into_iter()
        .flatten()
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Removes the least recently used books' pages until the cache is under
/// `limit` bytes. Returns the bytes freed.
pub fn prune_page_cache(cache: &Path, limit: u64) -> u64 {
    let Ok(entries) = fs::read_dir(cache) else {
        return 0;
    };
    let mut books: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .map(|e| {
            let size = page_cache_size(&e.path());
            let used = walkdir::WalkDir::new(e.path())
                .into_iter()
                .flatten()
                .filter_map(|f| f.metadata().ok()?.modified().ok())
                .max()
                .unwrap_or(std::time::UNIX_EPOCH);
            (used, size, e.path())
        })
        .collect();
    let mut total: u64 = books.iter().map(|b| b.1).sum();
    books.sort_by_key(|b| b.0);
    let mut freed = 0;
    for (_, size, path) in books {
        if total <= limit {
            break;
        }
        if fs::remove_dir_all(&path).is_ok() {
            total -= size;
            freed += size;
        }
    }
    freed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use crate::*;
    use libreri_core::BookQuery;

    fn fixture(name: &str) -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../libreri-formats/tests/fixtures")
            .join(name)
    }

    #[test]
    fn serves_comic_pages_from_zip_and_rar() {
        let (dir, lib) = library();
        fs::copy(
            fixture("sample.cbr"),
            lib.layout().books_dir().join("owl.cbr"),
        )
        .unwrap();
        lib.scan(&NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap().remove(0);
        assert_eq!(book.metadata.title, "Night Owl #4");
        assert!(book.has_cover);

        let cache = dir.path().join("cache");
        let pb = lib.open_pages(&book.id, &cache).unwrap();
        assert_eq!(
            (pb.kind, pb.pages, pb.right_to_left),
            (PageKind::Comic, 3, true)
        );
        let (bytes, mime) = lib.page_image(&book.id, 3, 1000, &cache).unwrap();
        assert_eq!(mime, "image/png");
        assert!(bytes.starts_with(b"\x89PNG"));
        assert!(lib.page_image(&book.id, 4, 1000, &cache).is_err());
        assert!(page_cache_size(&cache) > 0);
        assert!(prune_page_cache(&cache, 0) > 0);
        assert_eq!(page_cache_size(&cache), 0);
        // Still works after the cache is cleared.
        assert!(lib.page_image(&book.id, 1, 1000, &cache).is_ok());
    }

    #[test]
    fn renders_djvu_pages_and_words() {
        if !libreri_helpers::status(libreri_helpers::Helper::DjVuLibre).installed {
            eprintln!("DjVuLibre is not installed; skipping");
            return;
        }
        let (dir, lib) = library();
        fs::copy(
            fixture("sample.djvu"),
            lib.layout().books_dir().join("optics.djvu"),
        )
        .unwrap();
        lib.scan(&NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap().remove(0);
        assert_eq!(book.metadata.title, "Optics of DjVu");
        assert!(book.has_cover);
        let cache = dir.path().join("cache");
        let pb = lib.open_pages(&book.id, &cache).unwrap();
        assert_eq!((pb.kind, pb.pages, pb.has_text), (PageKind::Djvu, 2, true));
        assert_eq!(pb.outline.len(), 2);
        let (jpg, mime) = lib.page_image(&book.id, 1, 700, &cache).unwrap();
        assert_eq!(mime, "image/jpeg");
        assert!(jpg.starts_with(&[0xFF, 0xD8]));
        assert!(cache
            .join(book.id.to_string())
            .join("djvu-1-800.jpg")
            .is_file());
        assert_eq!(lib.page_words(&book.id, 1).unwrap()[2].text, "light");
        let texts = lib.page_texts(&book.id, &cache).unwrap();
        assert_eq!(texts.len(), 2);
        assert!(texts[1].contains("Straße"));
    }
}
