//! Captured paper notes (Phase 8b): photographed pages, straightened and
//! cleaned, saved as one searchable PDF in `Notes/<profile>/Captures/` and
//! linked from a place in a book by a "capture" annotation.

use crate::{Error, Library, Result};
use std::fs;
use std::path::PathBuf;

pub const CAPTURE_DIR: &str = "Captures";

impl Library {
    /// Saves captured pages as a PDF (pages are JPEG or PNG; `ocr` are
    /// words found on them). Returns its path in the library.
    pub fn save_capture(
        &self,
        title: &str,
        pages: Vec<Vec<u8>>,
        ocr: &[libreri_pdf_edit::OcrWords],
    ) -> Result<String> {
        if pages.is_empty() {
            return Err(Error::InvalidInput("add a page first".into()));
        }
        let dir = self.notes_folder()?.join(CAPTURE_DIR);
        fs::create_dir_all(&dir)?;
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        let title = if title.is_empty() {
            chrono::Local::now()
                .format("Notes %Y-%m-%d %H-%M")
                .to_string()
        } else {
            title
        };
        let file = crate::paths::unique_path(
            &dir,
            &format!("{}.pdf", crate::reading::notes_folder_name(&title)),
        );
        libreri_pdf_edit::pictures_pdf(pages, ocr, &file)
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        crate::paths::rel_of(self.layout(), &file).ok_or(Error::BookNotFound)
    }

    /// The file of one of the signed-in profile's captures.
    pub fn capture_file(&self, rel: &str) -> Result<PathBuf> {
        let dir = self.own_notes_dir()?;
        self.layout()
            .resolve_relative(rel)
            .filter(|p| p.starts_with(&dir) && p.extension().is_some_and(|e| e == "pdf"))
            .ok_or_else(|| Error::NotAllowed("those pages belong to someone else".into()))
    }

    /// Moves a capture to the system trash.
    pub fn delete_capture(&self, rel: &str) -> Result<()> {
        let path = self.capture_file(rel)?;
        if !path.exists() {
            return Ok(());
        }
        // No fallback to a permanent delete: if there is no trash, say so.
        trash::delete(&path).map_err(|e| Error::Trash(e.to_string()))
    }
}

/// The capture named in an annotation's locator.
pub fn capture_of(locator: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(locator).ok()?;
    v.get("capture")?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use crate::testutil::library;

    #[test]
    fn keeps_captures_in_the_notes_folder() {
        let (_d, lib) = library();
        let mut jpeg = Vec::new();
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
            60,
            80,
            image::Rgb([255, 255, 255]),
        ))
        .write_to(
            &mut std::io::Cursor::new(&mut jpeg),
            image::ImageFormat::Jpeg,
        )
        .unwrap();
        let rel = lib.save_capture("Lecture 3", vec![jpeg], &[]).unwrap();
        assert!(rel.ends_with("/Captures/Lecture 3.pdf"), "{rel}");
        assert!(lib.capture_file(&rel).unwrap().is_file());
        assert!(lib.may_open(&rel));
        assert!(lib.save_capture("x", vec![], &[]).is_err());
        assert!(lib.capture_file("Books/a.pdf").is_err());
        lib.delete_capture(&rel).unwrap();
        assert!(!lib.capture_file(&rel).unwrap().exists());
    }
}
