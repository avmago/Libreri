//! Reading a feed (RSS 0.9x/1.0/2.0, Atom or JSON Feed) into entries.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

/// A feed as read: its title and site, and its entries.
#[derive(Debug, Clone, PartialEq)]
pub struct Parsed {
    pub title: String,
    /// The site the feed belongs to.
    pub site: Option<String>,
    /// The feed's picture (a podcast's artwork).
    pub image: Option<String>,
    /// Who makes it (a podcast's host or publisher).
    pub author: Option<String>,
    pub entries: Vec<FeedEntry>,
}

/// One entry of a feed.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct FeedEntry {
    /// Stays the same for this entry each time the feed is read.
    pub key: String,
    pub title: String,
    /// The entry's page.
    pub link: Option<String>,
    pub authors: Vec<String>,
    /// The abstract or summary, as plain text.
    pub summary: String,
    /// When it was published (RFC 3339).
    pub published: Option<String>,
    /// Topics given by the feed (arXiv: "cs.AI", "math.PR").
    pub topics: Vec<String>,
    /// A PDF of it, when there is one.
    pub pdf: Option<String>,
    pub doi: Option<String>,
    pub arxiv_id: Option<String>,
    /// arXiv: "new", "cross", "replace" or "replace-cross".
    pub announce: Option<String>,
    /// Podcasts: the episode's audio, its type and length (seconds).
    #[serde(default)]
    pub audio: Option<String>,
    #[serde(default)]
    pub audio_type: Option<String>,
    #[serde(default)]
    pub duration: Option<f64>,
    /// The episode's own picture.
    #[serde(default)]
    pub image: Option<String>,
    /// Podcasting 2.0: a transcript (address and type) and chapters.
    #[serde(default)]
    pub transcript: Option<String>,
    #[serde(default)]
    pub transcript_type: Option<String>,
    #[serde(default)]
    pub chapters: Option<String>,
}

/// Reads a feed. `url` resolves relative links.
pub fn parse(bytes: &[u8], url: &str) -> Result<Parsed, String> {
    let parser = feed_rs::parser::Builder::new()
        .base_uri(Some(url))
        // Entries without an id get an empty one; ours is made below, so
        // it is the same each time (feed-rs would make a random one).
        .id_generator(|_, _, _| String::new())
        .sanitize_content(true)
        .build();
    let feed = parser
        .parse(bytes)
        .map_err(|e| format!("this is not a feed Libreri can read ({e})"))?;
    let title = feed
        .title
        .as_ref()
        .map(|t| plain_text(&t.content))
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| host_of(url).unwrap_or_else(|| url.to_owned()));
    let site = feed
        .links
        .iter()
        .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate") && !is_feed_type(l))
        .map(|l| l.href.clone());
    let extra = podcast_tags(bytes);
    let entries = feed
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let mut out = entry(e);
            if let Some(x) = extra.get(i) {
                out.transcript = x.transcript.clone().map(|(u, _)| u);
                out.transcript_type = x.transcript.clone().map(|(_, t)| t);
                out.chapters = x.chapters.clone();
                if x.duration.is_some() {
                    out.duration = x.duration;
                }
            }
            out
        })
        .collect();
    let image = feed
        .logo
        .as_ref()
        .or(feed.icon.as_ref())
        .map(|i| i.uri.clone())
        .filter(|u| u.starts_with("http"));
    let author = feed
        .authors
        .first()
        .map(|p| plain_text(&p.name))
        .filter(|a| !a.is_empty());
    Ok(Parsed {
        title,
        site,
        image,
        author,
        entries,
    })
}

fn is_feed_type(l: &feed_rs::model::Link) -> bool {
    l.media_type
        .as_deref()
        .is_some_and(|t| t.contains("rss") || t.contains("atom") || t.contains("xml"))
}

pub(crate) fn host_of(url: &str) -> Option<String> {
    url::Url::parse(url)
        .ok()?
        .host_str()
        .map(|h| h.trim_start_matches("www.").to_owned())
}

