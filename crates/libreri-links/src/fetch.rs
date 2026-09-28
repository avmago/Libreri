//! Details of a link, fetched once when it is added: the title, author or
//! channel, length and picture, from the site's oEmbed answer and the
//! page's own tags. After that the link works offline.

use crate::address::{self, Address, KnownVideo};
use regex::Regex;
use serde::Serialize;
use std::io::Read;
use std::sync::OnceLock;
use std::time::Duration;

/// Pages larger than this are not read.
const MAX_PAGE: u64 = 6 * 1024 * 1024;
/// Pictures larger than this are left out.
pub(crate) const MAX_PICTURE: u64 = 4 * 1024 * 1024;

/// What a link points to.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LinkKind {
    /// A video that plays in the side panel.
    Video,
    /// A web page (opens in the browser; can be kept offline).
    Web,
}

/// Details of a link.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkDetails {
    pub url: String,
    pub kind: LinkKind,
    pub site: String,
    pub title: Option<String>,
    /// The channel or author.
    pub author: Option<String>,
    pub description: Option<String>,
    /// Length in seconds, when the site says.
    pub duration: Option<f64>,
    /// Where the video plays from (an https address for a frame).
    pub embed: Option<String>,
    /// YouTube or Vimeo: embedded by Libreri itself, with a start time.
    pub video: Option<KnownVideo>,
    /// A start time written in the address.
    pub start: Option<f64>,
}

/// Details and the page's picture, as fetched.
pub struct Fetched {
    pub details: LinkDetails,
    /// The picture (thumbnail) and its type, e.g. `image/jpeg`.
    pub picture: Option<(Vec<u8>, String)>,
    /// The page itself, for an offline copy (web pages only).
    pub page: Option<String>,
}

pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(15)))
        .timeout_global(Some(Duration::from_secs(60)))
        .max_redirects(8)
        .user_agent(concat!(
            "Mozilla/5.0 (compatible; Libreri/",
            env!("CARGO_PKG_VERSION"),
            ")"
        ))
        .build()
        .into()
}

fn error(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(c) => format!("the site answered with an error (HTTP {c})"),
        ureq::Error::HostNotFound => {
            "the site could not be reached; check the address and the internet connection".into()
        }
        ureq::Error::Timeout(_) => "the site took too long to answer".into(),
        other => format!("the site could not be reached: {other}"),
    }
}

/// A text answer (a page or JSON), with its type.
pub(crate) fn get_text(agent: &ureq::Agent, url: &str) -> Result<(String, String), String> {
    let mut res = agent.get(url).call().map_err(error)?;
    let kind = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(MAX_PAGE)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok((String::from_utf8_lossy(&bytes).into_owned(), kind))
}

/// A picture, if it is one and not too large.
pub(crate) fn get_picture(agent: &ureq::Agent, url: &str) -> Option<(Vec<u8>, String)> {
    let mut res = agent.get(url).call().ok()?;
    let kind = res
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_ascii_lowercase();
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(MAX_PICTURE + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    if bytes.len() as u64 > MAX_PICTURE {
        return None;
    }
    let kind = if kind.starts_with("image/") && kind != "image/svg+xml" {
        kind
    } else {
        sniff(&bytes)?.to_owned()
    };
    Some((bytes, kind))
}

/// The type of a picture from its first bytes (SVG is not accepted: it can
/// carry scripts).
pub(crate) fn sniff(b: &[u8]) -> Option<&'static str> {
    if b.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if b.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if b.starts_with(b"GIF8") {
        Some("image/gif")
    } else if b.len() > 12 && &b[..4] == b"RIFF" && &b[8..12] == b"WEBP" {
        Some("image/webp")
    } else if b.len() > 12 && &b[4..12] == b"ftypavif" {
        Some("image/avif")
    } else {
        None
    }
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#x27;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

/// The value of an attribute in a tag's text.
fn attr(tag: &str, name: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r#"(?is)\s([a-z:_-]+)\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#).expect("regex")
    });
    re.captures_iter(tag).find_map(|c| {
        c[1].eq_ignore_ascii_case(name).then(|| {
            unescape(
                c.get(2)
                    .or_else(|| c.get(3))
                    .or_else(|| c.get(4))
                    .map_or("", |m| m.as_str()),
            )
        })
    })
}

