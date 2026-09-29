//! Fetching feeds (with "not modified" answers), finding a site's feed,
//! and downloading entries.

use crate::parse::{self, Parsed};
use regex::Regex;
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

/// Feeds larger than this are refused.
const MAX_FEED: u64 = 16 * 1024 * 1024;
/// Pages read for an offline copy.
const MAX_PAGE: u64 = 8 * 1024 * 1024;
/// PDFs larger than this are not downloaded.
pub const MAX_PDF: u64 = 300 * 1024 * 1024;

pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_global(Some(Duration::from_secs(90)))
        .max_redirects(8)
        .user_agent(concat!(
            "Mozilla/5.0 (compatible; Libreri/",
            env!("CARGO_PKG_VERSION"),
            "; feed reader)"
        ))
        .build()
        .into()
}

/// Downloads of PDFs can take longer.
fn slow_agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(600)))
        .max_redirects(8)
        .user_agent(concat!(
            "Mozilla/5.0 (compatible; Libreri/",
            env!("CARGO_PKG_VERSION"),
            "; feed reader)"
        ))
        .build()
        .into()
}

fn error(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(404) => "the address was not found (404)".into(),
        ureq::Error::StatusCode(c) => format!("the site answered with an error ({c})"),
        ureq::Error::HostNotFound => {
            "the site could not be reached; check the internet connection".into()
        }
        ureq::Error::Timeout(_) => "the site took too long to answer".into(),
        other => format!("the site could not be reached: {other}"),
    }
}

/// What a feed's site said.
pub struct Response {
    pub bytes: Vec<u8>,
    pub content_type: String,
    /// The address after redirects.
    pub url: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

fn get(
    agent: &ureq::Agent,
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
    limit: u64,
) -> Result<Option<Response>, String> {
    let mut req = agent.get(url);
    if let Some(e) = etag {
        req = req.header("If-None-Match", e);
    }
    if let Some(l) = last_modified {
        req = req.header("If-Modified-Since", l);
    }
    let mut res = req.call().map_err(error)?;
    if res.status().as_u16() == 304 {
        return Ok(None);
    }
    let header = |name: &str| {
        res.headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned)
    };
    let content_type = header("content-type")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let etag = header("etag");
    let last_modified = header("last-modified");
    let final_url = {
        use ureq::ResponseExt;
        res.get_uri().to_string()
    };
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("the download stopped: {e}"))?;
    if bytes.len() as u64 > limit {
        return Err("the file is too large".into());
    }
    Ok(Some(Response {
        bytes,
        content_type,
        url: final_url,
        etag,
        last_modified,
    }))
}

