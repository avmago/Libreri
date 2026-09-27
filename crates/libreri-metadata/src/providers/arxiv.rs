//! arXiv (export.arxiv.org): preprints by id and by title. Answers are
//! Atom feeds.

use crate::http::enc;
use crate::{Candidate, Http, Source};
use libreri_core::{BookMetadata, ContentType};
use libreri_formats::xml::{self, collapse, Element};

const BASE: &str = "https://export.arxiv.org/api/query";

pub fn by_id(http: &dyn Http, id: &str) -> Result<Vec<Candidate>, String> {
    let body = fetch(http, &format!("{BASE}?id_list={}", enc(id)))?;
    let mut c = parse(&body);
    c.retain(|c| c.metadata.arxiv_id.as_deref() == Some(id));
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
    let mut q = format!("ti:%22{}%22", enc(title));
    if let Some(a) = author {
        let last = a.split_whitespace().last().unwrap_or(a);
        q.push_str(&format!("+AND+au:{}", enc(last)));
    }
    let body = fetch(http, &format!("{BASE}?search_query={q}&max_results=6"))?;
    Ok(parse(&body))
}

fn fetch(http: &dyn Http, url: &str) -> Result<String, String> {
    let res = http.get(url, &[])?;
    if !res.ok() {
        return Err(format!("it answered with an error ({})", res.status));
    }
    String::from_utf8(res.body).map_err(|_| "it sent an answer that could not be read".into())
}

/// The broad subject of an arXiv category ("cs.LG" → "Computer Science").
fn field(category: &str) -> Option<&'static str> {
    let archive = category.split('.').next()?;
    Some(match archive {
        "cs" => "Computer Science",
        "math" => "Mathematics",
        "stat" => "Statistics",
        "eess" => "Electrical Engineering",
        "econ" => "Economics",
        "q-bio" => "Quantitative Biology",
        "q-fin" => "Quantitative Finance",
        "astro-ph" => "Physics/Astrophysics",
        "cond-mat" => "Physics/Condensed Matter",
        "quant-ph" => "Physics/Quantum Physics",
        "hep-th" | "hep-ph" | "hep-ex" | "hep-lat" => "Physics/High Energy Physics",
        "gr-qc" => "Physics/General Relativity",
        "nlin" => "Physics/Nonlinear Sciences",
        "nucl-th" | "nucl-ex" => "Physics/Nuclear Physics",
        "physics" | "math-ph" => "Physics",
        _ => return None,
    })
}

/// A few common subject classes, as readable tags.
fn class_name(category: &str) -> Option<&'static str> {
    Some(match category {
        "cs.AI" => "Artificial intelligence",
        "cs.CL" => "Natural language processing",
        "cs.CV" => "Computer vision",
        "cs.LG" | "stat.ML" => "Machine learning",
        "cs.CR" => "Cryptography and security",
        "cs.DS" => "Algorithms",
        "cs.PL" => "Programming languages",
        "cs.RO" => "Robotics",
        "cs.SE" => "Software engineering",
        "cs.DB" => "Databases",
        "cs.DC" => "Distributed computing",
        "cs.NE" => "Neural networks",
        "cs.HC" => "Human-computer interaction",
        "math.PR" => "Probability",
        "math.NT" => "Number theory",
        "math.AG" => "Algebraic geometry",
        "math.CO" => "Combinatorics",
        "math.OC" => "Optimization",
        "quant-ph" => "Quantum physics",
        _ => return None,
    })
}

fn text(e: &Element, name: &str) -> Option<String> {
    e.kids(name)
        .next()
        .map(|x| collapse(&x.text_deep()))
        .filter(|t| !t.is_empty())
}

pub(crate) fn parse(body: &str) -> Vec<Candidate> {
    let feed = xml::parse(body);
    feed.all("entry")
        .into_iter()
        .filter_map(|e| {
            let link = text(e, "id")?;
            let id = libreri_formats::find_arxiv(&link)?;
            let primary = e
                .kids("primary_category")
                .next()
                .and_then(|c| c.attr("term"))
                .map(str::to_owned);
            let classes: Vec<String> = e
                .kids("category")
                .filter_map(|c| c.attr("term"))
                .map(str::to_owned)
                .collect();
            let doi = text(e, "doi");
            let m = BookMetadata {
                title: text(e, "title").unwrap_or_default(),
                authors: e.kids("author").filter_map(|a| text(a, "name")).collect(),
                about: text(e, "summary"),
                year: text(e, "published")
                    .as_deref()
                    .and_then(crate::normalise::year),
                content_type: if doi.is_some() {
                    ContentType::ResearchPaper
                } else {
                    ContentType::Preprint
                },
                journal: text(e, "journal_ref"),
                doi: doi.map(|d| d.to_lowercase()),
                arxiv_id: Some(id.clone()),
                tags: crate::normalise::tags(
                    classes
                        .iter()
                        .filter_map(|c| class_name(c))
                        .map(String::from),
                ),
                categories: primary
                    .as_deref()
                    .or(classes.first().map(String::as_str))
                    .and_then(field)
                    .map(String::from)
                    .into_iter()
                    .collect(),
                url: Some(format!("https://arxiv.org/abs/{id}")),
                ..Default::default()
            };
            Some(Candidate {
                source: Source::Arxiv,
                link: Some(format!("https://arxiv.org/abs/{id}")),
                source_id: id,
                metadata: m,
                cover_url: None,
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
    fn reads_atom_entries() {
        let c = parse(&fixture("arxiv_entry.xml"));
        assert_eq!(c.len(), 1);
        let m = &c[0].metadata;
        assert_eq!(c[0].source_id, "1706.03762");
        assert_eq!(m.title, "Attention Is All You Need");
        assert_eq!(m.authors, ["Ashish Vaswani", "Noam Shazeer"]);
        assert_eq!(m.year, Some(2017));
        assert_eq!(m.content_type, ContentType::Preprint);
        assert_eq!(m.categories, ["Computer Science"]);
        assert_eq!(m.tags, ["Natural language processing", "Machine learning"]);
        assert!(m
            .about
            .as_deref()
            .unwrap()
            .starts_with("The dominant sequence"));
        assert!(parse("<feed></feed>").is_empty());
        assert!(parse("not xml").is_empty());
    }
}
