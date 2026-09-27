//! ISBNdb (api2.isbndb.com): books by ISBN and title. Needs the reader's own
//! API key (a paid service).

use super::{num, s, strings};
use crate::http::{enc, json};
use crate::normalise::{about, language, tags, year};
use crate::{Candidate, Http, Source};
use libreri_core::BookMetadata;
use serde_json::Value;

const BASE: &str = "https://api2.isbndb.com";

pub fn by_isbn(http: &dyn Http, key: &str, isbn13: &str) -> Result<Vec<Candidate>, String> {
    let v = json(http.get(&format!("{BASE}/book/{isbn13}"), &[("Authorization", key)])?)?;
    Ok(v.get("book")
        .and_then(parse_book)
        .map(|mut c| {
            c.score = 1.0;
            c
        })
        .into_iter()
        .collect())
}

pub fn search(http: &dyn Http, key: &str, title: &str) -> Result<Vec<Candidate>, String> {
    let url = format!("{BASE}/books/{}?page=1&pageSize=6&column=title", enc(title));
    let v = json(http.get(&url, &[("Authorization", key)])?)?;
    Ok(v.get("books")
        .and_then(Value::as_array)
        .map(|b| b.iter().filter_map(parse_book).collect())
        .unwrap_or_default())
}

pub(crate) fn parse_book(b: &Value) -> Option<Candidate> {
    let isbn13 = s(b, "isbn13");
    let m = BookMetadata {
        title: s(b, "title").or_else(|| s(b, "title_long"))?,
        authors: strings(b, "authors")
            .iter()
            .map(|a| crate::normalise::person(a))
            .collect(),
        publisher: s(b, "publisher"),
        year: s(b, "date_published").as_deref().and_then(year),
        pages: num(b, "pages"),
        isbn13: isbn13.clone(),
        isbn10: s(b, "isbn10").or_else(|| s(b, "isbn")),
        language: s(b, "language").as_deref().and_then(language),
        about: s(b, "synopsis")
            .or_else(|| s(b, "overview"))
            .as_deref()
            .and_then(about),
        edition: s(b, "edition"),
        tags: tags(strings(b, "subjects")),
        ..Default::default()
    };
    Some(Candidate {
        source: Source::Isbndb,
        source_id: isbn13.clone().unwrap_or_else(|| m.title.clone()),
        link: isbn13.map(|i| format!("https://isbndb.com/book/{i}")),
        cover_url: s(b, "image"),
        metadata: m,
        score: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fixture, FakeHttp};

    #[test]
    fn reads_books_and_sends_the_key() {
        let v: Value = serde_json::from_str(&fixture("isbndb_book.json")).unwrap();
        let c = parse_book(&v["book"]).unwrap();
        assert_eq!(c.metadata.title, "Linear Algebra Done Right");
        assert_eq!(c.metadata.authors, ["John Smith"]);
        assert_eq!(c.metadata.year, Some(2015));
        assert_eq!(c.metadata.pages, Some(340));
        assert_eq!(
            c.cover_url.as_deref(),
            Some("https://images.isbndb.com/covers/61/57/9780306406157.jpg")
        );

        let http = FakeHttp::default().status("api2.isbndb.com", 401);
        let err = by_isbn(&http, "k", "9780306406157").unwrap_err();
        assert!(err.contains("API key"), "{err}");
    }
}
