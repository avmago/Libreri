//! Writing PDFs.
//!
//! Phase 6a: **marked-up copies**. The reader turns each page's markup into
//! a small drawing list (filled and stroked paths, and pictures for text,
//! stamps and signatures) in page fractions; [`mark_up`] draws that list on
//! top of each page of a PDF and saves a new file. The original is never
//! changed. For DjVu and comics, [`pages_to_pdf`] first makes a PDF of the
//! page images.
//!
//! Page edits, redaction and versions come in Phase 6b.

mod draw;
mod path;

pub use draw::{mark_up, pages_to_pdf, DrawOp, DrawPage, PageImage};

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