fn entry(e: &feed_rs::model::Entry) -> FeedEntry {
    let title = e
        .title
        .as_ref()
        .map(|t| plain_text(&t.content))
        .unwrap_or_default();
    let link = e
        .links
        .iter()
        .find(|l| {
            l.rel.as_deref().is_none_or(|r| r == "alternate")
                && l.media_type.as_deref() != Some("application/pdf")
        })
        .or_else(|| e.links.first())
        .map(|l| l.href.clone());
    let mut authors: Vec<String> = e
        .authors
        .iter()
        .flat_map(|p| split_authors(&p.name))
        .collect();
    authors.dedup();
    let raw_summary = e
        .summary
        .as_ref()
        .map(|t| t.content.clone())
        .or_else(|| e.content.as_ref().and_then(|c| c.body.clone()))
        .unwrap_or_default();
    let mut summary = plain_text(&raw_summary);
    // arXiv's RSS: "arXiv:2409.01234v1 Announce Type: new Abstract: …"
    let mut announce = None;
    static ARXIV_HEAD: OnceLock<Regex> = OnceLock::new();
    let head = ARXIV_HEAD.get_or_init(|| {
        Regex::new(r"^arXiv:\S+\s+Announce Type:\s*(\S+)\s*(?:Abstract:\s*)?").expect("regex")
    });
    if let Some(c) = head.captures(&summary) {
        announce = Some(c[1].to_owned());
        summary = summary[c.get(0).map_or(0, |m| m.end())..].trim().to_owned();
    }
    if summary.chars().count() > 3000 {
        summary = summary.chars().take(3000).collect::<String>() + "…";
    }
    let topics: Vec<String> = e
        .categories
        .iter()
        .map(|c| c.term.trim().to_owned())
        .filter(|t| !t.is_empty() && t.len() < 80)
        .collect();
    let pdf = e
        .links
        .iter()
        .find(|l| {
            l.media_type.as_deref() == Some("application/pdf")
                || l.title.as_deref() == Some("pdf")
                || l.href.to_ascii_lowercase().ends_with(".pdf")
        })
        .map(|l| l.href.clone())
        .or_else(|| {
            e.media.iter().flat_map(|m| &m.content).find_map(|c| {
                let pdf = c
                    .content_type
                    .as_ref()
                    .is_some_and(|t| t.to_string().starts_with("application/pdf"));
                pdf.then(|| c.url.as_ref().map(|u| u.to_string())).flatten()
            })
        });
    let arxiv_id = arxiv_id_of(&e.id)
        .or_else(|| link.as_deref().and_then(arxiv_id_of))
        .or_else(|| e.links.iter().find_map(|l| arxiv_id_of(&l.href)));
    let pdf = pdf.or_else(|| {
        arxiv_id
            .as_ref()
            .map(|id| format!("https://arxiv.org/pdf/{id}"))
    });
    let doi = std::iter::once(e.id.as_str())
        .chain(e.links.iter().map(|l| l.href.as_str()))
        .chain(std::iter::once(raw_summary.as_str()))
        .find_map(doi_of)
        .or_else(|| link.as_deref().and_then(nature_doi));
    let published = e.published.or(e.updated).map(|d| d.to_rfc3339());
    let (audio, audio_type, duration, image) = audio_of(e);
    let key = if !e.id.trim().is_empty() {
        e.id.trim().to_owned()
    } else if let Some(l) = &link {
        l.clone()
    } else {
        format!("{title}|{}", published.clone().unwrap_or_default())
    };
    FeedEntry {
        key,
        title: if title.is_empty() {
            "Untitled".into()
        } else {
            title
        },
        link,
        authors,
        summary,
        published,
        topics,
        pdf,
        doi,
        arxiv_id,
        announce,
        audio,
        audio_type,
        duration,
        image,
        transcript: None,
        transcript_type: None,
        chapters: None,
    }
}

