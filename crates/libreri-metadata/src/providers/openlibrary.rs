//! Open Library (openlibrary.org): books by ISBN and by title.

use super::{names, num, s, strings};
use crate::http::{enc, json};
use crate::normalise::{language, tags, year};
use crate::{Candidate, Http, Source};
use libreri_core::BookMetadata;
use serde_json::Value;

const BASE: &str = "https://openlibrary.org";

pub fn by_isbn(http: &dyn Http, isbn13: &str) -> Result<Vec<Candidate>, String> {
    let url = format!("{BASE}/api/books?bibkeys=ISBN:{isbn13}&format=json&jscmd=data");
    let v = json(http.get(&url, &[])?)?;
    Ok(parse_books(&v, isbn13))
}

pub fn search(
    http: &dyn Http,
    title: &str,
    author: Option<&str>,
) -> Result<Vec<Candidate>, String> {
    let mut url = format!(
        "{BASE}/search.json?title={}&limit=8&fields=key,title,subtitle,author_name,first_publish_year,publisher,number_of_pages_median,isbn,language,subject,cover_i,cover_edition_key",
        enc(title)
    );
    if let Some(a) = author {
        url.push_str(&format!("&author={}", enc(a)));
    }
    let v = json(http.get(&url, &[])?)?;
    Ok(parse_search(&v))
}

/// The `api/books?jscmd=data` answer: {"ISBN:…": {…}}.
pub(crate) fn parse_books(v: &Value, isbn13: &str) -> Vec<Candidate> {
    let Some(obj) = v.as_object() else {
        return Vec::new();
    };
    obj.values()
        .map(|b| {
            let ids = b.get("identifiers").cloned().unwrap_or(Value::Null);
            let isbn13s = strings(&ids, "isbn_13");
            let isbn10s = strings(&ids, "isbn_10");
            let edition = strings(&ids, "openlibrary").into_iter().next();
            let cover = b
                .get("cover")
                .and_then(|c| s(c, "large").or_else(|| s(c, "medium")))
                .or_else(|| {
                    Some(format!(
                        "https://covers.openlibrary.org/b/isbn/{isbn13}-L.jpg?default=false"
                    ))
                });
            let m = BookMetadata {
                title: s(b, "title").unwrap_or_default(),
                subtitle: s(b, "subtitle"),
                authors: names(b, "authors", "name"),
                publisher: names(b, "publishers", "name").into_iter().next(),
                year: s(b, "publish_date").as_deref().and_then(year),
                pages: num(b, "number_of_pages"),
                isbn13: isbn13s
                    .into_iter()
                    .next()
                    .or_else(|| Some(isbn13.to_owned())),
                isbn10: isbn10s.into_iter().next(),
                tags: tags(names(b, "subjects", "name")),
                url: s(b, "url"),
                ..Default::default()
            };
            Candidate {
                source: Source::OpenLibrary,
                source_id: edition.clone().or_else(|| s(b, "key")).unwrap_or_default(),
                link: s(b, "url"),
                metadata: m,
                cover_url: cover,
                score: 1.0,
            }
        })
        .collect()
}

/// The `search.json` answer.
pub(crate) fn parse_search(v: &Value) -> Vec<Candidate> {
    let docs = v
        .get("docs")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    docs.iter()
        .map(|d| {
            let key = s(d, "key").unwrap_or_default();
            let isbns = strings(d, "isbn");
            let m = BookMetadata {
                title: s(d, "title").unwrap_or_default(),
                subtitle: s(d, "subtitle"),
                authors: strings(d, "author_name"),
                year: super::num(d, "first_publish_year").map(|y| y as i32),
                publisher: strings(d, "publisher").into_iter().next(),
                pages: num(d, "number_of_pages_median"),
                isbn13: isbns.iter().find(|i| i.len() == 13).cloned(),
                isbn10: isbns.iter().find(|i| i.len() == 10).cloned(),
                language: strings(d, "language").first().and_then(|l| language(l)),
                tags: tags(strings(d, "subject")),
                ..Default::default()
            };
            let cover = d
                .get("cover_i")
                .and_then(Value::as_i64)
                .filter(|&i| i > 0)
                .map(|i| format!("https://covers.openlibrary.org/b/id/{i}-L.jpg"));
            Candidate {
                source: Source::OpenLibrary,
                link: (!key.is_empty()).then(|| format!("{BASE}{key}")),
                source_id: key,
                metadata: m,
                cover_url: cover,
                score: 0.0,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixture;

    #[test]
    fn reads_isbn_answers() {
        let v: Value = serde_json::from_str(&fixture("openlibrary_isbn.json")).unwrap();
        let c = parse_books(&v, "9780306406157");
        assert_eq!(c.len(), 1);
        let m = &c[0].metadata;
        assert_eq!(m.title, "Linear Algebra Done Right");
        assert_eq!(m.authors, ["John Smith"]);
        assert_eq!(m.publisher.as_deref(), Some("Springer"));
        assert_eq!(m.year, Some(2015));
        assert_eq!(m.pages, Some(340));
        assert_eq!(m.tags, ["Linear algebra", "Vector spaces"]);
        assert_eq!(c[0].source_id, "OL7353617M");
        assert_eq!(
            c[0].cover_url.as_deref(),
            Some("https://covers.openlibrary.org/b/id/8739161-L.jpg")
        );
        assert!(parse_books(&serde_json::json!({}), "x").is_empty());
    }

    #[test]
    fn reads_search_answers() {
        let v: Value = serde_json::from_str(&fixture("openlibrary_search.json")).unwrap();
        let c = parse_search(&v);
        assert_eq!(c.len(), 2);
        let m = &c[0].metadata;
        assert_eq!(m.title, "Linear Algebra Done Right");
        assert_eq!(m.authors, ["Sheldon Axler"]);
        assert_eq!(m.year, Some(1995));
        assert_eq!(m.language.as_deref(), Some("en"));
        assert_eq!(m.isbn13.as_deref(), Some("9783319110806"));
        assert_eq!(
            c[0].link.as_deref(),
            Some("https://openlibrary.org/works/OL1943966W")
        );
        assert!(c[1].cover_url.is_none());
    }
}
