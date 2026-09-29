//! Articles kept as Markdown: the readable part of the page, with the
//! item's details as front matter, so it can join the library as a book.

use crate::state::FeedItem;
use libreri_links::copy::Readable;

/// A front-matter value on one line, quoted.
fn value(s: &str) -> String {
    let one: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    // The library's reader of front matter does not unescape: keep it plain.
    format!("\"{}\"", one.replace('"', "'"))
}

/// The article as Markdown with front matter.
pub fn markdown(item: &FeedItem, article: &Readable, url: &str, saved: &str) -> String {
    let e = &item.entry;
    let title = if e.title.trim().is_empty() || e.title == "Untitled" {
        article.title.clone()
    } else {
        e.title.clone()
    };
    let mut authors = e.authors.clone();
    if authors.is_empty() {
        if let Some(b) = article
            .byline
            .as_deref()
            .map(str::trim)
            .filter(|b| !b.is_empty())
        {
            authors.push(
                b.trim_start_matches("By ")
                    .trim_start_matches("by ")
                    .to_owned(),
            );
        }
    }
    let mut fm = vec!["---".to_owned(), format!("title: {}", value(&title))];
    if !authors.is_empty() {
        fm.push("authors:".into());
        fm.extend(authors.iter().map(|a| format!("  - {}", value(a))));
    }
    if let Some(d) = &e.published {
        fm.push(format!("date: {}", value(d.get(..10).unwrap_or(d))));
    }
    fm.push(format!("publisher: {}", value(&item.source)));
    fm.push(format!("url: {}", value(url)));
    if let Some(d) = &e.doi {
        fm.push(format!("doi: {}", value(d)));
    }
    if !e.topics.is_empty() {
        fm.push(format!(
            "tags: [{}]",
            e.topics
                .iter()
                .map(|t| value(&t.replace(',', " ")))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    if !e.summary.is_empty() {
        let short: String = e.summary.chars().take(600).collect();
        fm.push(format!("description: {}", value(&short)));
    }
    fm.push(format!("saved: {}", value(saved)));
    fm.push("---".into());
    let body =
        htmd::convert(&article.body).unwrap_or_else(|_| crate::parse::plain_text(&article.body));
    let by = if authors.is_empty() {
        String::new()
    } else {
        format!("*{}*\n\n", authors.join(", "))
    };
    format!(
        "{}\n\n# {}\n\n{by}> Saved from [{}]({}) on {saved}.\n\n{}\n",
        fm.join("\n"),
        title.trim(),
        item.source.replace(['[', ']'], ""),
        url,
        body.trim()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parse::FeedEntry;

    #[test]
    fn writes_front_matter_and_body() {
        let item = FeedItem {
            id: "i1".into(),
            feed: "s1".into(),
            source: "A \"Blog\"".into(),
            entry: FeedEntry {
                key: "k".into(),
                title: "On lighthouses".into(),
                authors: vec!["Jane Smith".into()],
                published: Some("2026-09-28T10:00:00+00:00".into()),
                topics: vec!["sea, ships".into()],
                summary: "About keepers.".into(),
                ..Default::default()
            },
            found_at: String::new(),
            read: false,
            file: None,
            book: None,
            download_error: None,
        };
        let article = Readable {
            title: "Ignored".into(),
            byline: None,
            body: "<p>The <b>lamp</b> was lit.</p><img src=\"data:image/png;base64,AAAA\" alt=\"Tower\">".into(),
            dir: None,
            lang: None,
        };
        let md = markdown(
            &item,
            &article,
            "https://blog.example/1",
            "29 September 2026",
        );
        assert!(md.starts_with("---\ntitle: \"On lighthouses\"\nauthors:\n  - \"Jane Smith\"\ndate: \"2026-09-28\"\npublisher: \"A 'Blog'\""));
        assert!(md.contains("tags: [\"sea ships\"]"));
        assert!(md.contains("# On lighthouses"));
        assert!(md.contains("The **lamp** was lit."));
        assert!(md.contains("![Tower](data:image/png;base64,AAAA)"));
    }
}
