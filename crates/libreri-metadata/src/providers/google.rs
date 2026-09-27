//! Google Books (googleapis.com/books): books by ISBN and by title. Works
//! without a key, within Google's daily limits.

use super::{num, s, strings};
use crate::http::{enc, json};
use crate::normalise::{about, category_path, language, year};
use crate::{Candidate, Http, Source};
use libreri_core::BookMetadata;
use serde_json::Value;

const BASE: &str = "https://www.googleapis.com/books/v1/volumes";

pub fn by_isbn(http: &dyn Http, isbn13: &str) -> Result<Vec<Candidate>, String> {
    let v = json(http.get(&format!("{BASE}?q=isbn:{isbn13}&maxResults=3"), &[])?)?;
    let mut c = parse(&v);
    // Google sometimes answers an ISBN search with other books.
    c.retain(|c| c.metadata.isbn13.as_deref() == Some(isbn13));
    for x in &mut c {
        x.score = 1.0;
    }
    Ok(c)
}

pub fn search(
    http: &dyn Http,
    title: &str,
    author: Option<&str>,
) -> Result<Vec<Candidate>, String> {
    let mut q = format!("intitle:{}", enc(title));
    if let Some(a) = author {
        q.push_str(&format!("+inauthor:{}", enc(a)));
    }
    let v = json(http.get(&format!("{BASE}?q={q}&maxResults=8&printType=books"), &[])?)?;
    Ok(parse(&v))
}

/// A cover link: https, the larger size, no page-curl effect.
fn cover(links: &Value) -> Option<String> {
    let url = [
        "extraLarge",
        "large",
        "medium",
        "thumbnail",
        "smallThumbnail",
    ]
    .iter()
    .find_map(|k| s(links, k))?;
    Some(
        url.replacen("http://", "https://", 1)
            .replace("&edge=curl", "")
            .replace("zoom=5", "zoom=1"),
    )
}

pub(crate) fn parse(v: &Value) -> Vec<Candidate> {
    let items = v
        .get("items")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    items
        .iter()
        .filter_map(|item| {
            let id = s(item, "id")?;
            let info = item.get("volumeInfo")?;
            let ids = info.get("industryIdentifiers").and_then(Value::as_array);
            let isbn = |kind: &str| {
                ids.and_then(|a| {
                    a.iter()
                        .find(|x| s(x, "type").as_deref() == Some(kind))
                        .and_then(|x| s(x, "identifier"))
                })
            };
            let m = BookMetadata {
                title: s(info, "title").unwrap_or_default(),
                subtitle: s(info, "subtitle"),
                authors: strings(info, "authors"),
                publisher: s(info, "publisher").map(|p| p.trim_matches('"').to_owned()),
                year: s(info, "publishedDate").as_deref().and_then(year),
                about: s(info, "description").as_deref().and_then(about),
                pages: num(info, "pageCount"),
                isbn13: isbn("ISBN_13"),
                isbn10: isbn("ISBN_10"),
                language: s(info, "language").as_deref().and_then(language),
                categories: strings(info, "categories")
                    .iter()
                    .filter_map(|c| category_path(c))
                    .collect(),
                ..Default::default()
            };
            Some(Candidate {
                source: Source::GoogleBooks,
                link: s(info, "canonicalVolumeLink").or_else(|| s(info, "infoLink")),
                source_id: id,
                metadata: m,
                cover_url: info.get("imageLinks").and_then(cover),
                score: 0.0,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixture;

    #[test]
    fn reads_volumes() {
        let v: Value = serde_json::from_str(&fixture("google_isbn.json")).unwrap();
        let c = parse(&v);
        assert_eq!(c.len(), 1);
        let m = &c[0].metadata;
        assert_eq!(m.title, "Linear Algebra Done Right");
        assert_eq!(m.isbn13.as_deref(), Some("9780306406157"));
        assert_eq!(m.categories, ["Mathematics/Algebra/Linear"]);
        assert_eq!(m.language.as_deref(), Some("en"));
        assert_eq!(m.year, Some(2015));
        assert_eq!(
            m.about.as_deref(),
            Some("This best-selling textbook for a second course in linear algebra.")
        );
        assert_eq!(
            c[0].cover_url.as_deref(),
            Some("https://books.google.com/books/content?id=abc123&printsec=frontcover&img=1&zoom=1&source=gbs_api")
        );
        let v: Value = serde_json::from_str(r#"{"totalItems":0}"#).unwrap();
        assert!(parse(&v).is_empty());
    }
}
