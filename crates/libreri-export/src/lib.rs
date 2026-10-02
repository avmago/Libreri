//! Libreri data in other formats.
//!
//! Everything here is a pure function of the data it is given: the library
//! crate collects books, notes and files, and this crate turns them into
//! CSV, Excel, JSON, BibTeX, RIS, CSL-JSON, Obsidian notes or Calibre OPF,
//! formats citations, and reads and writes Libreri archives. No PINs, API
//! keys or computer-specific paths ever reach an export.

pub mod anki;
pub mod archive;
pub mod citation;
pub mod foreign;
mod names;
pub mod notes;
pub mod opf;
pub mod table;

use libreri_core::{Annotation, Book, ContentType, ReadingStatus};
use serde::{Deserialize, Serialize};

pub use citation::CitationStyle;
pub use names::{split_name, PersonName};

/// One book with the signed-in reader's notes about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub book: Book,
    pub annotations: Vec<Annotation>,
    /// The reader's notebook for the book: (library-relative path, content).
    pub notebook: Option<(String, String)>,
}

impl Entry {
    pub fn new(book: Book) -> Self {
        Self {
            book,
            annotations: Vec::new(),
            notebook: None,
        }
    }
}

/// Every format Libreri exports to.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    /// `.libreri` archive: everything, to import into Libreri again.
    Archive,
    Csv,
    Xlsx,
    Json,
    Bibtex,
    Ris,
    CslJson,
    /// A folder of Markdown notes for Obsidian (or any Markdown app).
    Obsidian,
    /// Book folders with `metadata.opf` and `cover.jpg`, for Calibre.
    Calibre,
    /// A copy of the catalogue database.
    Sqlite,
}

impl ExportFormat {
    /// File extension, or `None` for formats that write a folder.
    pub fn extension(self) -> Option<&'static str> {
        Some(match self {
            Self::Archive => "libreri",
            Self::Csv => "csv",
            Self::Xlsx => "xlsx",
            Self::Json => "json",
            Self::Bibtex => "bib",
            Self::Ris => "ris",
            Self::CslJson => "json",
            Self::Sqlite => "db",
            Self::Obsidian | Self::Calibre => return None,
        })
    }
}

/// "Research paper" for `ContentType::ResearchPaper`.
pub fn content_type_label(t: ContentType) -> &'static str {
    match t {
        ContentType::Book => "Book",
        ContentType::Textbook => "Textbook",
        ContentType::ResearchPaper => "Research paper",
        ContentType::ConferencePaper => "Conference paper",
        ContentType::Preprint => "Preprint",
        ContentType::Thesis => "Thesis",
        ContentType::LectureNotes => "Lecture notes",
        ContentType::Slides => "Slides",
        ContentType::TechnicalReport => "Technical report",
        ContentType::WhitePaper => "White paper",
        ContentType::Manual => "Manual",
        ContentType::Reference => "Reference",
        ContentType::Standard => "Standard",
        ContentType::Magazine => "Magazine",
        ContentType::Article => "Article",
        ContentType::Comic => "Comic",
        ContentType::CheatSheet => "Cheat sheet",
        ContentType::PersonalNotes => "Personal notes",
        ContentType::Audiobook => "Audiobook",
        ContentType::Other => "Other",
    }
}

pub fn status_label(s: ReadingStatus) -> &'static str {
    match s {
        ReadingStatus::None => "",
        ReadingStatus::WantToRead => "Want to read",
        ReadingStatus::Reading => "Reading",
        ReadingStatus::Finished => "Finished",
        ReadingStatus::Abandoned => "Stopped",
    }
}

/// "Title: Subtitle".
pub fn full_title(book: &Book) -> String {
    let m = &book.metadata;
    match m
        .subtitle
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        Some(sub) => format!("{}: {sub}", m.title.trim()),
        None => m.title.trim().to_owned(),
    }
}

/// Longest file-name stem, in UTF-8 bytes, that names made here use. Most
/// file systems allow 255 bytes; this leaves room for " (12)" and an
/// extension.
pub const MAX_NAME_BYTES: usize = 200;

