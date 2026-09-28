//! Comparing two documents (Phase 6c): versions of a book, two books, or a
//! book and a file on this computer. PDFs and DjVu scans; pages without
//! text use saved OCR text when there is some.

use crate::{Error, Library, Progress, Result};
use libreri_core::{BookId, FileType};
use libreri_formats::djvu;
use libreri_formats::pdftext::PdfDoc;
use libreri_pdf_edit::{Change, ChangeKind, CmpWord, Grey, PagePair};
use std::path::{Path, PathBuf};

/// What to compare.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompareSource {
    /// A book as it is now.
    Book(BookId),
    /// An earlier version of a book.
    Version { book: BookId, version: BookId },
    /// A PDF or DjVu file on this computer.
    File(PathBuf),
}

/// One side of a comparison, opened.
pub struct CompareDoc {
    pub label: String,
    pub kind: FileType,
    pub path: PathBuf,
    /// The book the file is (or was) for its saved OCR text.
    book: Option<BookId>,
    pdf: Option<PdfDoc>,
    pages: u32,
}

/// The result.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    pub a_pages: u32,
    pub b_pages: u32,
    pub pairs: Vec<PagePair>,
    pub changes: Vec<Change>,
}

/// Width of the renderings compared for looks.
const LOOK_WIDTH: u32 = 480;

impl CompareDoc {
    fn open(label: String, path: PathBuf, kind: FileType, book: Option<BookId>) -> Result<Self> {
        let bad = |e: String| Error::InvalidInput(e);
        match kind {
            FileType::Pdf => {
                let pdf = PdfDoc::open(&path).map_err(bad)?;
                let pages = pdf.page_count();
                Ok(Self {
                    label,
                    kind,
                    path,
                    book,
                    pdf: Some(pdf),
                    pages,
                })
            }
            FileType::Djvu => {
                let pages = djvu::info(&path).map_err(bad)?.sizes.len() as u32;
                Ok(Self {
                    label,
                    kind,
                    path,
                    book,
                    pdf: None,
                    pages,
                })
            }
            _ => Err(Error::InvalidInput(
                "only PDF and DjVu files can be compared".into(),
            )),
        }
    }

    pub fn pages(&self) -> u32 {
        self.pages
    }

    /// A page `width` pixels wide as RGB pixels, width and height.
    pub fn render(&self, page: u32, width: u32) -> Result<(Vec<u8>, u32, u32)> {
        let bad = |e: String| Error::InvalidInput(e);
        if let Some(pdf) = &self.pdf {
            return pdf.render_rgb(page, width).map_err(bad);
        }
        let pnm = djvu::render_page(&self.path, page, width).map_err(bad)?;
        let img = image::load_from_memory(&pnm)
            .map_err(|e| Error::InvalidInput(e.to_string()))?
            .to_rgb8();
        let (w, h) = img.dimensions();
        Ok((img.into_raw(), w, h))
    }

    /// A page as a JPEG, for the screen and the report.
    pub fn jpeg(&self, page: u32, width: u32) -> Result<(Vec<u8>, u32, u32)> {
        let (rgb, w, h) = self.render(page, width)?;
        let img = image::RgbImage::from_raw(w, h, rgb)
            .ok_or_else(|| Error::InvalidInput("the page could not be drawn".into()))?;
        let mut out = std::io::Cursor::new(Vec::new());
        image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 82)
            .encode_image(&img)
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        Ok((out.into_inner(), w, h))
    }

    fn words(
        &self,
        lib: &Library,
        progress: &dyn Progress,
        step: (u64, u64),
    ) -> Result<Vec<Vec<CmpWord>>> {
        let convert = |ws: Vec<djvu::Word>| -> Vec<CmpWord> {
            ws.into_iter()
                .map(|w| CmpWord {
                    text: w.text,
                    rect: w.rect,
                })
                .collect()
        };
        let mut pages: Vec<Vec<CmpWord>> = match &self.pdf {
            Some(pdf) => {
                let mut out = Vec::with_capacity(self.pages as usize);
                for p in 1..=self.pages {
                    if progress.cancelled() {
                        return Err(Error::Cancelled);
                    }
                    progress.report(
                        step.0 + u64::from(p),
                        step.1,
                        &format!("Reading {}", self.label),
                    );
                    out.push(convert(pdf.words(p)));
                }
                out
            }
            None => djvu::all_page_words(&self.path, self.pages as usize)
                .map_err(Error::InvalidInput)?
                .into_iter()
                .map(convert)
                .collect(),
        };
        // Scans: the words OCR found.
        if let Some(book) = &self.book {
            if let Some(ocr) = lib.ocr_cached(book) {
                for p in &ocr.pages {
                    if let Some(slot) = pages.get_mut(p.page as usize - 1) {
                        if slot.is_empty() {
                            *slot = p
                                .words
                                .iter()
                                .map(|w| CmpWord {
                                    text: w.text.clone(),
                                    rect: w.rect.map(f64::from),
                                })
                                .collect();
                        }
                    }
                }
            }
        }
        Ok(pages)
    }
}

