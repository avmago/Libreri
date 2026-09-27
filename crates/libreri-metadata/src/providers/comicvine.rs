//! ComicVine (comicvine.gamespot.com): comic issues and series. Needs the
//! reader's own API key.

use super::{s, text};
use crate::http::{enc, json};
use crate::normalise::{about, year};
use crate::{Candidate, Http, Source};
use libreri_core::{BookMetadata, ContentType};
use serde_json::Value;

pub fn search(http: &dyn Http, key: &str, title: &str) -> Result<Vec<Candidate>, String> {
    let url = format!(
        "https://comicvine.gamespot.com/api/search/?api_key={}&format=json&resources=issue,volume&limit=8&query={}",
        enc(key),
        enc(title)
    );
    let v = json(http.get(&url, &[])?)?;
    if let Some(err) = s(&v, "error").filter(|e| e != "OK") {
        return Err(if err.contains("API Key") {
            "it refused the request; check the API key".into()
        } else {
            err
        });
    }
    Ok(parse(&v))
}

pub(crate) fn parse(v: &Value) -> Vec<Candidate> {
    let results = v
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    results
        .iter()
        .filter_map(|r| {
            let kind = s(r, "resource_type")?;
            let id = text(r, "id")?;
            let (title, series, number, date) = match kind.as_str() {
                "issue" => {
                    let volume = r.get("volume").and_then(|x| s(x, "name"));
                    let number = s(r, "issue_number");
                    // Issues often have no name of their own.
                    let title = match (s(r, "name"), &volume, &number) {
                        (Some(n), _, _) => n,
                        (None, Some(v), Some(n)) => format!("{v} #{n}"),
                        (None, Some(v), None) => v.clone(),
                        _ => return None,
                    };
                    (
                        title,
                        volume,
                        number.and_then(|n| n.parse::<f64>().ok()),
                        s(r, "cover_date"),
                    )
                }
                "volume" => (s(r, "name")?, None, None, s(r, "start_year")),
                _ => return None,
            };
            let description = s(r, "deck").or_else(|| s(r, "description"));
            let m = BookMetadata {
                title,
                about: description.as_deref().and_then(about),
                year: date.as_deref().and_then(year),
                publisher: r.get("publisher").and_then(|p| s(p, "name")),
                content_type: ContentType::Comic,
                series,
                series_number: number,
                url: s(r, "site_detail_url"),
                ..Default::default()
            };
            let cover = r
                .get("image")
                .and_then(|i| s(i, "original_url").or_else(|| s(i, "super_url")));
            Some(Candidate {
                source: Source::ComicVine,
                source_id: format!("{kind}/{id}"),
                link: s(r, "site_detail_url"),
                metadata: m,
                cover_url: cover,
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
    fn reads_issues_and_volumes() {
        let v: Value = serde_json::from_str(&fixture("comicvine_search.json")).unwrap();
        let c = parse(&v);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].metadata.title, "Star Rangers #1");
        assert_eq!(c[0].metadata.series.as_deref(), Some("Star Rangers"));
        assert_eq!(c[0].metadata.series_number, Some(1.0));
        assert_eq!(c[0].metadata.year, Some(1963));
        assert_eq!(c[0].metadata.content_type, ContentType::Comic);
        assert_eq!(c[1].source_id, "volume/42");
        assert_eq!(c[1].metadata.publisher.as_deref(), Some("Acme Comics"));
        assert!(c[0]
            .cover_url
            .as_deref()
            .unwrap()
            .starts_with("https://comicvine.gamespot.com/"));
    }
}