/// `<meta property|name="…" content="…">` values.
fn meta(html: &str, names: &[&str]) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?is)<meta\s[^>]*>").expect("regex"));
    for name in names {
        for m in re.find_iter(html) {
            let tag = m.as_str();
            let key = attr(tag, "property")
                .or_else(|| attr(tag, "name"))
                .or_else(|| attr(tag, "itemprop"));
            if key.is_some_and(|k| k.eq_ignore_ascii_case(name)) {
                if let Some(v) = attr(tag, "content").filter(|v| !v.trim().is_empty()) {
                    return Some(v.trim().to_owned());
                }
            }
        }
    }
    None
}

fn title_tag(html: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?is)<title[^>]*>(.*?)</title>").expect("regex"));
    let t = unescape(re.captures(html)?.get(1)?.as_str());
    let t = t.split_whitespace().collect::<Vec<_>>().join(" ");
    (!t.is_empty()).then_some(t)
}

/// The oEmbed address a page offers (`<link type="application/json+oembed">`).
fn oembed_link(html: &str, base: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?is)<link\s[^>]*>").expect("regex"));
    re.find_iter(html).find_map(|m| {
        let tag = m.as_str();
        let t = attr(tag, "type")?;
        if !t.eq_ignore_ascii_case("application/json+oembed") {
            return None;
        }
        absolute(base, &attr(tag, "href")?)
    })
}

pub(crate) fn absolute(base: &str, href: &str) -> Option<String> {
    let u = url::Url::parse(base).ok()?.join(href.trim()).ok()?;
    matches!(u.scheme(), "http" | "https").then(|| u.to_string())
}

/// ISO 8601 durations ("PT4M13S").
fn iso_duration(s: &str) -> Option<f64> {
    let s = s
        .trim()
        .strip_prefix("PT")
        .or_else(|| s.strip_prefix("P0DT"))?;
    let mut total = 0.0;
    let mut num = String::new();
    for c in s.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'H' | 'M' | 'S' => {
                let n: f64 = num.parse().ok()?;
                num.clear();
                total += n * match c {
                    'H' => 3600.0,
                    'M' => 60.0,
                    _ => 1.0,
                };
            }
            _ => return None,
        }
    }
    Some(total)
}

#[derive(Default)]
struct OEmbed {
    title: Option<String>,
    author: Option<String>,
    duration: Option<f64>,
    thumbnail: Option<String>,
    frame: Option<String>,
}