/// `s` cut to at most `max` UTF-8 bytes, never inside a character.
pub fn truncate_bytes(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// A file or folder name that is valid on every system: at most 100
/// characters and [`MAX_NAME_BYTES`] bytes, no path separators or reserved
/// characters.
pub fn safe_file_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if matches!(
                c,
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '#' | '^' | '[' | ']'
            ) || c.is_control()
            {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cleaned: String = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .chars()
        .take(100)
        .collect();
    let cleaned = truncate_bytes(&cleaned, MAX_NAME_BYTES)
        .trim_matches(|c: char| c == '.' || c.is_whitespace())
        .to_owned();
    let reserved = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "LPT1", "LPT2", "LPT3",
    ];
    if cleaned.is_empty() || reserved.contains(&cleaned.to_ascii_uppercase().as_str()) {
        "Untitled".to_owned()
    } else {
        cleaned
    }
}

/// Picks `name` or `name 2`, `name 3`… so it is not in `taken` (compared
/// without case, as on macOS and Windows), and records the choice.
pub fn unique_name(taken: &mut std::collections::HashSet<String>, stem: &str, ext: &str) -> String {
    let make = |n: u32| {
        let s = if n == 1 {
            stem.to_owned()
        } else {
            format!("{stem} {n}")
        };
        if ext.is_empty() {
            s
        } else {
            format!("{s}.{ext}")
        }
    };
    let mut n = 1;
    loop {
        let candidate = make(n);
        if taken.insert(candidate.to_lowercase()) {
            return candidate;
        }
        n += 1;
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use libreri_core::*;

    pub fn book(title: &str) -> Book {
        Book {
            id: BookId::from_hex("ab".repeat(32)).unwrap(),
            rel_path: format!("Books/Physics/{title}.pdf"),
            file_type: FileType::Pdf,
            file_size: 1234,
            has_cover: false,
            missing: false,
            added_at: "2026-09-01T10:00:00Z".into(),
            modified_at: "2026-09-02T10:00:00Z".into(),
            metadata: BookMetadata {
                title: title.into(),
                authors: vec!["Jane Q. Smith".into(), "Ludwig van Beethoven".into()],
                year: Some(2019),
                publisher: Some("Acme Press".into()),
                isbn13: Some("9780131103627".into()),
                edition: Some("2".into()),
                tags: vec!["Optics".into(), "Light & waves".into()],
                categories: vec!["Science/Physics".into()],
                language: Some("en".into()),
                ..Default::default()
            },
            user: BookUserState {
                status: ReadingStatus::Finished,
                rating: 4,
                ..Default::default()
            },
        }
    }

    pub fn paper() -> Book {
        let mut b = book("Attention Is All You Need");
        b.metadata = BookMetadata {
            title: "Attention Is All You Need".into(),
            authors: vec![
                "Ashish Vaswani".into(),
                "Noam Shazeer".into(),
                "Niki Parmar".into(),
            ],
            year: Some(2017),
            content_type: ContentType::ConferencePaper,
            journal: Some("Advances in Neural Information Processing Systems".into()),
            volume: Some("30".into()),
            doi: Some("10.48550/arXiv.1706.03762".into()),
            arxiv_id: Some("1706.03762".into()),
            ..Default::default()
        };
        b
    }

    pub fn highlight(book: &Book, quote: &str, note: Option<&str>) -> Annotation {
        Annotation {
            id: "8f14e45f-ceea-4e7a-9c3f-0b6d9d2a1c11".into(),
            book_id: book.id.clone(),
            kind: AnnotationKind::Highlight,
            color: Some(HighlightColor::Yellow),
            locator: r#"{"type":"pdf","page":4}"#.into(),
            quote: Some(TextQuote {
                exact: quote.into(),
                ..Default::default()
            }),
            note: note.map(str::to_owned),
            label: Some("p. 4".into()),
            position: 0.1,
            created_at: "2026-09-03T10:00:00Z".into(),
            modified_at: "2026-09-03T10:00:00Z".into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn long_names_fit_in_255_bytes() {
        let cjk = "漢".repeat(100);
        let name = safe_file_name(&cjk);
        assert!(name.len() <= MAX_NAME_BYTES, "{} bytes", name.len());
        assert!(name.chars().all(|c| c == '漢'));
        assert_eq!(truncate_bytes("aé", 2), "a");
        assert_eq!(truncate_bytes("abc", 10), "abc");
    }

    #[test]
    fn file_names_are_safe_and_unique() {
        assert_eq!(safe_file_name("a/b: c?"), "a b c");
        assert_eq!(safe_file_name(" .. "), "Untitled");
        assert_eq!(safe_file_name("con"), "Untitled");
        let mut taken = Default::default();
        assert_eq!(unique_name(&mut taken, "Optics", "md"), "Optics.md");
        assert_eq!(unique_name(&mut taken, "optics", "md"), "optics 2.md");
    }
}