/// The audio of a podcast episode: an `audio/*` enclosure or media file,
/// or a link to one.
fn audio_of(
    e: &feed_rs::model::Entry,
) -> (Option<String>, Option<String>, Option<f64>, Option<String>) {
    let mut duration = None;
    let mut image = None;
    let mut audio: Option<(String, Option<String>)> = None;
    for m in &e.media {
        if duration.is_none() {
            duration = m.duration.map(|d| d.as_secs_f64());
        }
        if image.is_none() {
            image = m.thumbnails.first().map(|t| t.image.uri.clone());
        }
        for c in &m.content {
            let kind = c.content_type.as_ref().map(|t| t.to_string());
            let url = c.url.as_ref().map(|u| u.to_string());
            let is_audio = kind.as_deref().is_some_and(|k| k.starts_with("audio/"))
                || url.as_deref().is_some_and(|u| {
                    let u = u.to_ascii_lowercase();
                    let path = u.split(['?', '#']).next().unwrap_or("");
                    [".mp3", ".m4a", ".aac", ".ogg", ".opus", ".flac", ".m4b"]
                        .iter()
                        .any(|x| path.ends_with(x))
                });
            if audio.is_none() && is_audio {
                if let Some(u) = url {
                    audio = Some((u, kind));
                    if duration.is_none() {
                        duration = c.duration.map(|d| d.as_secs_f64());
                    }
                }
            }
        }
    }
    if audio.is_none() {
        audio = e
            .links
            .iter()
            .find(|l| {
                l.rel.as_deref() == Some("enclosure")
                    && l.media_type
                        .as_deref()
                        .is_some_and(|t| t.starts_with("audio/"))
            })
            .map(|l| (l.href.clone(), l.media_type.clone()));
    }
    let (audio, audio_type) = match audio {
        Some((u, t)) => (Some(u), t),
        None => (None, None),
    };
    (
        audio,
        audio_type,
        duration.filter(|d| *d > 0.0),
        image.filter(|u| u.starts_with("http")),
    )
}

/// Podcasting 2.0 tags feed-rs does not read, item by item.
#[derive(Debug, Default, Clone)]
struct PodcastTags {
    /// The best transcript: address and type.
    transcript: Option<(String, String)>,
    chapters: Option<String>,
    /// `itunes:duration`, read here: feed-rs takes "58:00" for 58 seconds.
    duration: Option<f64>,
}

