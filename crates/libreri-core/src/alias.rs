//! Earlier ids of a book.
//!
//! A book's id is the hash of its file. When the file changes, or when notes
//! are brought over from another copy of the same book, the old id is kept as
//! an *alias* so every `libreri://book/<old id>` link still opens the book.

use crate::BookId;
use serde::{Deserialize, Serialize};

/// Why an old book id points to a book.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AliasKind {
    /// The book's file was edited; its old id is kept so links still work.
    #[default]
    Changed,
    /// Notes were imported from another copy or edition of the book. Their
    /// precise positions may not fit, so they find their place by quote.
    OtherFile,
}

impl AliasKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Changed => "changed",
            Self::OtherFile => "otherFile",
        }
    }

    pub fn parse(s: &str) -> Self {
        if s == "otherFile" {
            Self::OtherFile
        } else {
            Self::Changed
        }
    }
}

/// An earlier id of a book.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alias {
    pub id: BookId,
    #[serde(default)]
    pub kind: AliasKind,
}
