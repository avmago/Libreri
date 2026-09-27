//! OpenAlex (api.openalex.org): scholarly works, with topics that become
//! categories.

use super::{s, text};
use crate::http::{enc, json};
use crate::normalise::{language, tags};
use crate::{Candidate, Http, Source};
use libreri_core::{BookMetadata, ContentType};
use serde_json::Value;

const BASE: &str = "https://api.openalex.org/works";

pub fn by_doi(http: &dyn Http, doi: &str) -> Result<Vec<Candidate>, String> {
    let v = json(http.get(
        &format!("{BASE}/doi:{}", super::crossref::enc_doi(doi)),
        &[],
    )?)?;
    Ok(parse_work(&v)
        .map(|mut c| {
            c.score = 1.0;
            c
        })
        .into_iter()
        .collect())
}

pub fn search(http: &dyn Http, title: &str) -> Result<Vec<Candidate>, String> {
    let v = json(http.get(&format!("{BASE}?search={}&per-page=6", enc(title)), &[])?)?;
    Ok(v.get("results")
        .and_then(Value::as_array)
        .map(|r| r.iter().filter_map(parse_work).collect())
        .unwrap_or_default())
}

/// OpenAlex sends abstracts as {word: [positions]}; this puts them back in
/// order.
fn abstract_text(index: &Value) -> Option<String> {
    let obj = index.as_object()?;
    let mut words: Vec<(u64, &str)> = obj
        .iter()
        .flat_map(|(w, pos)| {
            pos.as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_u64)
                .map(move |p| (p, w.as_str()))
        })
        .collect();
    words.sort_unstable();
    let text = words.iter().map(|(_, w)| *w).collect::<Vec<_>>().join(" ");
    (text.len() >= 20).then_some(text)
}

fn content_type(kind: &str, source_type: Option<&str>) -> ContentType {
    match (kind, source_type) {
        ("article", Some("conference")) => ContentType::ConferencePaper,
        ("article", Some("repository")) | ("preprint", _) => ContentType::Preprint,
        ("article", _) | ("review", _) | ("letter", _) => ContentType::ResearchPaper,
        ("dissertation", _) => ContentType::Thesis,
        ("report", _) => ContentType::TechnicalReport,
        ("standard", _) => ContentType::Standard,
        ("book", _) | ("book-chapter", _) | ("monograph", _) => ContentType::Book,
        ("reference-entry", _) => ContentType::Reference,
        _ => ContentType::Article,
    }
}

pub(crate) fn parse_work(v: &Value) -> Option<Candidate> {
    let id = s(v, "id")?;
    let source = v.pointer("/primary_location/source");
    let source_type = source.and_then(|x| x.get("type")).and_then(Value::as_str);
    let doi = s(v, "doi").map(|d| d.trim_start_matches("https://doi.org/").to_lowercase());
    let authors = v
        .get("authorships")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(|x| x.get("author").and_then(|au| s(au, "display_name")))
                .collect()
        })
        .unwrap_or_default();
    // The main topic's field and subfield make a category path.
    let category = v.get("primary_topic").and_then(|t| {
        let field = t.get("field").and_then(|f| s(f, "display_name"))?;
        Some(match t.get("subfield").and_then(|f| s(f, "display_name")) {
            Some(sub) => format!("{field}/{sub}"),
            None => field,
        })
    });
    let keywords: Vec<String> = v
        .get("keywords")
        .and_then(Value::as_array)
        .map(|k| k.iter().filter_map(|x| s(x, "display_name")).collect())
        .unwrap_or_default();
    let arxiv = v
        .pointer("/ids/arxiv")
        .and_then(Value::as_str)
        .and_then(libreri_formats::find_arxiv);
    let biblio = v.get("biblio").cloned().unwrap_or(Value::Null);
    let m = BookMetadata {
        title: s(v, "title")
            .or_else(|| s(v, "display_name"))
            .unwrap_or_default(),
        authors,
        about: v.get("abstract_inverted_index").and_then(abstract_text),
        year: v
            .get("publication_year")
            .and_then(Value::as_i64)
            .map(|y| y as i32),
        language: s(v, "language").as_deref().and_then(language),
        content_type: content_type(&s(v, "type").unwrap_or_default(), source_type),
        journal: source.and_then(|x| s(x, "display_name")),
        volume: text(&biblio, "volume"),
        issue: text(&biblio, "issue"),
        doi: doi.clone(),
        arxiv_id: arxiv,
        tags: tags(keywords),
        categories: category.into_iter().collect(),
        url: doi.as_ref().map(|d| format!("https://doi.org/{d}")),
        ..Default::default()
    };
    Some(Candidate {
        source: Source::OpenAlex,
        link: Some(id.clone()),
        source_id: id.trim_start_matches("https://openalex.org/").to_owned(),
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
        let v: Value = serde_json::from_str(&fixture("openalex_work.json")).unwrap();
        let c = parse_work(&v).unwrap();
        let m = &c.metadata;
        assert_eq!(c.source_id, "W1901616594");
        assert_eq!(m.title, "Deep learning");
        assert_eq!(m.authors, ["Yann LeCun", "Yoshua Bengio"]);
        assert_eq!(
            m.about.as_deref(),
            Some("Deep learning allows computational models to learn")
        );
        assert_eq!(m.categories, ["Computer Science/Artificial Intelligence"]);
        assert_eq!(m.tags, ["Deep learning", "Representation learning"]);
        assert_eq!(m.journal.as_deref(), Some("Nature"));
        assert_eq!(m.content_type, ContentType::ResearchPaper);
        assert_eq!(m.doi.as_deref(), Some("10.1038/nature14539"));
        assert_eq!(m.volume.as_deref(), Some("521"));
    }
}
