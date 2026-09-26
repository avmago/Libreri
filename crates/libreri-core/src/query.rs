//! What the library view asks for: which books, in which order.

use crate::{ContentType, FileType, ReadingStatus};
use serde::{Deserialize, Serialize};

#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum SortKey {
    #[default]
    Title,
    Author,
    Added,
    Year,
    Size,
    Pages,
    LastOpened,
    Rating,
}

/// A filter for the book list. Empty lists mean "any".
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BookQuery {
    /// Folder relative to `Books/` ("" = top level). `None` = whole library.
    pub folder: Option<String>,
    /// With `folder`, also include books in its subfolders.
    pub include_subfolders: bool,
    /// Free text matched against title, authors, tags, ISBN, …
    pub search: Option<String>,
    pub file_types: Vec<FileType>,
    pub content_types: Vec<ContentType>,
    /// Books must have every one of these tags.
    pub tags: Vec<String>,
    /// Books must be in this category or one below it.
    pub category: Option<String>,
    pub status: Option<ReadingStatus>,
    pub favorites_only: bool,
    /// `Some(true)` only audiobooks, `Some(false)` hides them.
    pub audio: Option<bool>,
    pub missing_only: bool,
    pub sort: SortKey,
    pub descending: bool,
}

/// Turns what the user typed into a safe SQLite FTS5 query: each word
/// becomes a quoted prefix term, all of which must match.
pub fn fts_query(input: &str) -> Option<String> {
    let mut terms: Vec<String> = Vec::new();
    for word in input.split_whitespace() {
        let isbn_like = word
            .chars()
            .all(|c| c.is_ascii_digit() || matches!(c, '-' | 'x' | 'X'));
        if isbn_like {
            // "978-0-306-40615-7" is stored without hyphens.
            terms.push(word.replace('-', ""));
        } else {
            terms.extend(
                word.split(|c: char| !c.is_alphanumeric())
                    .filter(|w| !w.is_empty())
                    .map(str::to_owned),
            );
        }
    }
    terms.retain(|t| !t.is_empty());
    if terms.is_empty() {
        None
    } else {
        Some(
            terms
                .iter()
                .map(|t| format!("\"{t}\"*"))
                .collect::<Vec<_>>()
                .join(" "),
        )
    }
}

/// Key used to sort titles: ignores a leading English article and case.
pub fn sort_title(title: &str) -> String {
    let t = title.trim();
    let lower = t.to_lowercase();
    for article in ["the ", "a ", "an "] {
        if lower.starts_with(article) && lower.len() > article.len() {
            return lower[article.len()..].trim_start().to_owned();
        }
    }
    lower
}

/// Key used to sort by author: "John Smith" sorts as "smith john".
pub fn sort_author(authors: &[String]) -> String {
    let Some(first) = authors.first() else {
        return "\u{10FFFF}".to_owned(); // books without authors go last
    };
    if first.contains(',') {
        return first.to_lowercase().replace(',', "");
    }
    let mut words: Vec<&str> = first.split_whitespace().collect();
    if let Some(last) = words.pop() {
        words.insert(0, last);
    }
    words.join(" ").to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_prefix_queries_and_strips_syntax() {
        assert_eq!(
            fts_query("quantum  mech").as_deref(),
            Some("\"quantum\"* \"mech\"*")
        );
        assert_eq!(
            fts_query("a\"b OR (c)").as_deref(),
            Some("\"a\"* \"b\"* \"OR\"* \"c\"*")
        );
        assert_eq!(fts_query("  ()  "), None);
        assert_eq!(
            fts_query("978-0-306-40615-7").as_deref(),
            Some("\"9780306406157\"*")
        );
    }

    #[test]
    fn sort_keys() {
        assert_eq!(sort_title("The Art of War"), "art of war");
        assert_eq!(sort_title("A"), "a");
        assert_eq!(sort_author(&["John Smith".into()]), "smith john");
        assert_eq!(sort_author(&["Smith, Jane".into()]), "smith jane");
    }
}
