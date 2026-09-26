//! Books: file types, content types and the metadata we keep for each book.

use crate::BookId;
use serde::{Deserialize, Serialize};

/// Every file format Libreri accepts, detected from the file extension.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileType {
    Pdf,
    Epub,
    Mobi,
    Azw3,
    Fb2,
    Txt,
    Md,
    Djvu,
    Cbz,
    Cbr,
    Cb7,
    Cba,
    Cbt,
    Mp3,
    M4b,
    M4a,
    Aac,
    Ogg,
    Opus,
    Flac,
}

impl FileType {
    pub const ALL: &'static [FileType] = &[
        Self::Pdf,
        Self::Epub,
        Self::Mobi,
        Self::Azw3,
        Self::Fb2,
        Self::Txt,
        Self::Md,
        Self::Djvu,
        Self::Cbz,
        Self::Cbr,
        Self::Cb7,
        Self::Cba,
        Self::Cbt,
        Self::Mp3,
        Self::M4b,
        Self::M4a,
        Self::Aac,
        Self::Ogg,
        Self::Opus,
        Self::Flac,
    ];

    /// Detects the type from a file name. Returns `None` for files Libreri
    /// does not manage (they are left alone).
    pub fn from_path(path: &std::path::Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?.to_ascii_lowercase();
        Some(match ext.as_str() {
            "pdf" => Self::Pdf,
            "epub" => Self::Epub,
            "mobi" | "prc" => Self::Mobi,
            "azw3" | "azw" | "kf8" => Self::Azw3,
            "fb2" => Self::Fb2,
            "txt" => Self::Txt,
            "md" | "markdown" => Self::Md,
            "djvu" | "djv" => Self::Djvu,
            "cbz" => Self::Cbz,
            "cbr" => Self::Cbr,
            "cb7" => Self::Cb7,
            "cba" => Self::Cba,
            "cbt" => Self::Cbt,
            "mp3" => Self::Mp3,
            "m4b" => Self::M4b,
            "m4a" => Self::M4a,
            "aac" => Self::Aac,
            "ogg" | "oga" => Self::Ogg,
            "opus" => Self::Opus,
            "flac" => Self::Flac,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pdf => "pdf",
            Self::Epub => "epub",
            Self::Mobi => "mobi",
            Self::Azw3 => "azw3",
            Self::Fb2 => "fb2",
            Self::Txt => "txt",
            Self::Md => "md",
            Self::Djvu => "djvu",
            Self::Cbz => "cbz",
            Self::Cbr => "cbr",
            Self::Cb7 => "cb7",
            Self::Cba => "cba",
            Self::Cbt => "cbt",
            Self::Mp3 => "mp3",
            Self::M4b => "m4b",
            Self::M4a => "m4a",
            Self::Aac => "aac",
            Self::Ogg => "ogg",
            Self::Opus => "opus",
            Self::Flac => "flac",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.as_str() == s)
    }

    pub fn is_audio(self) -> bool {
        matches!(
            self,
            Self::Mp3 | Self::M4b | Self::M4a | Self::Aac | Self::Ogg | Self::Opus | Self::Flac
        )
    }

    pub fn is_comic(self) -> bool {
        matches!(
            self,
            Self::Cbz | Self::Cbr | Self::Cb7 | Self::Cba | Self::Cbt
        )
    }

    /// The content type a new book of this format starts with.
    pub fn default_content_type(self) -> ContentType {
        if self.is_audio() {
            ContentType::Audiobook
        } else if self.is_comic() {
            ContentType::Comic
        } else if self == Self::Md {
            ContentType::PersonalNotes
        } else {
            ContentType::Book
        }
    }
}

/// What kind of document a book is. Chosen by the user; guessed on import.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ContentType {
    #[default]
    Book,
    Textbook,
    ResearchPaper,
    ConferencePaper,
    Preprint,
    Thesis,
    LectureNotes,
    Slides,
    TechnicalReport,
    WhitePaper,
    Manual,
    Reference,
    Standard,
    Magazine,
    Article,
    Comic,
    CheatSheet,
    PersonalNotes,
    Audiobook,
    Other,
}

impl ContentType {
    pub const ALL: &'static [ContentType] = &[
        Self::Book,
        Self::Textbook,
        Self::ResearchPaper,
        Self::ConferencePaper,
        Self::Preprint,
        Self::Thesis,
        Self::LectureNotes,
        Self::Slides,
        Self::TechnicalReport,
        Self::WhitePaper,
        Self::Manual,
        Self::Reference,
        Self::Standard,
        Self::Magazine,
        Self::Article,
        Self::Comic,
        Self::CheatSheet,
        Self::PersonalNotes,
        Self::Audiobook,
        Self::Other,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Book => "book",
            Self::Textbook => "textbook",
            Self::ResearchPaper => "researchPaper",
            Self::ConferencePaper => "conferencePaper",
            Self::Preprint => "preprint",
            Self::Thesis => "thesis",
            Self::LectureNotes => "lectureNotes",
            Self::Slides => "slides",
            Self::TechnicalReport => "technicalReport",
            Self::WhitePaper => "whitePaper",
            Self::Manual => "manual",
            Self::Reference => "reference",
            Self::Standard => "standard",
            Self::Magazine => "magazine",
            Self::Article => "article",
            Self::Comic => "comic",
            Self::CheatSheet => "cheatSheet",
            Self::PersonalNotes => "personalNotes",
            Self::Audiobook => "audiobook",
            Self::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|t| t.as_str() == s)
    }
}

