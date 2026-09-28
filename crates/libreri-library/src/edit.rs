//! Changing a PDF's file (Phase 6b): page edits, redaction, corrections,
//! filled-in forms and markup saved as PDF annotations. Every change is
//! saved as a new version; the old file is kept (see `versions.rs`).

use crate::versions::{page_map, NewVersion};
use crate::{Error, Library, Result};
use libreri_core::{AnnotationKind, Book, BookId, FileType};
use libreri_pdf_edit::{EditPlan, EditReport, OcrWords, OutPage, PdfAnnot};
use std::fs;
use std::path::{Path, PathBuf};

fn pdf_err(e: libreri_pdf_edit::Error) -> Error {
    Error::InvalidInput(e.to_string())
}

/// True when the plan leaves the first page looking as it did.
fn first_page_same(plan: &EditPlan) -> bool {
    matches!(
        plan.pages.first(),
        Some(OutPage::Page {
            page: 1,
            rotate: 0,
            crop: None
        })
    ) && !plan.redactions.iter().any(|r| r.page == 1)
        && !plan.corrections.iter().any(|c| c.page == 1)
}

impl Library {
    /// A scratch file inside the library (same disk as the books, so the
    /// finished file can be moved into place).
    pub fn scratch_file(&self, ext: &str) -> Result<PathBuf> {
        let dir = self.layout().data_dir().join("tmp");
        fs::create_dir_all(&dir)?;
        Ok(dir.join(format!("{}.{ext}", uuid::Uuid::new_v4())))
    }

    fn editable_pdf(&self, id: &BookId) -> Result<(Book, PathBuf)> {
        self.require_edit()?;
        let book = self.book(id)?;
        if book.file_type != FileType::Pdf {
            return Err(Error::InvalidInput("only PDFs can be edited".into()));
        }
        let path = self
            .layout()
            .resolve_relative(&book.rel_path)
            .filter(|p| p.is_file())
            .ok_or(Error::BookNotFound)?;
        Ok((book, path))
    }

    fn ocr_words_for(&self, id: &BookId) -> Result<Vec<OcrWords>> {
        Ok(self
            .ocr_text(id)?
            .map(|t| {
                t.pages
                    .iter()
                    .filter(|p| !p.words.is_empty())
                    .map(|p| OcrWords {
                        page: p.page,
                        words: p
                            .words
                            .iter()
                            .map(|w| (w.text.clone(), w.rect.map(f64::from)))
                            .collect(),
                    })
                    .collect()
            })
            .unwrap_or_default())
    }

    /// Writes the edited PDF to `dest` without changing the book (for
    /// "Save as new book" and extracting pages).
    pub fn write_edited(&self, id: &BookId, plan: &EditPlan, dest: &Path) -> Result<EditReport> {
        let book = self.book(id)?;
        if book.file_type != FileType::Pdf {
            return Err(Error::InvalidInput("only PDFs can be edited".into()));
        }
        let path = self
            .layout()
            .resolve_relative(&book.rel_path)
            .ok_or(Error::BookNotFound)?;
        let ocr = if plan.embed_ocr {
            self.ocr_words_for(&book.id)?
        } else {
            Vec::new()
        };
        libreri_pdf_edit::apply(&path, plan, &ocr, dest).map_err(pdf_err)
    }

    /// Applies page edits to the book itself; the old file becomes a
    /// version. Notes, markup, positions and OCR text follow their pages.
    pub fn edit_pages(&self, id: &BookId, plan: &EditPlan) -> Result<(Book, EditReport)> {
        let (book, path) = self.editable_pdf(id)?;
        let ocr = if plan.embed_ocr {
            self.ocr_words_for(&book.id)?
        } else {
            Vec::new()
        };
        let tmp = self.scratch_file("pdf")?;
        let report = libreri_pdf_edit::apply(&path, plan, &ocr, &tmp);
        let report = match report {
            Ok(r) => r,
            Err(e) => {
                let _ = fs::remove_file(&tmp);
                return Err(pdf_err(e));
            }
        };
        // Redacted words must not stay findable in saved OCR text.
        if !plan.redactions.is_empty() {
            self.scrub_ocr(&book.id, plan)?;
        }
        let reason = if !plan.redactions.is_empty() {
            "Redacted"
        } else if plan.compress.is_some() && plan.pages.len() == report.pages as usize {
            "Made smaller"
        } else {
            "Edited pages"
        };
        let identity = report
            .page_map
            .iter()
            .all(|m| m.from == Some(m.to) && m.rotate == 0 && m.crop.is_none())
            && report.page_map.len() == libreri_pdf_edit::page_sizes(&path).map_or(0, |s| s.len());
        let saved = self.save_version(
            &book.id,
            NewVersion {
                file: &tmp,
                reason,
                pages: (!identity).then(|| page_map(&report.page_map)),
                cover_changed: !first_page_same(plan) || !report.flattened.is_empty(),
            },
        );
        if saved.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        Ok((saved?, report))
    }

