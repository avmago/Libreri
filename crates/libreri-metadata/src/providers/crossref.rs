//! Crossref (api.crossref.org): papers, chapters and books with a DOI.

use super::{s, strings, text};
use crate::http::{enc, json};
use crate::normalise::{about, language, person, tags};
use crate::{Candidate, Http, Source};
use libreri_core::{BookMetadata, ContentType};
use serde_json::Value;

const BASE: &str = "https://api.crossref.org/works";

pub fn by_doi(http: &dyn Http, doi: &str) -> Result<Vec<Candidate>, String> {
    let v = json(http.get(&format!("{BASE}/{}", enc_doi(doi)), &[])?)?;
    Ok(v.get("message")
        .and_then(parse_work)
        .map(|mut c| {
            c.score = 1.0;
            c
        })
        .into_iter()
        .collect())
}

pub fn search(
    http: &dyn Http,
    title: &str,
    author: Option<&str>,
) -> Result<Vec<Candidate>, String> {
    let mut url = format!("{BASE}?query.bibliographic={}&rows=6", enc(title));
    if let Some(a) = author {
        url.push_str(&format!("&query.author={}", enc(a)));
    }
    let v = json(http.get(&url, &[])?)?;
    Ok(v.pointer("/message/items")
        .and_then(Value::as_array)
        .map(|items| items.iter().filter_map(parse_work).collect())
        .unwrap_or_default())
}

/// A DOI in a URL path: its slashes stay, everything else is encoded.
pub(crate) fn enc_doi(doi: &str) -> String {
    doi.split('/').map(enc).collect::<Vec<_>>().join("/")
}

pub(crate) fn content_type(kind: &str) -> ContentType {
    match kind {
        "journal-article" => ContentType::ResearchPaper,
        "proceedings-article" => ContentType::ConferencePaper,
        "posted-content" => ContentType::Preprint,
        "dissertation" => ContentType::Thesis,
        "report" | "report-component" => ContentType::TechnicalReport,
        "standard" => ContentType::Standard,
        "reference-book" | "reference-entry" => ContentType::Reference,
        "monograph" | "book" | "edited-book" | "book-chapter" | "book-part" | "book-section" => {
            ContentType::Book
        }
        _ => ContentType::Article,
    }
}

fn people(v: &Value, key: &str) -> Vec<String> {
    v.get(key)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|p| match (s(p, "given"), s(p, "family")) {
                    (Some(g), Some(f)) => Some(format!("{g} {f}")),
                    (None, Some(f)) => Some(f),
                    _ => s(p, "name").map(|n| person(&n)),
                })
                .collect()
        })
        .unwrap_or_default()
}

fn first(v: &Value, key: &str) -> Option<String> {
    strings(v, key).into_iter().next()
}

fn issued_year(v: &Value) -> Option<i32> {
    ["issued", "published-print", "published-online", "created"]
        .iter()
        .find_map(|k| v.get(k)?.pointer("/date-parts/0/0")?.as_i64())
        .map(|y| y as i32)
}

pub(crate) fn parse_work(v: &Value) -> Option<Candidate> {
    let doi = s(v, "DOI")?.to_lowercase();
    let kind = s(v, "type").unwrap_or_default();
    let isbns = strings(v, "ISBN");
    let clean_isbn = |len: usize| {
        isbns
            .iter()
            .map(|i| i.replace('-', ""))
            .find(|i| i.len() == len)
    };
    let is_book = content_type(&kind) == ContentType::Book;
    let container = first(v, "container-title");
    let m = BookMetadata {
        title: first(v, "title").unwrap_or_default(),
        subtitle: first(v, "subtitle"),
        authors: people(v, "author"),
        contributors: people(v, "editor"),
        about: s(v, "abstract").as_deref().and_then(about),
        year: issued_year(v),
        publisher: s(v, "publisher"),
        isbn13: clean_isbn(13),
        isbn10: clean_isbn(10),
        language: s(v, "language").as_deref().and_then(language),
        content_type: content_type(&kind),
        doi: Some(doi.clone()),
        journal: if is_book { None } else { container.clone() },
        series: if is_book { container } else { None },
        volume: text(v, "volume"),
        issue: text(v, "issue"),
        tags: tags(strings(v, "subject")),
        url: Some(format!("https://doi.org/{doi}")),
        ..Default::default()
    };
    Some(Candidate {
        source: Source::Crossref,
        link: Some(format!("https://doi.org/{doi}")),
        source_id: doi,
        metadata: m,
        cover_url: None,
        score: 0.0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fixture;

    #[test]
    fn reads_works() {
        let v: Value = serde_json::from_str(&fixture("crossref_work.json")).unwrap();
        let c = parse_work(&v["message"]).unwrap();
        let m = &c.metadata;
        assert_eq!(m.title, "Deep learning");
        assert_eq!(
            m.authors,
            ["Yann LeCun", "Yoshua Bengio", "Geoffrey Hinton"]
        );
        assert_eq!(m.journal.as_deref(), Some("Nature"));
        assert_eq!(m.volume.as_deref(), Some("521"));
        assert_eq!(m.issue.as_deref(), Some("7553"));
        assert_eq!(m.year, Some(2015));
        assert_eq!(m.content_type, ContentType::ResearchPaper);
        assert_eq!(m.doi.as_deref(), Some("10.1038/nature14539"));
        assert_eq!(
            m.about.as_deref(),
            Some("Deep learning allows computational models to learn.")
        );
        assert_eq!(m.tags, ["Multidisciplinary"]);
        assert_eq!(content_type("posted-content"), ContentType::Preprint);
        assert_eq!(enc_doi("10.1000/a b"), "10.1000/a%20b");
    }
}