/// A feed read again: `None` when it has not changed since the last time.
pub struct Update {
    pub parsed: Parsed,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

/// Reads a feed, asking the site to answer "not modified" when nothing
/// changed since `etag` / `last_modified`.
pub fn read(
    url: &str,
    etag: Option<&str>,
    last_modified: Option<&str>,
) -> Result<Option<Update>, String> {
    let agent = agent();
    let Some(res) = get(&agent, url, etag, last_modified, MAX_FEED)? else {
        return Ok(None);
    };
    let parsed = parse::parse(&res.bytes, &res.url)?;
    Ok(Some(Update {
        parsed,
        etag: res.etag,
        last_modified: res.last_modified,
    }))
}

/// A feed found for what was typed.
pub struct Found {
    pub url: String,
    pub parsed: Parsed,
}

/// Makes a web address of what was typed: "example.org" or
/// "feed://example.org/rss" become https addresses.
pub fn normalise(input: &str) -> Result<String, String> {
    let s = input.trim();
    if s.is_empty() {
        return Err("type or paste an address".into());
    }
    let s = if let Some(rest) = s.strip_prefix("feed://") {
        format!("https://{rest}")
    } else if let Some(rest) = s.strip_prefix("feed:") {
        rest.to_owned()
    } else if s.contains("://") {
        s.to_owned()
    } else {
        format!("https://{s}")
    };
    let u = url::Url::parse(&s).map_err(|_| "that is not a web address".to_owned())?;
    if u.scheme() != "http" && u.scheme() != "https" {
        return Err("only http and https addresses can be followed".into());
    }
    if u.host_str().is_none() {
        return Err("that is not a web address".into());
    }
    Ok(u.to_string())
}

/// Feed addresses a web page names (`<link rel="alternate" …>`), in order.
pub fn feed_links(page: &str, base: &str) -> Vec<String> {
    static LINK: OnceLock<Regex> = OnceLock::new();
    static ATTR: OnceLock<Regex> = OnceLock::new();
    let link = LINK.get_or_init(|| Regex::new(r"(?is)<link\b[^>]*>").expect("regex"));
    let attr = ATTR.get_or_init(|| {
        Regex::new(r#"(?is)([a-z\-]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#).expect("regex")
    });
    let base = url::Url::parse(base).ok();
    let mut out = Vec::new();
    for tag in link.find_iter(page) {
        let mut rel = String::new();
        let mut kind = String::new();
        let mut href = String::new();
        for c in attr.captures_iter(tag.as_str()) {
            let v = c
                .get(2)
                .or(c.get(3))
                .or(c.get(4))
                .map_or("", |m| m.as_str())
                .to_owned();
            match c[1].to_ascii_lowercase().as_str() {
                "rel" => rel = v.to_ascii_lowercase(),
                "type" => kind = v.to_ascii_lowercase(),
                "href" => href = v.replace("&amp;", "&"),
                _ => {}
            }
        }
        let feed_type = kind.contains("rss") || kind.contains("atom") || kind.contains("feed+json");
        if rel.split_whitespace().any(|r| r == "alternate") && feed_type && !href.is_empty() {
            let abs = match &base {
                Some(b) => b.join(&href).map(|u| u.to_string()).unwrap_or(href),
                None => href,
            };
            if !out.contains(&abs) {
                out.push(abs);
            }
        }
    }
    out
}

/// Finds the feed for what was typed: a feed's address, or a site whose
/// pages name their feed, or a site with a feed at a usual place.
pub fn discover(input: &str) -> Result<Found, String> {
    let url = normalise(input)?;
    let agent = agent();
    let res = get(&agent, &url, None, None, MAX_FEED)?
        .ok_or_else(|| "the site gave no answer".to_owned())?;
    if let Ok(parsed) = parse::parse(&res.bytes, &res.url) {
        return Ok(Found {
            url: res.url,
            parsed,
        });
    }
    let page = String::from_utf8_lossy(&res.bytes);
    let mut tries = feed_links(&page, &res.url);
    if let Ok(base) = url::Url::parse(&res.url) {
        for path in [
            "/feed",
            "/rss",
            "/atom.xml",
            "/feed.xml",
            "/rss.xml",
            "/index.xml",
            "/feed/",
        ] {
            if let Ok(u) = base.join(path) {
                let u = u.to_string();
                if !tries.contains(&u) {
                    tries.push(u);
                }
            }
        }
    }
    for t in tries.iter().take(10) {
        if let Ok(Some(r)) = get(&agent, t, None, None, MAX_FEED) {
            if let Ok(parsed) = parse::parse(&r.bytes, &r.url) {
                return Ok(Found { url: r.url, parsed });
            }
        }
    }
    Err("no feed was found at that address. Paste the feed's own address (it often ends in /feed, /rss or .xml)".into())
}

/// What a download gave.
pub enum Download {
    Pdf(Vec<u8>),
    /// A page: its HTML and address (for an offline copy).
    Page {
        html: String,
        url: String,
    },
}

/// Downloads a PDF (when `pdf` is given, or the page turns out to be one)
/// or the entry's page.
pub fn download(pdf: Option<&str>, link: Option<&str>) -> Result<Download, String> {
    let slow = slow_agent();
    let mut last_error = None;
    if let Some(p) = pdf {
        match get(&slow, p, None, None, MAX_PDF) {
            Ok(Some(r)) if r.bytes.starts_with(b"%PDF") => return Ok(Download::Pdf(r.bytes)),
            Ok(_) => last_error = Some("the site did not give a PDF".to_owned()),
            Err(e) => last_error = Some(e),
        }
    }
    if let Some(l) = link {
        let r = get(&slow, l, None, None, MAX_PDF)?
            .ok_or_else(|| "the site gave no answer".to_owned())?;
        if r.bytes.starts_with(b"%PDF") {
            return Ok(Download::Pdf(r.bytes));
        }
        if r.bytes.len() as u64 > MAX_PAGE {
            return Err("the page is too large to keep".into());
        }
        return Ok(Download::Page {
            html: String::from_utf8_lossy(&r.bytes).into_owned(),
            url: r.url,
        });
    }
    Err(last_error.unwrap_or_else(|| "this item has nothing to download".into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalises_addresses() {
        assert_eq!(normalise("example.org").unwrap(), "https://example.org/");
        assert_eq!(
            normalise("feed://example.org/rss").unwrap(),
            "https://example.org/rss"
        );
        assert!(normalise("file:///etc/passwd").is_err());
        assert!(normalise("  ").is_err());
    }

    #[test]
    fn finds_feed_links_in_pages() {
        let page = r#"<html><head>
            <link rel="stylesheet" href="/s.css">
            <link rel="alternate" type="application/rss+xml" title="RSS" href="/feed.xml">
            <link type='application/atom+xml' rel='alternate' href='https://ex.org/atom'>
            <link rel="alternate" hreflang="fr" href="/fr/">
            </head></html>"#;
        assert_eq!(
            feed_links(page, "https://ex.org/blog/"),
            ["https://ex.org/feed.xml", "https://ex.org/atom"]
        );
    }
}
