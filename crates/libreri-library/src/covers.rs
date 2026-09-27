//! Stored covers: `.library-data/covers/<id>.jpg` and
//! `.library-data/thumbnails/<id>.jpg`.

use crate::paths::write_atomic;
use crate::{Library, Result};
use libreri_core::{BookId, LibraryLayout};
use std::path::PathBuf;

pub fn cover_path(layout: &LibraryLayout, id: &BookId) -> PathBuf {
    layout.data_dir().join("covers").join(format!("{id}.jpg"))
}

pub fn thumbnail_path(layout: &LibraryLayout, id: &BookId) -> PathBuf {
    layout
        .data_dir()
        .join("thumbnails")
        .join(format!("{id}.jpg"))
}

/// Library-relative path of a thumbnail, for the `book://` protocol.
pub fn thumbnail_rel(id: &BookId) -> String {
    format!("{}/thumbnails/{id}.jpg", libreri_core::layout::DATA_DIR)
}

/// Library-relative path of a cover, for the `book://` protocol.
pub fn cover_rel(id: &BookId) -> String {
    format!("{}/covers/{id}.jpg", libreri_core::layout::DATA_DIR)
}

/// Makes and writes the cover and thumbnail. Returns an error if `bytes`
/// is not an image.
pub fn store(layout: &LibraryLayout, id: &BookId, bytes: &[u8]) -> Result<()> {
    let covers = libreri_thumbs::make_covers(bytes)
        .map_err(|e| crate::Error::InvalidInput(e.to_string()))?;
    write_atomic(&cover_path(layout, id), &covers.cover)?;
    write_atomic(&thumbnail_path(layout, id), &covers.thumbnail)?;
    Ok(())
}

pub fn exists(layout: &LibraryLayout, id: &BookId) -> bool {
    thumbnail_path(layout, id).is_file()
}

pub fn rename(layout: &LibraryLayout, old: &BookId, new: &BookId) {
    let _ = std::fs::rename(cover_path(layout, old), cover_path(layout, new));
    let _ = std::fs::rename(thumbnail_path(layout, old), thumbnail_path(layout, new));
}

impl Library {
    /// Replaces a book's cover with `bytes` (any common image format). Used
    /// for PDF first pages rendered by the interface, and later for covers
    /// found online.
    pub fn set_cover(&self, id: &BookId, bytes: &[u8]) -> Result<()> {
        if self.with_db(|db| db.resolve_book_id(id))?.as_ref() != Some(id) {
            return Err(crate::Error::BookNotFound);
        }
        store(self.layout(), id, bytes)?;
        self.with_db(|db| db.set_has_cover(id, true, &crate::now()))
    }

    /// Once per library: forgets PDF covers that came out as a plain page.
    /// Before PDF.js had its image decoders, scanned PDFs (JPEG 2000 and
    /// JBIG2 pages) gave empty covers; without one, the interface draws the
    /// first page again.
    pub(crate) fn forget_blank_pdf_covers(&self) -> Result<usize> {
        const DONE: &str = "covers:blank-pdf-check:1";
        if self.with_db(|db| db.meta_get(DONE))?.as_deref() == Some("1") {
            return Ok(0);
        }
        let mut n = 0;
        for rec in self.with_db(|db| db.file_records())? {
            if !rec.rel_path.to_ascii_lowercase().ends_with(".pdf") {
                continue;
            }
            let Ok(bytes) = std::fs::read(thumbnail_path(self.layout(), &rec.id)) else {
                continue;
            };
            if libreri_thumbs::is_blank(&bytes) {
                let _ = std::fs::remove_file(thumbnail_path(self.layout(), &rec.id));
                let _ = std::fs::remove_file(cover_path(self.layout(), &rec.id));
                self.with_db(|db| db.set_has_cover(&rec.id, false, &crate::now()))?;
                n += 1;
            }
        }
        self.with_db(|db| db.meta_set(DONE, "1"))?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::BookQuery;

    fn png(colour: [u8; 3], line: bool) -> Vec<u8> {
        let mut img = image::RgbImage::from_pixel(300, 450, image::Rgb(colour));
        if line {
            for x in 20..280 {
                img.put_pixel(x, 100, image::Rgb([0, 0, 0]));
            }
        }
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn blank_pdf_covers_are_forgotten_once() {
        let (_d, lib) = library();
        let books = lib.layout().books_dir();
        std::fs::write(books.join("scan.pdf"), b"%PDF-1.4 not really").unwrap();
        std::fs::write(books.join("text.pdf"), b"%PDF-1.4 also not").unwrap();
        lib.scan(&NoProgress).unwrap();
        let all = lib.books(&BookQuery::default()).unwrap();
        let id = |name: &str| {
            all.iter()
                .find(|b| b.rel_path.ends_with(name))
                .unwrap()
                .id
                .clone()
        };
        lib.set_cover(&id("scan.pdf"), &png([255, 255, 255], false))
            .unwrap();
        lib.set_cover(&id("text.pdf"), &png([255, 255, 255], true))
            .unwrap();
        // Pretend the library was made before the check existed.
        lib.with_db(|db| db.meta_set("covers:blank-pdf-check:1", "0"))
            .unwrap();
        let root = lib.layout().root().to_path_buf();
        lib.close().unwrap();

        let lib = Library::open(&root, OpenOptions::default()).unwrap();
        assert!(!lib.book(&id("scan.pdf")).unwrap().has_cover);
        assert!(lib.book(&id("text.pdf")).unwrap().has_cover);

        // A later blank cover is kept: the check runs once per library.
        lib.set_cover(&id("scan.pdf"), &png([255, 255, 255], false))
            .unwrap();
        lib.close().unwrap();
        let lib = Library::open(&root, OpenOptions::default()).unwrap();
        assert!(lib.book(&id("scan.pdf")).unwrap().has_cover);
    }
}
