//! Highlights, comments and bookmarks.
//!
//! Every annotation points into a book with a *locator* (precise, format
//! specific) and, for text, a *quote* (the highlighted words and a little
//! context). The locator is stored as JSON that the reader for that format
//! understands; the quote lets an annotation find its place again when the
//! locator no longer fits, e.g. in another edition of the same book (see
//! `docs/data-portability.md`).

use crate::BookId;
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationKind {
    Highlight,
    Bookmark,
}

impl AnnotationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Highlight => "highlight",
            Self::Bookmark => "bookmark",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "highlight" => Some(Self::Highlight),
            "bookmark" => Some(Self::Bookmark),
            _ => None,
        }
    }
}

/// The four highlight colours.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum HighlightColor {
    #[default]
    Yellow,
    Green,
    Blue,
    Pink,
}

impl HighlightColor {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Yellow => "yellow",
            Self::Green => "green",
            Self::Blue => "blue",
            Self::Pink => "pink",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Self::Yellow, Self::Green, Self::Blue, Self::Pink]
            .into_iter()
            .find(|c| c.as_str() == s)
    }
}

/// The highlighted words with a little text before and after (W3C
/// TextQuoteSelector).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct TextQuote {
    pub exact: String,
    pub prefix: String,
    pub suffix: String,
}

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    /// UUID, created by whoever makes the annotation.
    pub id: String,
    pub book_id: BookId,
    pub kind: AnnotationKind,
    pub color: Option<HighlightColor>,
    /// Format-specific position as JSON, e.g. `{"type":"pdf","page":4,…}`.
    pub locator: String,
    pub quote: Option<TextQuote>,
    /// The comment written on a highlight, or a bookmark's name.
    pub note: Option<String>,
    /// Where it is, for people: "p. 4" or a chapter title.
    pub label: Option<String>,
    /// 0.0–1.0 through the book, for sorting.
    pub position: f64,
    pub created_at: String,
    pub modified_at: String,
}

/// Why an annotation was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnnotationError {
    #[error("the annotation id must be a UUID")]
    BadId,
    #[error("the position in the book is not valid JSON")]
    BadLocator,
    #[error("a highlight needs the highlighted text")]
    EmptyHighlight,
}

impl Annotation {
    /// Checks the fields and tidies text.
    pub fn validated(mut self) -> Result<Self, AnnotationError> {
        uuid::Uuid::parse_str(&self.id).map_err(|_| AnnotationError::BadId)?;
        serde_json::from_str::<serde_json::Value>(&self.locator)
            .map_err(|_| AnnotationError::BadLocator)?;
        if self.kind == AnnotationKind::Highlight {
            let empty = self
                .quote
                .as_ref()
                .is_none_or(|q| q.exact.trim().is_empty());
            if empty {
                return Err(AnnotationError::EmptyHighlight);
            }
            self.color.get_or_insert_default();
        }
        self.note = self
            .note
            .map(|n| n.trim().to_owned())
            .filter(|n| !n.is_empty());
        self.position = self.position.clamp(0.0, 1.0);
        Ok(self)
    }

    /// Link to this place in the book, used in notebooks and exports.
    pub fn link(&self) -> String {
        book_link(&self.book_id, Some(&self.id))
    }
}

/// `libreri://book/<id>` or `libreri://book/<id>#annotation=<uuid>`.
pub fn book_link(book: &BookId, annotation: Option<&str>) -> String {
    match annotation {
        Some(a) => format!("libreri://book/{book}#annotation={a}"),
        None => format!("libreri://book/{book}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn highlight() -> Annotation {
        Annotation {
            id: "8f14e45f-ceea-4e7a-9c3f-0b6d9d2a1c11".into(),
            book_id: BookId::from_hex("a".repeat(64)).unwrap(),
            kind: AnnotationKind::Highlight,
            color: None,
            locator: r#"{"type":"pdf","page":4}"#.into(),
            quote: Some(TextQuote {
                exact: "weighted sum".into(),
                ..Default::default()
            }),
            note: Some("  ".into()),
            label: Some("p. 4".into()),
            position: 1.5,
            created_at: String::new(),
            modified_at: String::new(),
        }
    }

    #[test]
    fn validation_fills_defaults_and_refuses_bad_input() {
        let a = highlight().validated().unwrap();
        assert_eq!(a.color, Some(HighlightColor::Yellow));
        assert_eq!(a.note, None);
        assert_eq!(a.position, 1.0);
        assert!(a
            .link()
            .ends_with("#annotation=8f14e45f-ceea-4e7a-9c3f-0b6d9d2a1c11"));

        let mut bad = highlight();
        bad.id = "x".into();
        assert_eq!(bad.validated(), Err(AnnotationError::BadId));
        let mut bad = highlight();
        bad.locator = "{".into();
        assert_eq!(bad.validated(), Err(AnnotationError::BadLocator));
        let mut bad = highlight();
        bad.quote = None;
        assert_eq!(bad.validated(), Err(AnnotationError::EmptyHighlight));
        let mut bookmark = highlight();
        bookmark.kind = AnnotationKind::Bookmark;
        bookmark.quote = None;
        assert!(bookmark.validated().is_ok());
    }
}