fn grey(rgb: &[u8]) -> Vec<u8> {
    rgb.chunks_exact(3)
        .map(|p| {
            ((u32::from(p[0]) * 299 + u32::from(p[1]) * 587 + u32::from(p[2]) * 114) / 1000) as u8
        })
        .collect()
}

impl Library {
    /// Opens one side of a comparison.
    pub fn compare_doc(&self, source: &CompareSource) -> Result<CompareDoc> {
        match source {
            CompareSource::Book(id) => {
                let book = self.book(id)?;
                let path = self
                    .layout()
                    .resolve_relative(&book.rel_path)
                    .filter(|p| p.is_file())
                    .ok_or(Error::BookNotFound)?;
                CompareDoc::open(
                    book.metadata.title.clone(),
                    path,
                    book.file_type,
                    Some(book.id),
                )
            }
            CompareSource::Version { book, version } => {
                let current = self.book(book)?;
                let path = self.version_path(book, version)?;
                let when = self
                    .versions(book)?
                    .into_iter()
                    .find(|v| &v.id == version)
                    .map(|v| v.saved_at.chars().take(10).collect::<String>())
                    .unwrap_or_default();
                // Earlier files had other ids: their OCR text is not kept.
                CompareDoc::open(
                    format!("{} (version of {when})", current.metadata.title),
                    path,
                    current.file_type,
                    None,
                )
            }
            CompareSource::File(path) => {
                let kind = FileType::from_path(path).ok_or_else(|| {
                    Error::InvalidInput("only PDF and DjVu files can be compared".into())
                })?;
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                CompareDoc::open(name, path.clone(), kind, None)
            }
        }
    }

