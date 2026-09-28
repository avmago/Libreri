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
    /// A drawing, shape, text box, sticky note, stamp, image or measurement
    /// on a page (markup mode). The locator holds the whole item.
    Markup,
    /// A spoken note: on selected text (with a quote) or at a place. The
    /// locator's `audio` is the recording in the profile's notes folder;
    /// `note` holds its transcript.
    Voice,
    /// Photographed paper notes about a place: the locator's `capture` is
    /// the PDF in the profile's notes folder; `note` holds its text.
    Capture,
    /// A link to a web page, a video, a file on this computer or another
    /// book, from selected text or a place: the locator's `link` holds it.
    Link,
}

impl AnnotationKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Highlight => "highlight",
            Self::Bookmark => "bookmark",
            Self::Markup => "markup",
            Self::Voice => "voice",
            Self::Capture => "capture",
            Self::Link => "link",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "highlight" => Some(Self::Highlight),
            "bookmark" => Some(Self::Bookmark),
            "markup" => Some(Self::Markup),
            "voice" => Some(Self::Voice),
            "capture" => Some(Self::Capture),
            "link" => Some(Self::Link),
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
    #[error("the markup item is not valid")]
    BadMarkup,
    #[error("the voice note's recording is missing")]
    BadVoice,
    #[error("the captured pages are missing")]
    BadCapture,
    #[error("the link is not valid")]
    BadLink,
    #[error("the markup item is too large (pictures are limited to about 2 MB)")]
    MarkupTooLarge,
}

/// Largest markup item (pictures and signatures are stored inside it).
pub const MAX_MARKUP_BYTES: usize = 3 * 1024 * 1024;

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
        if self.kind == AnnotationKind::Markup {
            let v: serde_json::Value =
                serde_json::from_str(&self.locator).map_err(|_| AnnotationError::BadLocator)?;
            let ok = v.get("type").and_then(|t| t.as_str()) == Some("markup")
                && v.get("page")
                    .and_then(|p| p.as_u64())
                    .is_some_and(|p| p >= 1)
                && v.get("item").is_some_and(|i| i.is_object());
            if !ok {
                return Err(AnnotationError::BadMarkup);
            }
            if self.locator.len() > MAX_MARKUP_BYTES {
                return Err(AnnotationError::MarkupTooLarge);
            }
        }
        if self.kind == AnnotationKind::Voice {
            let v: serde_json::Value =
                serde_json::from_str(&self.locator).map_err(|_| AnnotationError::BadLocator)?;
            let ok = v.get("audio").and_then(|a| a.as_str()).is_some_and(|a| {
                a.starts_with("Notes/")
                    && !a.split('/').any(|p| p == ".." || p.is_empty())
                    && [".flac", ".wav", ".ogg", ".opus", ".m4a", ".mp3", ".webm"]
                        .iter()
                        .any(|e| a.to_ascii_lowercase().ends_with(e))
            });
            if !ok {
                return Err(AnnotationError::BadVoice);
            }
        }
        if self.kind == AnnotationKind::Capture {
            let v: serde_json::Value =
                serde_json::from_str(&self.locator).map_err(|_| AnnotationError::BadLocator)?;
            let ok = v.get("capture").and_then(|a| a.as_str()).is_some_and(|a| {
                a.starts_with("Notes/")
                    && !a.split('/').any(|p| p == ".." || p.is_empty())
                    && a.to_ascii_lowercase().ends_with(".pdf")
            });
            if !ok {
                return Err(AnnotationError::BadCapture);
            }
        }
        if self.kind == AnnotationKind::Link {
            let v: serde_json::Value =
                serde_json::from_str(&self.locator).map_err(|_| AnnotationError::BadLocator)?;
            let link = v.get("link").ok_or(AnnotationError::BadLink)?;
            let url = link.get("url").and_then(|u| u.as_str()).unwrap_or("");
            let web = url.starts_with("https://") || url.starts_with("http://");
            let book = url.starts_with("libreri://book/");
            let file = link
                .get("file")
                .and_then(|f| f.as_str())
                .is_some_and(|f| !f.is_empty());
            // Files Libreri made for the link stay in the notes folder.
            let in_notes = |key: &str| {
                link.get(key).and_then(|p| p.as_str()).is_none_or(|p| {
                    p.starts_with("Notes/") && !p.split('/').any(|s| s == ".." || s.is_empty())
                })
            };
            if !(web || book || file) || !in_notes("picture") || !in_notes("copy") {
                return Err(AnnotationError::BadLink);
            }
        }
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

        let mut ink = highlight();
        ink.kind = AnnotationKind::Markup;
        ink.quote = None;
        ink.locator = r#"{"type":"markup","page":2,"item":{"tool":"pen","points":[]}}"#.into();
        assert!(ink.clone().validated().is_ok());
        ink.locator = r#"{"type":"markup","page":0,"item":{}}"#.into();
        assert_eq!(ink.clone().validated(), Err(AnnotationError::BadMarkup));
        let mut voice = ink;
        voice.kind = AnnotationKind::Voice;
        voice.locator = r#"{"type":"pdf","page":2,"audio":"Notes/Me/Voice notes/a.flac"}"#.into();
        assert!(voice.clone().validated().is_ok());
        voice.locator = r#"{"type":"pdf","page":2,"audio":"Notes/../secret.flac"}"#.into();
        assert_eq!(voice.clone().validated(), Err(AnnotationError::BadVoice));
        voice.locator = r#"{"type":"pdf","page":2}"#.into();
        assert_eq!(voice.clone().validated(), Err(AnnotationError::BadVoice));
        let mut cap = voice;
        cap.kind = AnnotationKind::Capture;
        cap.locator = r#"{"type":"pdf","page":2,"capture":"Notes/Me/Captures/p. 2.pdf"}"#.into();
        assert!(cap.clone().validated().is_ok());
        cap.locator = r#"{"type":"pdf","page":2,"capture":"Books/x.pdf"}"#.into();
        assert_eq!(cap.validated(), Err(AnnotationError::BadCapture));

        let mut link = highlight();
        link.kind = AnnotationKind::Link;
        link.locator = r#"{"type":"pdf","page":2,"link":{"url":"https://vimeo.com/1","picture":"Notes/Me/Links/a.jpg"}}"#.into();
        assert!(link.clone().validated().is_ok());
        link.locator = r#"{"type":"pdf","page":2,"link":{"url":"javascript:alert(1)"}}"#.into();
        assert_eq!(link.clone().validated(), Err(AnnotationError::BadLink));
        link.locator =
            r#"{"type":"pdf","page":2,"link":{"url":"https://a.b","copy":"../x.html"}}"#.into();
        assert_eq!(link.clone().validated(), Err(AnnotationError::BadLink));
        link.locator =
            r#"{"type":"pdf","page":2,"link":{"url":"","file":"/Users/me/talk.mp4"}}"#.into();
        assert!(link.validated().is_ok());
    }
}
