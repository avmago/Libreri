//! Writing PDFs.
//!
//! **Marked-up copies.** The reader turns each page's markup into
//! a small drawing list (filled and stroked paths, and pictures for text,
//! stamps and signatures) in page fractions; [`mark_up`] draws that list on
//! top of each page of a PDF and saves a new file. The original is never
//! changed. For DjVu and comics, [`pages_to_pdf`] first makes a PDF of the
//! page images.
//!
//! **Page edits:** [`apply`] edits a PDF's pages (order, turning, cropping,
//! pages from other files, blank and photographed pages), removes what
//! lies under redaction boxes (text, pictures, shapes and annotations; a
//! page that cannot be cleaned exactly becomes a picture), writes small
//! text corrections, embeds OCR text as an invisible layer and shrinks
//! pictures. [`annotate`] saves markup as standard PDF annotations. The
//! caller keeps the old file as a version.
//!
//! **Comparing:** [`compare_words`] pairs the pages of two documents and finds
//! words removed, added or changed; [`look_changes`] finds where pages look
//! different; [`compare_report`] writes the report PDF.

mod annotate;
mod compare;
mod draw;
mod edit;
mod fonts;
mod geom;
mod inspect;
mod path;
mod redact;
mod report;
mod std_fonts;

pub use annotate::{annotate, PdfAnnot, PdfAnnotKind};
pub use compare::{
    compare_words, line_rects, look_changes, pair_pages, Change, ChangeKind, CmpWord, Grey,
    PagePair,
};
pub use draw::{mark_up, pages_to_pdf, DrawOp, DrawPage, PageImage};
pub use edit::{
    apply, pictures_pdf, Correction, EditPlan, EditReport, OcrWords, OutPage, PageMove, Quality,
    Redaction,
};
pub use report::{compare_report, ReportInfo, ReportPicture};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the PDF could not be read: {0}")]
    Read(String),
    #[error("the PDF is password-protected; remove the password first")]
    Encrypted,
    #[error("page {0} does not exist")]
    NoPage(u32),
    #[error("a picture could not be read: {0}")]
    Picture(String),
    #[error("the drawing is not valid: {0}")]
    Path(String),
    #[error("the file could not be written: {0}")]
    Write(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Every page's size as shown (points, turning applied), in page order.
pub fn page_sizes(path: &std::path::Path) -> Result<Vec<(f64, f64)>> {
    let doc = lopdf::Document::load(path).map_err(|e| Error::Read(e.to_string()))?;
    Ok(doc
        .get_pages()
        .values()
        .map(|id| geom::Geometry::of(&doc, *id).shown())
        .collect())
}