    /// Removes OCR words under redaction boxes.
    fn scrub_ocr(&self, id: &BookId, plan: &EditPlan) -> Result<()> {
        let Some(mut ocr) = self.ocr_text(id)? else {
            return Ok(());
        };
        for page in &mut ocr.pages {
            let boxes: Vec<[f64; 4]> = plan
                .redactions
                .iter()
                .filter(|r| r.page == page.page)
                .flat_map(|r| r.boxes.iter().copied())
                .collect();
            if boxes.is_empty() {
                continue;
            }
            let before = page.words.len();
            page.words.retain(|w| {
                let [x, y, ww, hh] = w.rect.map(f64::from);
                let (cx, cy) = (x + ww / 2.0, y + hh / 2.0);
                !boxes.iter().any(|[bx, by, bw, bh]| {
                    cx >= *bx && cx <= bx + bw && cy >= *by && cy <= by + bh
                })
            });
            if page.words.len() != before {
                page.text = page
                    .words
                    .iter()
                    .map(|w| w.text.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
            }
        }
        self.save_ocr(id, &ocr)
    }

    /// Saves a PDF whose form fields were filled in (bytes from the reader)
    /// as the book's new version.
    pub fn save_filled_form(&self, id: &BookId, bytes: &[u8]) -> Result<Book> {
        let (book, _) = self.editable_pdf(id)?;
        if !bytes.starts_with(b"%PDF") {
            return Err(Error::InvalidInput(
                "the filled-in form is not a PDF".into(),
            ));
        }
        let tmp = self.scratch_file("pdf")?;
        fs::write(&tmp, bytes)?;
        let saved = self.save_version(
            &book.id,
            NewVersion {
                file: &tmp,
                reason: "Filled in form",
                pages: None,
                cover_changed: true,
            },
        );
        if saved.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        saved
    }

    /// Writes markup into the PDF as standard annotations (a new version).
    /// The marks written are then removed from Libreri's own markup, so
    /// they are not shown twice.
    pub fn save_markup_into_pdf(&self, id: &BookId, annots: &[PdfAnnot]) -> Result<Book> {
        let (book, path) = self.editable_pdf(id)?;
        let tmp = self.scratch_file("pdf")?;
        if let Err(e) = libreri_pdf_edit::annotate(&path, annots, &tmp) {
            let _ = fs::remove_file(&tmp);
            return Err(pdf_err(e));
        }
        let saved = self.save_version(
            &book.id,
            NewVersion {
                file: &tmp,
                reason: "Saved markup into the PDF",
                pages: None,
                cover_changed: annots.iter().any(|a| a.page == 1),
            },
        );
        if saved.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        let saved = saved?;
        for a in self.annotations(&saved.id)? {
            if a.kind == AnnotationKind::Markup && annots.iter().any(|p| p.id == a.id) {
                self.delete_annotation(&a.id)?;
            }
        }
        Ok(saved)
    }

    /// Writes a copy of an earlier version to `dest`.
    pub fn copy_version(&self, id: &BookId, version: &BookId, dest: &Path) -> Result<()> {
        let src = self.version_path(id, version)?;
        fs::copy(src, dest)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::library;
    use crate::NoProgress;
    use libreri_core::BookQuery;
    use libreri_pdf_edit::Redaction;

    #[test]
    fn redacting_scrubs_saved_ocr_and_keeps_a_version() {
        let (_dir, lib) = library();
        libreri_formats::test_text_pdf(
            &lib.layout().books_dir().join("memo.pdf"),
            &["The lighthouse keeper wrote every night", "two"],
        );
        lib.scan(&NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap().remove(0);
        // Pretend OCR found the words too.
        let ocr = crate::OcrText {
            format_version: 1,
            engine: "test".into(),
            languages: vec!["eng".into()],
            updated_at: String::new(),
            pages: vec![libreri_formats::ocr::OcrPage {
                page: 1,
                text: "The lighthouse keeper".into(),
                words: vec![
                    libreri_formats::ocr::OcrWord {
                        text: "The".into(),
                        rect: [0.12, 0.1, 0.03, 0.015],
                    },
                    libreri_formats::ocr::OcrWord {
                        text: "lighthouse".into(),
                        rect: [0.16, 0.1, 0.08, 0.015],
                    },
                ],
                confidence: 90.0,
            }],
        };
        lib.save_ocr(&book.id, &ocr).unwrap();
        let plan = EditPlan {
            pages: vec![
                OutPage::Page {
                    page: 1,
                    rotate: 0,
                    crop: None,
                },
                OutPage::Page {
                    page: 2,
                    rotate: 0,
                    crop: None,
                },
            ],
            redactions: vec![Redaction {
                page: 1,
                boxes: vec![[0.155, 0.095, 0.09, 0.03]],
                color: "#000000".into(),
            }],
            ..Default::default()
        };
        let (new, report) = lib.edit_pages(&book.id, &plan).unwrap();
        assert!(report.flattened.is_empty());
        let words: Vec<String> = lib.ocr_text(&new.id).unwrap().unwrap().pages[0]
            .words
            .iter()
            .map(|w| w.text.clone())
            .collect();
        assert_eq!(words, ["The"]);
        let versions = lib.versions(&new.id).unwrap();
        assert_eq!(versions[0].reason, "Redacted");
        let path = lib.layout().resolve_relative(&new.rel_path).unwrap();
        let text = libreri_formats::pdftext::page_texts(&path).unwrap();
        assert!(!text[0].contains("lighthouse"));
        // The scratch folder is left empty.
        let tmp = lib.layout().data_dir().join("tmp");
        assert_eq!(fs::read_dir(tmp).unwrap().count(), 0);
    }
}
