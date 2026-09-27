//! Reading other apps' libraries: Calibre, Zotero, BibTeX and RIS files
//! (Mendeley, JabRef and others), and Goodreads and StoryGraph exports.
//!
//! Each reader returns [`ForeignBook`]s: tidied details, the files the app
//! knows for the book (best first), its cover, the reader's personal data
//! and, for Zotero, PDF highlights. Nothing here touches a Libreri library;
//! `libreri-library` decides what to copy and how to merge. Other apps'
//! folders are only ever read.

mod bib;
mod calibre;
#[doc(hidden)]
pub mod fixtures;
mod reading_log;
mod zotero;

use libreri_core::{BookMetadata, FileType, HighlightColor, ReadingStatus};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub use bib::{parse_bibtex, parse_ris};
pub use calibre::DEFAULT_FORMAT_ORDER;
#[doc(hidden)]
pub use fixtures::{calibre_fixture, zotero_fixture};

/// Where a library comes from.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ForeignSource {
    /// A Calibre library folder (with `metadata.db`).
    Calibre,
    /// Zotero's data folder (with `zotero.sqlite` and `storage/`).
    Zotero,
    /// A BibTeX file, such as a Mendeley or JabRef export.
    Bibtex,
    /// A RIS file (Mendeley, EndNote, Zotero).
    Ris,
    /// Goodreads "Export library" CSV.
    Goodreads,
    /// The StoryGraph export CSV.
    StoryGraph,
}

impl ForeignSource {
    pub fn label(self) -> &'static str {
        match self {
            Self::Calibre => "Calibre",
            Self::Zotero => "Zotero",
            Self::Bibtex => "BibTeX",
            Self::Ris => "RIS",
            Self::Goodreads => "Goodreads",
            Self::StoryGraph => "The StoryGraph",
        }
    }

    /// Reading logs only update books already in the library.
    pub fn is_reading_log(self) -> bool {
        matches!(self, Self::Goodreads | Self::StoryGraph)
    }
}

/// A file the other app has for a book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignFile {
    pub path: PathBuf,
    pub file_type: FileType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForeignMark {
    Highlight,
    /// A sticky note at a place on the page.
    Note,
}

/// A PDF highlight or note made in the other app.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignAnnotation {
    /// A UUID made from the other app's own id, so importing twice finds
    /// the same note instead of adding it again.
    pub id: String,
    /// The file it was made in.
    pub file: PathBuf,
    pub mark: ForeignMark,
    pub color: HighlightColor,
    pub text: Option<String>,
    pub comment: Option<String>,
    /// 0-based page.
    pub page_index: u32,
    /// The page's printed label ("xii", "4").
    pub page_label: Option<String>,
    /// Rectangles in PDF points: x1, y1, x2, y2 from the bottom left.
    pub rects: Vec<[f64; 4]>,
    pub created_at: String,
    pub modified_at: String,
}

/// The reader's own data about a book in the other app.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ForeignPersonal {
    pub status: Option<ReadingStatus>,
    /// 0 = none, 1–5 stars.
    pub rating: u8,
    pub favorite: bool,
    /// When last read, RFC 3339.
    pub last_read: Option<String>,
}

/// One book (or paper) in the other app.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignBook {
    /// The other app's id ("calibre:12", "zotero:1:ABCD2345").
    pub key: String,
    pub metadata: BookMetadata,
    /// Files Libreri can open, in the app's order (best first for Zotero).
    pub files: Vec<ForeignFile>,
    pub cover: Option<PathBuf>,
    pub personal: Option<ForeignPersonal>,
    pub annotations: Vec<ForeignAnnotation>,
    /// Notes written about the book (Zotero child notes), as plain text.
    pub notes: Vec<String>,
    pub added_at: Option<String>,
}

impl ForeignBook {
    fn new(key: String, metadata: BookMetadata) -> Self {
        Self {
            key,
            metadata,
            files: Vec::new(),
            cover: None,
            personal: None,
            annotations: Vec::new(),
            notes: Vec::new(),
            added_at: None,
        }
    }
}