/// "1:02:03", "58:00" or "3480" as seconds.
fn itunes_duration(v: &str) -> Option<f64> {
    let mut total = 0.0;
    for part in v.trim().split(':') {
        total = total * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    (total > 0.0).then_some(total)
}

/// Transcript types, most useful first.
fn transcript_rank(kind: &str) -> u8 {
    match kind {
        "text/vtt" => 0,
        "application/x-subrip" | "application/srt" | "text/srt" => 1,
        "application/json" => 2,
        "text/html" => 3,
        "text/plain" => 4,
        _ => 9,
    }
}

fn podcast_tags(bytes: &[u8]) -> Vec<PodcastTags> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_reader(bytes);
    let mut out = Vec::new();
    let mut cur: Option<PodcastTags> = None;
    let mut in_duration = false;
    let mut buf = Vec::new();
    while let Ok(ev) = reader.read_event_into(&mut buf) {
        match &ev {
            Event::Eof => break,
            Event::Start(e) if matches!(e.local_name().as_ref(), b"item" | b"entry") => {
                cur = Some(PodcastTags::default());
            }
            Event::End(e) if matches!(e.local_name().as_ref(), b"item" | b"entry") => {
                out.push(cur.take().unwrap_or_default());
            }
            Event::Start(e) if e.name().as_ref() == b"itunes:duration" => {
                in_duration = cur.is_some();
            }
            Event::End(e) if e.name().as_ref() == b"itunes:duration" => {
                in_duration = false;
            }
            Event::Text(t) if in_duration => {
                if let (Some(tags), v) = (cur.as_mut(), String::from_utf8_lossy(t.as_ref())) {
                    tags.duration = itunes_duration(&v);
                }
            }
            Event::Start(e) | Event::Empty(e) => {
                let Some(tags) = cur.as_mut() else {
                    buf.clear();
                    continue;
                };
                let name = e.name();
                let attr = |k: &[u8]| {
                    e.attributes().flatten().find_map(|a| {
                        (a.key.as_ref() == k)
                            .then(|| a.unescape_value().ok().map(|v| v.trim().to_owned()))
                            .flatten()
                    })
                };
                match name.as_ref() {
                    b"podcast:transcript" => {
                        if let Some(url) = attr(b"url").filter(|u| u.starts_with("http")) {
                            let kind = attr(b"type").unwrap_or_default().to_ascii_lowercase();
                            let better = tags
                                .transcript
                                .as_ref()
                                .is_none_or(|(_, k)| transcript_rank(&kind) < transcript_rank(k));
                            if better {
                                tags.transcript = Some((url, kind));
                            }
                        }
                    }
                    b"podcast:chapters" => {
                        if let Some(url) = attr(b"url").filter(|u| u.starts_with("http")) {
                            tags.chapters = Some(url);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        buf.clear();
    }
    out
}

/// "A. Smith, B. Jones and C. Lee" as three names (arXiv's RSS gives all
/// authors in one `dc:creator`).
fn split_authors(names: &str) -> Vec<String> {
    let names = plain_text(names);
    if !names.contains(',') && !names.contains(" and ") {
        return vec![names].into_iter().filter(|n| !n.is_empty()).collect();
    }
    names
        .split(',')
        .flat_map(|p| p.split(" and "))
        .map(|p| p.trim().to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// An arXiv id in an address or `oai:arXiv.org:…` id, without the version.
pub fn arxiv_id_of(s: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(
            r"(?i)(?:arxiv\.org/(?:abs|pdf)/|oai:arxiv\.org:|arxiv:)([a-z\-]+(?:\.[A-Z]{2})?/\d{7}|\d{4}\.\d{4,5})(?:v\d+)?",
        )
        .expect("regex")
    });
    re.captures(s).map(|c| c[1].to_owned())
}

/// A DOI written in an address or text.
pub fn doi_of(s: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r#"\b(10\.\d{4,9}/[^\s"<>&]+)"#).expect("regex"));
    re.captures(s)
        .map(|c| c[1].trim_end_matches(['.', ',', ';', ')', ']']).to_owned())
}

/// Nature's articles are at `nature.com/articles/<id>`, and their DOI is
/// `10.1038/<id>`.
fn nature_doi(link: &str) -> Option<String> {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"^https?://(?:www\.)?nature\.com/articles/([A-Za-z0-9.\-]+)").expect("regex")
    });
    re.captures(link).map(|c| format!("10.1038/{}", &c[1]))
}

/// Text without tags, entities decoded and spaces tidied.
pub fn plain_text(html: &str) -> String {
    static TAG: OnceLock<Regex> = OnceLock::new();
    let tag = TAG.get_or_init(|| Regex::new(r"(?s)<!--.*?-->|<[^>]*>").expect("regex"));
    let breaks = html
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n")
        .replace("</p>", "\n\n");
    let text = decode_entities(&tag.replace_all(&breaks, " "));
    // Tidy spaces, keeping paragraph breaks.
    let mut out = String::with_capacity(text.len());
    for (i, para) in text
        .split("\n\n")
        .map(|p| p.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|p| !p.is_empty())
        .enumerate()
    {
        if i > 0 {
            out.push_str("\n\n");
        }
        out.push_str(&para);
    }
    out
}

fn decode_entities(s: &str) -> String {
    if !s.contains('&') {
        return s.to_owned();
    }
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"&(#x[0-9a-fA-F]+|#\d+|[a-zA-Z]+);").expect("regex"));
    re.replace_all(s, |c: &regex::Captures| {
        let name = &c[1];
        let ch = if let Some(hex) = name.strip_prefix("#x").or(name.strip_prefix("#X")) {
            u32::from_str_radix(hex, 16).ok().and_then(char::from_u32)
        } else if let Some(dec) = name.strip_prefix('#') {
            dec.parse().ok().and_then(char::from_u32)
        } else {
            match name {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some('\u{a0}'),
                "ndash" => Some('–'),
                "mdash" => Some('—'),
                "hellip" => Some('…'),
                "rsquo" => Some('’'),
                "lsquo" => Some('‘'),
                "rdquo" => Some('”'),
                "ldquo" => Some('“'),
                _ => None,
            }
        };
        ch.map_or_else(|| c[0].to_owned(), |ch| ch.to_string())
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const ARXIV_RSS: &str = r#"<?xml version='1.0' encoding='UTF-8'?>
<rss xmlns:arxiv="http://arxiv.org/schemas/atom" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:atom="http://www.w3.org/2005/Atom" version="2.0">
  <channel>
    <title>cs.AI updates on arXiv.org</title>
    <link>http://rss.arxiv.org/rss/cs.AI</link>
    <description>cs.AI updates on the arXiv.org e-print archive.</description>
    <item>
      <title>Learning to Reason with Small Models</title>
      <link>https://arxiv.org/abs/2409.01234</link>
      <description>arXiv:2409.01234v1 Announce Type: new
Abstract: We show that &lt;i&gt;small&lt;/i&gt; models can reason.</description>
      <guid isPermaLink="false">oai:arXiv.org:2409.01234v1</guid>
      <category>cs.AI</category>
      <category>cs.LG</category>
      <pubDate>Mon, 30 Sep 2024 00:00:00 -0400</pubDate>
      <arxiv:announce_type>new</arxiv:announce_type>
      <dc:creator>Ada Lovelace, Alan Turing and Grace Hopper</dc:creator>
    </item>
  </channel>
</rss>"#;

    #[test]
    fn reads_arxiv_rss() {
        let p = parse(ARXIV_RSS.as_bytes(), "https://rss.arxiv.org/rss/cs.AI").unwrap();
        assert_eq!(p.title, "cs.AI updates on arXiv.org");
        let e = &p.entries[0];
        assert_eq!(e.title, "Learning to Reason with Small Models");
        assert_eq!(e.key, "oai:arXiv.org:2409.01234v1");
        assert_eq!(e.authors, ["Ada Lovelace", "Alan Turing", "Grace Hopper"]);
        assert_eq!(e.summary, "We show that small models can reason.");
        assert_eq!(e.announce.as_deref(), Some("new"));
        assert_eq!(e.topics, ["cs.AI", "cs.LG"]);
        assert_eq!(e.arxiv_id.as_deref(), Some("2409.01234"));
        assert_eq!(e.pdf.as_deref(), Some("https://arxiv.org/pdf/2409.01234"));
        assert!(e.published.as_deref().unwrap().starts_with("2024-09-30"));
    }

    #[test]
    fn reads_atom() {
        let atom = r#"<?xml version="1.0" encoding="utf-8"?>
<feed xmlns="http://www.w3.org/2005/Atom">
  <title>A Blog</title>
  <link href="https://blog.example.org/"/>
  <link rel="self" type="application/atom+xml" href="https://blog.example.org/atom.xml"/>
  <entry>
    <title>Post &amp; more</title>
    <link href="/posts/1"/>
    <id>tag:blog.example.org,2024:1</id>
    <updated>2024-09-01T10:00:00Z</updated>
    <author><name>Sam</name></author>
    <summary type="html">&lt;p&gt;One.&lt;/p&gt;&lt;p&gt;Two, see doi:10.1000/xyz123.&lt;/p&gt;</summary>
    <link rel="enclosure" type="application/pdf" href="https://blog.example.org/paper.pdf"/>
  </entry>
</feed>"#;
        let p = parse(atom.as_bytes(), "https://blog.example.org/atom.xml").unwrap();
        assert_eq!(p.title, "A Blog");
        assert_eq!(p.site.as_deref(), Some("https://blog.example.org/"));
        let e = &p.entries[0];
        assert_eq!(e.title, "Post & more");
        assert_eq!(e.link.as_deref(), Some("https://blog.example.org/posts/1"));
        assert_eq!(e.summary, "One.\n\nTwo, see doi:10.1000/xyz123.");
        assert_eq!(e.doi.as_deref(), Some("10.1000/xyz123"));
        assert_eq!(e.pdf.as_deref(), Some("https://blog.example.org/paper.pdf"));
        assert_eq!(e.authors, ["Sam"]);
    }

    #[test]
    fn keys_stay_the_same_without_ids() {
        let rss = r#"<rss version="2.0"><channel><title>T</title>
            <item><title>No guid</title><link>https://ex.org/a</link></item>
            <item><title>Nothing</title></item></channel></rss>"#;
        let a = parse(rss.as_bytes(), "https://ex.org/feed").unwrap();
        let b = parse(rss.as_bytes(), "https://ex.org/feed").unwrap();
        assert_eq!(a.entries[0].key, "https://ex.org/a");
        assert_eq!(a.entries[0].key, b.entries[0].key);
        assert_eq!(a.entries[1].key, b.entries[1].key);
    }

    #[test]
    fn finds_ids() {
        assert_eq!(
            arxiv_id_of("http://arxiv.org/abs/hep-th/9901001v2").as_deref(),
            Some("hep-th/9901001")
        );
        assert_eq!(
            doi_of("https://doi.org/10.1038/s41586-024-07000-1.").as_deref(),
            Some("10.1038/s41586-024-07000-1")
        );
        assert_eq!(plain_text("a&#233;&nbsp;b &amp; <b>c</b>"), "aé b & c");
        assert_eq!(
            nature_doi("https://www.nature.com/articles/d41586-026-02972-w").as_deref(),
            Some("10.1038/d41586-026-02972-w")
        );
    }
}