fn oembed(agent: &ureq::Agent, url: &str) -> Option<OEmbed> {
    let (text, _) = get_text(agent, url).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let s = |k: &str| {
        v.get(k)
            .and_then(|x| x.as_str())
            .map(|x| x.trim().to_owned())
            .filter(|x| !x.is_empty())
    };
    let html = s("html").unwrap_or_default();
    static SRC: OnceLock<Regex> = OnceLock::new();
    let src = SRC.get_or_init(|| {
        Regex::new(r#"(?is)<iframe[^>]*\ssrc\s*=\s*["']([^"']+)["']"#).expect("regex")
    });
    let frame = src
        .captures(&html)
        .map(|c| unescape(&c[1]))
        .map(|f| {
            if f.starts_with("//") {
                format!("https:{f}")
            } else {
                f
            }
        })
        .filter(|f| f.starts_with("https://"));
    Some(OEmbed {
        title: s("title"),
        author: s("author_name"),
        duration: v.get("duration").and_then(|d| d.as_f64()),
        thumbnail: s("thumbnail_url"),
        frame,
    })
}

fn known_oembed(video: &KnownVideo, url: &str) -> String {
    let enc: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
    match video {
        KnownVideo::Youtube { .. } => {
            format!("https://www.youtube.com/oembed?format=json&url={enc}")
        }
        KnownVideo::Vimeo { .. } => format!("https://vimeo.com/api/oembed.json?url={enc}"),
    }
}

/// Fetches a link's details. For web pages the page is kept for an
/// offline copy.
pub fn fetch(input: &str) -> Result<Fetched, String> {
    let a: Address = address::read(input)?;
    let agent = agent();
    if let Some(video) = &a.video {
        // Known sites: their oEmbed answer, and the page for the length.
        let o = oembed(&agent, &known_oembed(video, &a.url)).unwrap_or_default();
        let mut duration = o.duration;
        if duration.is_none() {
            if let KnownVideo::Youtube { .. } = video {
                duration = get_text(&agent, &address::watch_url(video, None))
                    .ok()
                    .and_then(|(page, _)| {
                        meta(&page, &["duration"])
                            .and_then(|d| iso_duration(&d))
                            .or_else(|| {
                                static LEN: OnceLock<Regex> = OnceLock::new();
                                let re = LEN.get_or_init(|| {
                                    Regex::new(r#""lengthSeconds"\s*:\s*"(\d+)""#).expect("regex")
                                });
                                re.captures(&page).and_then(|c| c[1].parse().ok())
                            })
                    });
            }
        }
        let thumbnail = o.thumbnail.clone().or_else(|| match video {
            KnownVideo::Youtube { id } => {
                Some(format!("https://i.ytimg.com/vi/{id}/hqdefault.jpg"))
            }
            KnownVideo::Vimeo { .. } => None,
        });
        let picture = thumbnail.and_then(|t| get_picture(&agent, &t));
        return Ok(Fetched {
            details: LinkDetails {
                embed: Some(address::embed_url(video, None)),
                kind: LinkKind::Video,
                site: a.site,
                title: o.title,
                author: o.author,
                description: None,
                duration,
                video: Some(video.clone()),
                start: a.start,
                url: a.url,
            },
            picture,
            page: None,
        });
    }
    let (page, content_type) = get_text(&agent, &a.url)?;
    let is_html = content_type.contains("html") || content_type.is_empty();
    let o = if is_html {
        oembed_link(&page, &a.url).and_then(|u| oembed(&agent, &u))
    } else {
        None
    };
    let o = o.unwrap_or_default();
    // A video from another site: its oEmbed frame, or the page's own
    // `og:video` player when that is a page (not a file).
    let og_player = meta(&page, &["og:video:secure_url", "og:video:url", "og:video"])
        .filter(|_| {
            meta(&page, &["og:video:type"]).is_some_and(|t| t.eq_ignore_ascii_case("text/html"))
        })
        .filter(|u| u.starts_with("https://"));
    let embed = o.frame.clone().or(og_player);
    let video = embed.is_some();
    let title = o
        .title
        .clone()
        .or_else(|| meta(&page, &["og:title", "twitter:title"]))
        .or_else(|| title_tag(&page));
    let picture_url = o
        .thumbnail
        .clone()
        .or_else(|| meta(&page, &["og:image:secure_url", "og:image", "twitter:image"]))
        .and_then(|p| absolute(&a.url, &p));
    let picture = picture_url.and_then(|p| get_picture(&agent, &p));
    let duration = o.duration.or_else(|| {
        meta(&page, &["video:duration", "og:video:duration"])
            .and_then(|d| d.parse().ok())
            .or_else(|| meta(&page, &["duration"]).and_then(|d| iso_duration(&d)))
    });
    Ok(Fetched {
        details: LinkDetails {
            kind: if video {
                LinkKind::Video
            } else {
                LinkKind::Web
            },
            site: meta(&page, &["og:site_name"]).unwrap_or(a.site),
            title,
            author: o
                .author
                .or_else(|| meta(&page, &["author", "article:author"])),
            description: meta(
                &page,
                &["og:description", "description", "twitter:description"],
            )
            .map(|d| d.chars().take(400).collect()),
            duration,
            embed,
            video: None,
            start: a.start,
            url: a.url,
        },
        picture,
        page: (is_html && !video).then_some(page),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_page_tags() {
        let html = r#"<html><head><title> The  Lighthouse &amp; Keeper </title>
            <meta property="og:site_name" content="Sea Stories">
            <meta content="A keeper's log." name="description">
            <link rel="alternate" type="application/json+oembed" href="/oembed?id=4">
            <meta itemprop="duration" content="PT4M13S"></head></html>"#;
        assert_eq!(title_tag(html).as_deref(), Some("The Lighthouse & Keeper"));
        assert_eq!(
            meta(html, &["og:site_name"]).as_deref(),
            Some("Sea Stories")
        );
        assert_eq!(
            meta(html, &["description"]).as_deref(),
            Some("A keeper's log.")
        );
        assert_eq!(
            oembed_link(html, "https://example.org/a/b").as_deref(),
            Some("https://example.org/oembed?id=4")
        );
        assert_eq!(
            meta(html, &["duration"]).and_then(|d| iso_duration(&d)),
            Some(253.0)
        );
    }

    #[test]
    fn knows_pictures() {
        assert_eq!(sniff(b"\x89PNG\r\n"), Some("image/png"));
        assert_eq!(sniff(b"<svg"), None);
    }
}

#[cfg(test)]
mod site_tests {
    use super::*;
    use tiny_http::{Header, Response, Server};

    /// A small site: a video page (oEmbed), an article with a picture.
    fn site() -> (String, std::thread::JoinHandle<()>) {
        let server = Server::http("127.0.0.1:0").unwrap();
        let base = format!(
            "http://127.0.0.1:{}",
            server.server_addr().to_ip().unwrap().port()
        );
        let b = base.clone();
        let t = std::thread::spawn(move || {
            for _ in 0..8 {
                let Ok(Some(req)) = server.recv_timeout(Duration::from_secs(5)) else {
                    break;
                };
                let (body, kind): (Vec<u8>, &str) = match req.url() {
                    "/talk" => (format!(r#"<html><head><title>A talk</title>
                        <link rel="alternate" type="application/json+oembed" href="{b}/oembed?u=talk">
                        <meta property="og:image" content="/thumb.png"></head><body>Video</body></html>"#).into_bytes(), "text/html"),
                    "/oembed?u=talk" => (br#"{"type":"video","title":"Keepers of the light","author_name":"Sea Channel","duration":754,
                        "html":"<iframe width=\"560\" src=\"https://video.example/embed/42\" allowfullscreen></iframe>"}"#.to_vec(), "application/json"),
                    "/thumb.png" | "/pics/tower.png" => (b"\x89PNG\r\n\x1a\nxx".to_vec(), "image/png"),
                    "/article" => (format!(r#"<html><head><title>Lighthouses | Sea Wiki</title>
                        <meta property="og:site_name" content="Sea Wiki"><meta name="description" content="All about lighthouses.">
                        </head><body><article><h1>Lighthouses</h1><p>{0}</p><img src="/pics/tower.png"><p>{0}</p><p>{0}</p></article></body></html>"#,
                        "A lighthouse is a tower that sends light to help ships at sea find their way along dangerous coasts. ".repeat(5)).into_bytes(), "text/html; charset=utf-8"),
                    _ => (b"no".to_vec(), "text/plain"),
                };
                let _ = req.respond(
                    Response::from_data(body)
                        .with_header(Header::from_bytes("Content-Type", kind).unwrap()),
                );
            }
        });
        (base, t)
    }

    #[test]
    fn fetches_videos_from_other_sites_and_pages() {
        let (base, _t) = site();
        let v = fetch(&format!("{base}/talk")).unwrap();
        assert_eq!(v.details.kind, LinkKind::Video);
        assert_eq!(
            v.details.embed.as_deref(),
            Some("https://video.example/embed/42")
        );
        assert_eq!(v.details.title.as_deref(), Some("Keepers of the light"));
        assert_eq!(v.details.author.as_deref(), Some("Sea Channel"));
        assert_eq!(v.details.duration, Some(754.0));
        assert_eq!(v.picture.as_ref().map(|p| p.1.as_str()), Some("image/png"));
        assert!(v.page.is_none());

        let w = fetch(&format!("{base}/article")).unwrap();
        assert_eq!(w.details.kind, LinkKind::Web);
        assert_eq!(w.details.site, "Sea Wiki");
        assert_eq!(w.details.title.as_deref(), Some("Lighthouses | Sea Wiki"));
        assert_eq!(
            w.details.description.as_deref(),
            Some("All about lighthouses.")
        );
        let page = w.page.unwrap();
        let html = crate::copy::make(
            &page,
            &crate::copy::Source {
                url: &w.details.url,
                title: None,
                site: "Sea Wiki",
                saved: "today",
            },
            crate::copy::web_pictures(),
        )
        .unwrap();
        assert!(html.contains("A lighthouse is a tower"));
        assert!(
            html.contains("data:image/png;base64,iVBORw0KGgp4eA=="),
            "{html}"
        );
    }
}