/// Everything read from another app.
#[derive(Debug, Clone, PartialEq)]
pub struct ForeignLibrary {
    pub source: ForeignSource,
    pub books: Vec<ForeignBook>,
    pub warnings: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ForeignError {
    #[error("{0}")]
    NotRecognised(String),
    #[error("{0}")]
    Unreadable(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

impl From<rusqlite::Error> for ForeignError {
    fn from(e: rusqlite::Error) -> Self {
        Self::Unreadable(format!("the database could not be read: {e}"))
    }
}

/// Recognises a Calibre library, a Zotero data folder, or an export file.
pub fn detect(path: &Path) -> Option<ForeignSource> {
    if path.is_dir() {
        if path.join("metadata.db").is_file() {
            return Some(ForeignSource::Calibre);
        }
        if path.join("zotero.sqlite").is_file() {
            return Some(ForeignSource::Zotero);
        }
        return None;
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "bib" | "bibtex" => Some(ForeignSource::Bibtex),
        "ris" => Some(ForeignSource::Ris),
        "db" if path.file_name().is_some_and(|n| n == "metadata.db") => {
            Some(ForeignSource::Calibre)
        }
        "sqlite" if path.file_name().is_some_and(|n| n == "zotero.sqlite") => {
            Some(ForeignSource::Zotero)
        }
        "csv" => {
            let head = std::fs::read(path).ok()?;
            let head = String::from_utf8_lossy(&head[..head.len().min(4096)]).to_string();
            let first = head.lines().next().unwrap_or("");
            if first.contains("Exclusive Shelf") {
                Some(ForeignSource::Goodreads)
            } else if first.contains("Read Status") {
                Some(ForeignSource::StoryGraph)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Reads a library. `path` is the folder (Calibre, Zotero) or the file.
pub fn read(path: &Path, source: ForeignSource) -> Result<ForeignLibrary, ForeignError> {
    // A picked `metadata.db` or `zotero.sqlite` means its folder.
    let dir = if path.is_file() && matches!(source, ForeignSource::Calibre | ForeignSource::Zotero)
    {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let mut lib = match source {
        ForeignSource::Calibre => calibre::read(dir)?,
        ForeignSource::Zotero => zotero::read(dir)?,
        ForeignSource::Bibtex => bib::read_bibtex_file(path)?,
        ForeignSource::Ris => bib::read_ris_file(path)?,
        ForeignSource::Goodreads => reading_log::read_goodreads(path)?,
        ForeignSource::StoryGraph => reading_log::read_storygraph(path)?,
    };
    for b in &mut lib.books {
        libreri_metadata::normalise::tidy(&mut b.metadata);
        if b.metadata.title.trim().is_empty() {
            b.metadata.title = b
                .files
                .first()
                .map(|f| BookMetadata::title_from_file_name(&f.path))
                .unwrap_or_else(|| "Untitled".into());
        }
    }
    Ok(lib)
}

/// A file Libreri can open, if `path` is one.
pub(crate) fn foreign_file(path: PathBuf) -> Option<ForeignFile> {
    let file_type = FileType::from_path(&path)?;
    Some(ForeignFile { path, file_type })
}

/// A stable UUID for another app's note id.
pub(crate) fn stable_uuid(key: &str) -> String {
    const NS: uuid::Uuid = uuid::Uuid::from_u128(0x6c1b_7d0e_2f5a_4e21_9b8c_3d4e_5f60_7182);
    uuid::Uuid::new_v5(&NS, key.as_bytes()).to_string()
}

/// "2020-01-02 10:11:12" (UTC) → "2020-01-02T10:11:12Z".
pub(crate) fn sql_time(s: &str) -> Option<String> {
    let s = s.trim();
    if s.len() < 10 {
        return None;
    }
    let date = &s[..10];
    let time = s.get(11..19).unwrap_or("00:00:00");
    chrono::NaiveDateTime::parse_from_str(&format!("{date} {time}"), "%Y-%m-%d %H:%M:%S")
        .ok()
        .map(|t| {
            t.and_utc()
                .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
        })
}

/// The nearest of Libreri's four highlight colours to a hex colour.
pub(crate) fn nearest_colour(hex: &str) -> HighlightColor {
    let h = hex.trim().trim_start_matches('#');
    let Ok(v) = u32::from_str_radix(h, 16) else {
        return HighlightColor::Yellow;
    };
    if h.len() != 6 {
        return HighlightColor::Yellow;
    }
    let (r, g, b) = (
        (v >> 16) as f64,
        ((v >> 8) & 0xff) as f64,
        (v & 0xff) as f64,
    );
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    if max - min < 30.0 {
        return HighlightColor::Yellow; // grey
    }
    let hue = if max == r {
        60.0 * (((g - b) / (max - min)).rem_euclid(6.0))
    } else if max == g {
        60.0 * ((b - r) / (max - min) + 2.0)
    } else {
        60.0 * ((r - g) / (max - min) + 4.0)
    };
    match hue {
        h if h < 20.0 => HighlightColor::Pink,
        h if h < 70.0 => HighlightColor::Yellow,
        h if h < 170.0 => HighlightColor::Green,
        h if h < 255.0 => HighlightColor::Blue,
        _ => HighlightColor::Pink,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colours_times_and_ids() {
        assert_eq!(nearest_colour("#ffd400"), HighlightColor::Yellow);
        assert_eq!(nearest_colour("#5fb236"), HighlightColor::Green);
        assert_eq!(nearest_colour("#2ea8e5"), HighlightColor::Blue);
        assert_eq!(nearest_colour("#ff6666"), HighlightColor::Pink);
        assert_eq!(nearest_colour("#a28ae5"), HighlightColor::Pink);
        assert_eq!(nearest_colour("#f19837"), HighlightColor::Yellow);
        assert_eq!(nearest_colour("#aaaaaa"), HighlightColor::Yellow);
        assert_eq!(
            sql_time("2020-01-02 10:11:12").as_deref(),
            Some("2020-01-02T10:11:12Z")
        );
        assert_eq!(
            sql_time("2020-01-02").as_deref(),
            Some("2020-01-02T00:00:00Z")
        );
        assert_eq!(stable_uuid("zotero:1:ABC"), stable_uuid("zotero:1:ABC"));
        assert_ne!(stable_uuid("zotero:1:ABC"), stable_uuid("zotero:1:ABD"));
    }

    #[test]
    fn detects_sources() {
        let dir = tempfile::tempdir().unwrap();
        let cal = dir.path().join("Calibre Library");
        std::fs::create_dir(&cal).unwrap();
        std::fs::write(cal.join("metadata.db"), "").unwrap();
        assert_eq!(detect(&cal), Some(ForeignSource::Calibre));
        let gr = dir.path().join("goodreads_library_export.csv");
        std::fs::write(&gr, "Book Id,Title,Author,Exclusive Shelf\n").unwrap();
        assert_eq!(detect(&gr), Some(ForeignSource::Goodreads));
        let sg = dir.path().join("sg.csv");
        std::fs::write(&sg, "Title,Authors,ISBN/UID,Read Status,Star Rating\n").unwrap();
        assert_eq!(detect(&sg), Some(ForeignSource::StoryGraph));
        let other = dir.path().join("x.csv");
        std::fs::write(&other, "a,b\n").unwrap();
        assert_eq!(detect(&other), None);
        assert_eq!(
            detect(&dir.path().join("refs.bib")),
            Some(ForeignSource::Bibtex)
        );
    }
}