    /// Compares two opened documents.
    pub fn compare(
        &self,
        a: &CompareDoc,
        b: &CompareDoc,
        progress: &dyn Progress,
    ) -> Result<Comparison> {
        let total = u64::from(a.pages + b.pages) * 2;
        let aw = a.words(self, progress, (0, total))?;
        let bw = b.words(self, progress, (u64::from(a.pages), total))?;
        let (pairs, mut changes) = libreri_pdf_edit::compare_words(&aw, &bw);
        let base = u64::from(a.pages + b.pages);
        let both: Vec<(usize, u32, u32)> = pairs
            .iter()
            .enumerate()
            .filter_map(|(i, p)| Some((i, p.a?, p.b?)))
            .collect();
        for (n, (i, pa, pb)) in both.iter().copied().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            progress.report(
                base + (n as u64 * (total - base)) / both.len().max(1) as u64,
                total,
                "Comparing how the pages look",
            );
            let (Ok((ra, wa, ha)), Ok((rb, wb, hb))) =
                (a.render(pa, LOOK_WIDTH), b.render(pb, LOOK_WIDTH))
            else {
                continue;
            };
            let (ga, gb) = (grey(&ra), grey(&rb));
            let ignore: Vec<[f64; 4]> = aw[pa as usize - 1]
                .iter()
                .chain(&bw[pb as usize - 1])
                .map(|w| w.rect)
                .collect();
            let regions = libreri_pdf_edit::look_changes(
                &Grey {
                    pixels: &ga,
                    width: wa,
                    height: ha,
                },
                &Grey {
                    pixels: &gb,
                    width: wb,
                    height: hb,
                },
                &ignore,
            );
            if !regions.is_empty() {
                changes.push(Change {
                    kind: ChangeKind::Look,
                    pair: i as u32,
                    a_rects: regions.clone(),
                    b_rects: regions,
                    a_text: String::new(),
                    b_text: String::new(),
                });
            }
        }
        changes.sort_by_key(|c| c.pair);
        progress.report(total, total, "Done");
        Ok(Comparison {
            a_pages: a.pages,
            b_pages: b.pages,
            pairs,
            changes,
        })
    }

    /// Writes the comparison report PDF.
    pub fn compare_report(
        &self,
        a: &CompareDoc,
        b: &CompareDoc,
        result: &Comparison,
        made: &str,
        dest: &Path,
    ) -> Result<()> {
        let info = libreri_pdf_edit::ReportInfo {
            title: &format!("Comparison: {}", a.label),
            a_label: &a.label,
            b_label: &b.label,
            made,
        };
        libreri_pdf_edit::compare_report(
            &info,
            &result.pairs,
            &result.changes,
            |side, page| {
                let doc = if side == 0 { a } else { b };
                doc.jpeg(page, 1000).ok().map(|(jpeg, width, height)| {
                    libreri_pdf_edit::ReportPicture {
                        jpeg,
                        width,
                        height,
                    }
                })
            },
            dest,
        )
        .map_err(|e| Error::InvalidInput(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::library;
    use crate::versions::NewVersion;
    use crate::NoProgress;
    use libreri_core::BookQuery;

    #[test]
    fn compares_a_version_with_the_book() {
        let (dir, lib) = library();
        libreri_formats::test_text_pdf(
            &lib.layout().books_dir().join("log.pdf"),
            &[
                "The keeper Samuel Harte wrote at dusk",
                "Storms of the year",
            ],
        );
        lib.scan(&NoProgress).unwrap();
        let old = lib.books(&BookQuery::default()).unwrap().remove(0);
        let edited = dir.path().join("e.pdf");
        libreri_formats::test_text_pdf(
            &edited,
            &[
                "The keeper Ada Harte wrote at dusk",
                "A new page",
                "Storms of the year",
            ],
        );
        let new = lib
            .save_version(
                &old.id,
                NewVersion {
                    file: &edited,
                    reason: "Edited pages",
                    pages: None,
                    cover_changed: false,
                },
            )
            .unwrap();
        let a = lib
            .compare_doc(&CompareSource::Version {
                book: new.id.clone(),
                version: old.id.clone(),
            })
            .unwrap();
        let b = lib
            .compare_doc(&CompareSource::Book(new.id.clone()))
            .unwrap();
        assert!(a.label.contains("version of"));
        let r = lib.compare(&a, &b, &NoProgress).unwrap();
        assert_eq!((r.a_pages, r.b_pages), (2, 3));
        let kinds: Vec<ChangeKind> = r.changes.iter().map(|c| c.kind).collect();
        assert_eq!(
            kinds,
            [ChangeKind::Changed, ChangeKind::PageAdded],
            "{:?}",
            r.changes
        );
        assert_eq!(
            (r.changes[0].a_text.as_str(), r.changes[0].b_text.as_str()),
            ("Samuel", "Ada")
        );
        let report = dir.path().join("report.pdf");
        lib.compare_report(&a, &b, &r, "today", &report).unwrap();
        let texts = libreri_formats::pdftext::page_texts(&report).unwrap();
        assert_eq!(
            texts.len(),
            3,
            "summary, the changed page and the added page"
        );
    }

    #[test]
    fn finds_a_drawing_that_changed() {
        let dir = tempfile::tempdir().unwrap();
        let (a, b) = (dir.path().join("a.pdf"), dir.path().join("b.pdf"));
        libreri_formats::test_text_pdf(&a, &["Plan of the lighthouse"]);
        libreri_formats::test_text_pdf(&b, &["Plan of the lighthouse"]);
        // The second has a box drawn in the lower half.
        let mut doc = lopdf::Document::load(&b).unwrap();
        let page = *doc.get_pages().get(&1).unwrap();
        let extra = doc.add_object(lopdf::Stream::new(
            lopdf::Dictionary::new(),
            b"0 0 1 rg 100 100 200 150 re f".to_vec(),
        ));
        let d = doc.get_dictionary_mut(page).unwrap();
        let first = d.get(b"Contents").unwrap().clone();
        d.set("Contents", vec![first, lopdf::Object::Reference(extra)]);
        doc.save(&b).unwrap();
        let (_d, lib) = library();
        let da = lib.compare_doc(&CompareSource::File(a)).unwrap();
        let db = lib.compare_doc(&CompareSource::File(b)).unwrap();
        let r = lib.compare(&da, &db, &NoProgress).unwrap();
        assert_eq!(r.changes.len(), 1, "{:?}", r.changes);
        assert_eq!(r.changes[0].kind, ChangeKind::Look);
        let [x, y, w, h] = r.changes[0].b_rects[0];
        // 100..300 of 612 across, 542..692 of 792 down.
        assert!(
            x < 0.17 && x + w > 0.48 && y < 0.69 && y + h > 0.86,
            "{:?}",
            r.changes[0].b_rects
        );
    }

    /// Compares two files and writes the result as JSON, for trying the
    /// interface: `LIBRERI_CMP_A=a.pdf LIBRERI_CMP_B=b.pdf LIBRERI_CMP_OUT=r.json cargo test compare::tests::look`.
    #[test]
    fn look() {
        let (Some(a), Some(b), Some(out)) = (
            std::env::var_os("LIBRERI_CMP_A"),
            std::env::var_os("LIBRERI_CMP_B"),
            std::env::var_os("LIBRERI_CMP_OUT"),
        ) else {
            return;
        };
        let (_d, lib) = library();
        let da = lib.compare_doc(&CompareSource::File(a.into())).unwrap();
        let db = lib.compare_doc(&CompareSource::File(b.into())).unwrap();
        let r = lib.compare(&da, &db, &NoProgress).unwrap();
        let json = serde_json::json!({ "pairs": r.pairs, "changes": r.changes, "aPages": r.a_pages, "bPages": r.b_pages });
        std::fs::write(&out, json.to_string()).unwrap();
        lib.compare_report(
            &da,
            &db,
            &r,
            "test",
            &std::path::Path::new(&out).with_extension("pdf"),
        )
        .unwrap();
    }
}