/// Where a reader is with a book. Personal: stored per profile.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum ReadingStatus {
    #[default]
    None,
    WantToRead,
    Reading,
    Finished,
    Abandoned,
}

impl ReadingStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::WantToRead => "wantToRead",
            Self::Reading => "reading",
            Self::Finished => "finished",
            Self::Abandoned => "abandoned",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [
            Self::None,
            Self::WantToRead,
            Self::Reading,
            Self::Finished,
            Self::Abandoned,
        ]
        .into_iter()
        .find(|t| t.as_str() == s)
    }
}

/// Bibliographic details shared by every profile. This is what the details
/// panel edits and what the JSON sidecar stores.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookMetadata {
    pub title: String,
    pub subtitle: Option<String>,
    pub authors: Vec<String>,
    /// Editors, translators, illustrators — free text such as "Jane Smith (translator)".
    pub contributors: Vec<String>,
    pub about: Option<String>,
    pub tags: Vec<String>,
    /// Hierarchical, written as paths: "Science/Physics".
    pub categories: Vec<String>,
    pub year: Option<i32>,
    pub publisher: Option<String>,
    pub pages: Option<u32>,
    pub isbn13: Option<String>,
    pub isbn10: Option<String>,
    pub edition: Option<String>,
    /// BCP 47 code such as "en" or "de".
    pub language: Option<String>,
    pub content_type: ContentType,
    pub series: Option<String>,
    pub series_number: Option<f64>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    pub journal: Option<String>,
    pub volume: Option<String>,
    pub issue: Option<String>,
    pub url: Option<String>,
}

/// Why metadata could not be saved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MetadataError {
    #[error("the title cannot be empty")]
    EmptyTitle,
    #[error("ISBN-13: {0}")]
    Isbn13(crate::isbn::IsbnError),
    #[error("ISBN-10: {0}")]
    Isbn10(crate::isbn::IsbnError),
    #[error("ISBN-10 and ISBN-13 are for different books")]
    IsbnMismatch,
    #[error("the year looks wrong")]
    Year,
}

fn tidy(s: Option<String>) -> Option<String> {
    s.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty())
}

fn tidy_list(list: Vec<String>) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(list.len());
    for item in list {
        let item = item.split_whitespace().collect::<Vec<_>>().join(" ");
        if !item.is_empty() && !out.iter().any(|o| o.eq_ignore_ascii_case(&item)) {
            out.push(item);
        }
    }
    out
}

fn tidy_category(c: &str) -> String {
    c.split('/')
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join("/")
}

impl BookMetadata {
    /// Trims text, removes duplicates, validates ISBNs and fills in the
    /// missing ISBN form when the other one is known.
    pub fn normalized(mut self) -> Result<Self, MetadataError> {
        use crate::isbn::*;
        self.title = self.title.trim().to_owned();
        if self.title.is_empty() {
            return Err(MetadataError::EmptyTitle);
        }
        self.subtitle = tidy(self.subtitle);
        self.authors = tidy_list(self.authors);
        self.contributors = tidy_list(self.contributors);
        self.about = tidy(self.about);
        self.tags = tidy_list(self.tags);
        self.categories = tidy_list(self.categories.iter().map(|c| tidy_category(c)).collect());
        self.publisher = tidy(self.publisher);
        self.edition = tidy(self.edition);
        self.language = tidy(self.language);
        self.series = tidy(self.series);
        self.doi = tidy(self.doi);
        self.arxiv_id = tidy(self.arxiv_id);
        self.journal = tidy(self.journal);
        self.volume = tidy(self.volume);
        self.issue = tidy(self.issue);
        self.url = tidy(self.url);
        if let Some(y) = self.year {
            if !(-3000..=2100).contains(&y) {
                return Err(MetadataError::Year);
            }
        }
        self.pages = self.pages.filter(|p| *p > 0);

        let i13 = tidy(self.isbn13)
            .map(|s| normalize_isbn13(&s).map_err(MetadataError::Isbn13))
            .transpose()?;
        let i10 = tidy(self.isbn10)
            .map(|s| normalize_isbn10(&s).map_err(MetadataError::Isbn10))
            .transpose()?;
        (self.isbn13, self.isbn10) = match (i13, i10) {
            (Some(a), Some(b)) => {
                if isbn10_to_13(&b).as_deref() != Some(a.as_str()) {
                    return Err(MetadataError::IsbnMismatch);
                }
                (Some(a), Some(b))
            }
            (Some(a), None) => {
                let b = isbn13_to_10(&a);
                (Some(a), b)
            }
            (None, Some(b)) => (isbn10_to_13(&b), Some(b)),
            (None, None) => (None, None),
        };
        Ok(self)
    }

