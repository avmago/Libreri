//! Semantic Scholar (api.semanticscholar.org): papers by DOI or arXiv id.

use super::{s, strings};
use crate::http::json;
use crate::normalise::tags;
use crate::{Candidate, Http, Source};
use libreri_core::{BookMetadata, ContentType};
use serde_json::Value;

const FIELDS: &str =
    "title,authors,year,venue,externalIds,abstract,fieldsOfStudy,s2FieldsOfStudy,publicationTypes,journal,url";

/// `id` is "DOI:10.…" or "ARXIV:1706.03762".
pub fn by_id(http: &dyn Http, id: &str) -> Result<Vec<Candidate>, String> {
    let (kind, rest) = id.split_once(':').unwrap_or(("", id));
    let path = format!("{kind}:{}", super::crossref::enc_doi(rest));
    let url = format!("https://api.semanticscholar.org/graph/v1/paper/{path}?fields={FIELDS}");
    let v = json(http.get(&url, &[])?)?;
    Ok(parse(&v)
        .map(|mut c| {
            c.score = 1.0;
            c
        })
        .into_iter()
        .collect())
}

fn content_type(types: &[String], has_arxiv: bool) -> ContentType {
    let has = |t: &str| types.iter().any(|x| x == t);
    if has("Conference") {
        ContentType::ConferencePaper
    } else if has("JournalArticle") || has("Review") {
        ContentType::ResearchPaper
    } else if has("Book") {
        ContentType::Book
    } else if has_arxiv {
        ContentType::Preprint
    } else {
        ContentType::ResearchPaper
    }
}

pub(crate) fn parse(v: &Value) -> Option<Candidate> {
    let id = s(v, "paperId")?;
    let ids = v.get("externalIds").cloned().unwrap_or(Value::Null);
    let arxiv = s(&ids, "ArXiv");
    let journal = v.get("journal").cloned().unwrap_or(Value::Null);
    let fields = strings(v, "fieldsOfStudy");
    let mut subjects = fields.clone();
    subjects.extend(
        v.get("s2FieldsOfStudy")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|f| s(f, "category")),
    );
    let m = BookMetadata {
        title: s(v, "title").unwrap_or_default(),
        authors: super::names(v, "authors", "name"),
        about: s(v, "abstract"),
        year: v.get("year").and_then(Value::as_i64).map(|y| y as i32),
        content_type: content_type(&strings(v, "publicationTypes"), arxiv.is_some()),
        journal: s(&journal, "name").or_else(|| s(v, "venue")),
        volume: s(&journal, "volume"),
        doi: s(&ids, "DOI").map(|d| d.to_lowercase()),
        arxiv_id: arxiv,
        tags: tags(subjects),
        categories: fields.into_iter().take(1).collect(),
        url: s(v, "url"),
        ..Default::default()
    };
    Some(Candidate {
        source: Source::SemanticScholar,
        link: s(v, "url"),
        source_id: id,
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
    fn reads_papers() {
        let v: Value = serde_json::from_str(&fixture("semantic_paper.json")).unwrap();
        let c = parse(&v).unwrap();
        let m = &c.metadata;
        assert_eq!(m.title, "Attention is All you Need");
        assert_eq!(m.authors.len(), 2);
        assert_eq!(m.arxiv_id.as_deref(), Some("1706.03762"));
        assert_eq!(m.content_type, ContentType::ConferencePaper);
        assert_eq!(m.categories, ["Computer Science"]);
        assert_eq!(m.tags, ["Computer Science"]);
        assert_eq!(m.year, Some(2017));
    }
}