    /// A title made from a file name: "the_art-of.war" → "the art of war".
    pub fn title_from_file_name(path: &std::path::Path) -> String {
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();
        let t = stem
            .replace(['_', '.'], " ")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if t.is_empty() {
            "Untitled".to_owned()
        } else {
            t
        }
    }
}

/// Personal state of one profile for one book.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookUserState {
    pub status: ReadingStatus,
    /// 0 = not rated, 1–5 stars.
    pub rating: u8,
    pub favorite: bool,
    /// 0.0–1.0.
    pub progress: f32,
    pub last_opened: Option<String>,
}

/// Everything about one book in the library.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Book {
    pub id: BookId,
    /// Relative to the library root, with `/` separators: "Books/Physics/a.pdf".
    pub rel_path: String,
    pub file_type: FileType,
    /// Sent to TypeScript as a number; no file comes near 2^53 bytes.
    #[cfg_attr(feature = "specta", specta(type = u32))]
    pub file_size: u64,
    pub has_cover: bool,
    /// The file could not be found at `rel_path` during the last scan.
    pub missing: bool,
    pub added_at: String,
    pub modified_at: String,
    pub metadata: BookMetadata,
    pub user: BookUserState,
}

impl Book {
    /// The folder a book is in, relative to `Books/` ("" for the top level).
    pub fn folder(&self) -> &str {
        folder_of(&self.rel_path)
    }
}

/// The folder part of a library-relative book path, relative to `Books/`.
pub fn folder_of(rel_path: &str) -> &str {
    let inner = rel_path
        .strip_prefix(crate::layout::BOOKS_DIR)
        .and_then(|s| s.strip_prefix('/'))
        .unwrap_or(rel_path);
    match inner.rfind('/') {
        Some(i) => &inner[..i],
        None => "",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn file_types_from_extensions() {
        assert_eq!(
            FileType::from_path(Path::new("a/B.PDF")),
            Some(FileType::Pdf)
        );
        assert_eq!(
            FileType::from_path(Path::new("x.markdown")),
            Some(FileType::Md)
        );
        assert_eq!(FileType::from_path(Path::new("x.docx")), None);
        assert_eq!(FileType::from_path(Path::new("noext")), None);
        for t in FileType::ALL {
            assert_eq!(FileType::parse(t.as_str()), Some(*t));
        }
        for t in ContentType::ALL {
            assert_eq!(ContentType::parse(t.as_str()), Some(*t));
        }
    }

    #[test]
    fn normalizing_fills_isbn_and_tidies_lists() {
        let m = BookMetadata {
            title: "  Physics ".into(),
            authors: vec!["John  Smith".into(), "john smith".into(), " ".into()],
            categories: vec![" Science / Physics ".into()],
            isbn13: Some("978-0-306-40615-7".into()),
            ..Default::default()
        }
        .normalized()
        .unwrap();
        assert_eq!(m.title, "Physics");
        assert_eq!(m.authors, vec!["John Smith"]);
        assert_eq!(m.categories, vec!["Science/Physics"]);
        assert_eq!(m.isbn10.as_deref(), Some("0306406152"));
    }

    #[test]
    fn normalizing_rejects_bad_input() {
        let base = BookMetadata {
            title: "T".into(),
            ..Default::default()
        };
        assert_eq!(
            BookMetadata {
                title: " ".into(),
                ..Default::default()
            }
            .normalized(),
            Err(MetadataError::EmptyTitle)
        );
        assert!(matches!(
            BookMetadata {
                isbn13: Some("9780306406158".into()),
                ..base.clone()
            }
            .normalized(),
            Err(MetadataError::Isbn13(_))
        ));
        assert_eq!(
            BookMetadata {
                isbn13: Some("9780306406157".into()),
                isbn10: Some("080442957X".into()),
                ..base
            }
            .normalized(),
            Err(MetadataError::IsbnMismatch)
        );
    }

    #[test]
    fn folders_and_titles() {
        assert_eq!(folder_of("Books/a.pdf"), "");
        assert_eq!(folder_of("Books/Sci/Phys/a.pdf"), "Sci/Phys");
        assert_eq!(
            BookMetadata::title_from_file_name(Path::new("the_art.of war.pdf")),
            "the art of war"
        );
    }
}
