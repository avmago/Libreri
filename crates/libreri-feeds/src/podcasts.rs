//! Podcasts: finding shows (Apple's podcast search, or Podcast Index with
//! the reader's own key), transcripts and chapters, and downloading
//! episodes. Shows themselves are RSS feeds, read like any other feed.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

/// A show found by a search.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Show {
    pub title: String,
    pub author: Option<String>,
    /// The show's RSS feed.
    pub feed_url: String,
    pub artwork: Option<String>,
    pub categories: Vec<String>,
    pub episodes: Option<u32>,
    /// The newest episode (RFC 3339).
    pub latest: Option<String>,
    pub about: Option<String>,
}

/// A Podcast Index key and secret (free, from api.podcastindex.org).
#[derive(Debug, Clone)]
pub struct IndexKey {
    pub key: String,
    pub secret: String,
}

fn agent() -> ureq::Agent {
    crate::fetch::agent()
}

fn get_json(
    req: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
) -> Result<serde_json::Value, String> {
    let mut res = req.call().map_err(|e| match e {
        ureq::Error::StatusCode(401) => "the Podcast Index key or secret is not right".to_owned(),
        ureq::Error::StatusCode(c) => format!("the search service answered with an error ({c})"),
        ureq::Error::HostNotFound => {
            "the search service could not be reached; check the internet connection".into()
        }
        other => format!("the search service could not be reached: {other}"),
    })?;
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(8 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes)
        .map_err(|_| "the search service gave an answer Libreri cannot read".into())
}

fn text(v: &serde_json::Value, k: &str) -> Option<String> {
    v.get(k)
        .and_then(|x| x.as_str())
        .map(|s| s.trim().to_owned())
        .filter(|s| !s.is_empty())
}

/// Shows matching the words, from Apple's podcast directory (no key).
pub fn search_apple(query: &str) -> Result<Vec<Show>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    let mut u = url::Url::parse("https://itunes.apple.com/search").map_err(|e| e.to_string())?;
    u.query_pairs_mut()
        .append_pair("media", "podcast")
        .append_pair("entity", "podcast")
        .append_pair("limit", "30")
        .append_pair("term", q);
    let v = get_json(agent().get(u.as_str()))?;
    Ok(apple_shows(&v))
}

pub(crate) fn apple_shows(v: &serde_json::Value) -> Vec<Show> {
    v.get("results")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter_map(|r| {
            let feed_url = text(r, "feedUrl").filter(|u| u.starts_with("http"))?;
            Some(Show {
                title: text(r, "collectionName").unwrap_or_else(|| feed_url.clone()),
                author: text(r, "artistName"),
                feed_url,
                artwork: text(r, "artworkUrl600").or_else(|| text(r, "artworkUrl100")),
                categories: r
                    .get("genres")
                    .and_then(|g| g.as_array())
                    .into_iter()
                    .flatten()
                    .filter_map(|g| g.as_str().map(str::to_owned))
                    .filter(|g| g != "Podcasts")
                    .collect(),
                episodes: r
                    .get("trackCount")
                    .and_then(|n| n.as_u64())
                    .map(|n| n as u32),
                latest: text(r, "releaseDate"),
                about: None,
            })
        })
        .collect()
}

/// The headers Podcast Index asks for: the key, the time, and a SHA-1 of
/// key, secret and time.
pub(crate) fn index_headers(key: &IndexKey, now: u64) -> [(String, String); 3] {
    let auth = sha1_smol::Sha1::from(format!("{}{}{now}", key.key.trim(), key.secret.trim()))
        .digest()
        .to_string();
    [
        ("X-Auth-Key".into(), key.key.trim().to_owned()),
        ("X-Auth-Date".into(), now.to_string()),
        ("Authorization".into(), auth),
    ]
}

fn index_get(
    key: &IndexKey,
    path: &str,
    params: &[(&str, &str)],
) -> Result<serde_json::Value, String> {
    let mut u = url::Url::parse(&format!("https://api.podcastindex.org/api/1.0/{path}"))
        .map_err(|e| e.to_string())?;
    {
        let mut q = u.query_pairs_mut();
        for (k, v) in params {
            q.append_pair(k, v);
        }
    }
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let mut req = agent().get(u.as_str());
    for (k, v) in index_headers(key, now) {
        req = req.header(k, v);
    }
    get_json(req)
}

pub(crate) fn index_shows(v: &serde_json::Value) -> Vec<Show> {
    v.get("feeds")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter_map(|f| {
            let feed_url = text(f, "url").filter(|u| u.starts_with("http"))?;
            let latest = f
                .get("newestItemPubdate")
                .or_else(|| f.get("newestItemPublishTime"))
                .and_then(|n| n.as_i64())
                .filter(|n| *n > 0)
                .and_then(|n| chrono::DateTime::from_timestamp(n, 0))
                .map(|d| d.to_rfc3339());
            Some(Show {
                title: text(f, "title").unwrap_or_else(|| feed_url.clone()),
                author: text(f, "author").or_else(|| text(f, "ownerName")),
                feed_url,
                artwork: text(f, "artwork").or_else(|| text(f, "image")),
                categories: f
                    .get("categories")
                    .and_then(|c| c.as_object())
                    .map(|c| {
                        c.values()
                            .filter_map(|x| x.as_str().map(str::to_owned))
                            .collect()
                    })
                    .unwrap_or_default(),
                episodes: f
                    .get("episodeCount")
                    .and_then(|n| n.as_u64())
                    .map(|n| n as u32),
                latest,
                about: text(f, "description").map(|d| {
                    let d = crate::parse::plain_text(&d);
                    if d.chars().count() > 280 {
                        d.chars().take(280).collect::<String>() + "…"
                    } else {
                        d
                    }
                }),
            })
        })
        .collect()
}

/// Shows matching the words, from Podcast Index.
pub fn search_index(key: &IndexKey, query: &str) -> Result<Vec<Show>, String> {
    let q = query.trim();
    if q.is_empty() {
        return Ok(Vec::new());
    }
    Ok(index_shows(&index_get(
        key,
        "search/byterm",
        &[("q", q), ("max", "30")],
    )?))
}

/// Shows popular now on Podcast Index, optionally in a category.
pub fn trending(
    key: &IndexKey,
    category: Option<&str>,
    lang: Option<&str>,
) -> Result<Vec<Show>, String> {
    let mut params = vec![("max", "40")];
    if let Some(c) = category.filter(|c| !c.is_empty()) {
        params.push(("cat", c));
    }
    if let Some(l) = lang.filter(|l| !l.is_empty()) {
        params.push(("lang", l));
    }
    Ok(index_shows(&index_get(key, "podcasts/trending", &params)?))
}

/// Podcast Index's categories.
pub fn categories(key: &IndexKey) -> Result<Vec<String>, String> {
    let v = index_get(key, "categories/list", &[])?;
    let mut out: Vec<String> = v
        .get("feeds")
        .and_then(|r| r.as_array())
        .into_iter()
        .flatten()
        .filter_map(|c| text(c, "name"))
        .collect();
    out.sort();
    out.dedup();
    Ok(out)
}

/// Checks a key and secret.
pub fn check_key(key: &IndexKey) -> Result<(), String> {
    index_get(key, "categories/list", &[]).map(|_| ())
}

/// A show's artwork (at most 8 MB).
pub fn fetch_image(url: &str) -> Result<Vec<u8>, String> {
    let mut res = agent().get(url).call().map_err(|e| e.to_string())?;
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(8 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("the picture is too large".into());
    }
    Ok(bytes)
}

/// A line of a transcript.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cue {
    /// Seconds from the start (none: the transcript has no times).
    pub start: Option<f64>,
    pub end: Option<f64>,
    pub speaker: Option<String>,
    pub text: String,
}

/// A chapter of an episode.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Chapter {
    pub start: f64,
    pub title: String,
}

fn get_text(url: &str) -> Result<String, String> {
    let mut res = agent()
        .get(url)
        .call()
        .map_err(|e| format!("it could not be fetched: {e}"))?;
    let mut bytes = Vec::new();
    res.body_mut()
        .as_reader()
        .take(16 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
}

/// Fetches and reads a transcript (WebVTT, SRT, Podcast Index JSON, HTML
/// or plain text).
pub fn transcript(url: &str, kind: Option<&str>) -> Result<Vec<Cue>, String> {
    let body = get_text(url)?;
    Ok(read_transcript(&body, kind.unwrap_or("")))
}

/// "01:02:03.456", "02:03,456" or "75.5" as seconds.
fn seconds(t: &str) -> Option<f64> {
    let t = t.trim().replace(',', ".");
    let mut total = 0.0;
    for part in t.split(':') {
        total = total * 60.0 + part.trim().parse::<f64>().ok()?;
    }
    Some(total)
}

pub fn read_transcript(body: &str, kind: &str) -> Vec<Cue> {
    let trimmed = body.trim_start_matches('\u{feff}').trim();
    if kind.contains("json") || trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            return v
                .get("segments")
                .and_then(|s| s.as_array())
                .into_iter()
                .flatten()
                .filter_map(|s| {
                    let text = text(s, "body")?;
                    Some(Cue {
                        start: s.get("startTime").and_then(|x| x.as_f64()),
                        end: s.get("endTime").and_then(|x| x.as_f64()),
                        speaker: text_opt(s, "speaker"),
                        text,
                    })
                })
                .collect();
        }
    }
    if trimmed.contains("-->") {
        let mut out = Vec::new();
        for block in trimmed.replace("\r\n", "\n").split("\n\n") {
            let lines: Vec<&str> = block.lines().collect();
            let Some(ti) = lines.iter().position(|l| l.contains("-->")) else {
                continue;
            };
            let mut parts = lines[ti].split("-->");
            let start = parts.next().and_then(seconds);
            let end = parts
                .next()
                .and_then(|e| e.split_whitespace().next())
                .and_then(seconds);
            let mut speaker = None;
            let mut words = Vec::new();
            for l in &lines[ti + 1..] {
                let mut l = l.trim().to_owned();
                // WebVTT voice tags: <v Ada Lovelace>text</v>
                if let Some(rest) = l.strip_prefix("<v ").or_else(|| l.strip_prefix("<v.")) {
                    if let Some((who, after)) = rest.split_once('>') {
                        speaker = Some(who.split('.').next_back().unwrap_or(who).trim().to_owned());
                        l = after.to_owned();
                    }
                }
                let clean = crate::parse::plain_text(&l);
                if !clean.is_empty() {
                    words.push(clean);
                }
            }
            if !words.is_empty() {
                out.push(Cue {
                    start,
                    end,
                    speaker,
                    text: words.join(" "),
                });
            }
        }
        return out;
    }
    // HTML or plain text: paragraphs without times.
    let plain = crate::parse::plain_text(trimmed);
    plain
        .split("\n\n")
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| Cue {
            start: None,
            end: None,
            speaker: None,
            text: p.to_owned(),
        })
        .collect()
}

fn text_opt(v: &serde_json::Value, k: &str) -> Option<String> {
    text(v, k)
}

/// Fetches and reads Podcasting 2.0 chapters (JSON).
pub fn chapters(url: &str) -> Result<Vec<Chapter>, String> {
    let body = get_text(url)?;
    Ok(read_chapters(&body))
}

pub fn read_chapters(body: &str) -> Vec<Chapter> {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body.trim_start_matches('\u{feff}'))
    else {
        return Vec::new();
    };
    let mut out: Vec<Chapter> = v
        .get("chapters")
        .and_then(|c| c.as_array())
        .into_iter()
        .flatten()
        .filter(|c| c.get("toc").and_then(|t| t.as_bool()) != Some(false))
        .filter_map(|c| {
            Some(Chapter {
                start: c.get("startTime").and_then(|x| x.as_f64())?,
                title: text(c, "title").unwrap_or_default(),
            })
        })
        .collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    out
}

/// Episodes larger than this are not downloaded.
pub const MAX_EPISODE: u64 = 2 * 1024 * 1024 * 1024;

/// Downloads an episode's audio to `dest` (written aside, then renamed).
/// Returns the file's extension from its type or address.
pub fn download_audio(url: &str, dest: &Path) -> Result<(), String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(3600)))
        .max_redirects(10)
        .user_agent(concat!(
            "Mozilla/5.0 (compatible; Libreri/",
            env!("CARGO_PKG_VERSION"),
            "; podcasts)"
        ))
        .build()
        .into();
    let mut res = agent
        .get(url)
        .call()
        .map_err(|e| format!("the episode could not be downloaded: {e}"))?;
    let tmp = dest.with_extension("part");
    let result = (|| -> Result<(), String> {
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut reader = res.body_mut().with_config().limit(MAX_EPISODE).reader();
        let mut buf = vec![0u8; 256 * 1024];
        let mut got = 0u64;
        loop {
            let n = reader
                .read(&mut buf)
                .map_err(|e| format!("the download stopped: {e}"))?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            got += n as u64;
        }
        if got == 0 {
            return Err("the episode's file was empty".into());
        }
        file.flush().map_err(|e| e.to_string())?;
        drop(file);
        std::fs::rename(&tmp, dest).map_err(|e| e.to_string())
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

/// The file extension for an episode's audio.
pub fn audio_extension(url: &str, kind: Option<&str>) -> &'static str {
    match kind.unwrap_or("") {
        "audio/mpeg" | "audio/mp3" => return "mp3",
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" | "audio/aac" => return "m4a",
        "audio/ogg" => return "ogg",
        "audio/opus" => return "opus",
        "audio/flac" | "audio/x-flac" => return "flac",
        _ => {}
    }
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    for ext in ["mp3", "m4a", "m4b", "aac", "ogg", "opus", "flac"] {
        if path.ends_with(&format!(".{ext}")) {
            return if ext == "aac" { "m4a" } else { ext };
        }
    }
    "mp3"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_apple_and_index_answers() {
        let apple = serde_json::json!({"resultCount": 2, "results": [
            {"collectionName": "The Proof Hour", "artistName": "Ada", "feedUrl": "https://ex.org/proof.xml",
             "artworkUrl600": "https://ex.org/a600.jpg", "genres": ["Mathematics", "Podcasts"], "trackCount": 214,
             "releaseDate": "2026-09-30T10:00:00Z"},
            {"collectionName": "No feed"}
        ]});
        let shows = apple_shows(&apple);
        assert_eq!(shows.len(), 1);
        assert_eq!(shows[0].categories, ["Mathematics"]);
        assert_eq!(shows[0].episodes, Some(214));

        let index = serde_json::json!({"feeds": [{"id": 1, "title": "Signal Lab", "url": "https://ex.org/s.xml",
            "author": "Grace", "artwork": "https://ex.org/s.png", "categories": {"1": "Science"},
            "episodeCount": 9, "newestItemPubdate": 1790000000, "description": "<p>Noise &amp; signal</p>"}]});
        let shows = index_shows(&index);
        assert_eq!(shows[0].categories, ["Science"]);
        assert_eq!(shows[0].about.as_deref(), Some("Noise & signal"));
        assert!(shows[0].latest.is_some());
    }

    #[test]
    fn signs_index_requests() {
        let h = index_headers(
            &IndexKey {
                key: "KEY".into(),
                secret: "SECRET".into(),
            },
            1_700_000_000,
        );
        assert_eq!(h[1].1, "1700000000");
        assert_eq!(h[2].1, "5f8983664e541a83aaae7f3a47f1957fbca48aec");
        assert_eq!(h[2].1.len(), 40);
    }

    #[test]
    fn reads_transcripts_and_chapters() {
        let vtt = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:04.500\n<v Ada Lovelace>Numbers can do more\nthan count.</v>\n\n00:01:02.250 --> 00:01:05.000\nAnd so on.\n";
        let cues = read_transcript(vtt, "text/vtt");
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[0].start, Some(1.0));
        assert_eq!(cues[0].speaker.as_deref(), Some("Ada Lovelace"));
        assert_eq!(cues[0].text, "Numbers can do more than count.");
        assert_eq!(cues[1].start, Some(62.25));

        let srt = "1\r\n00:00:02,500 --> 00:00:03,000\r\nHello\r\n\r\n";
        assert_eq!(
            read_transcript(srt, "application/x-subrip")[0].start,
            Some(2.5)
        );

        let json = r#"{"version":"1.0.0","segments":[{"speaker":"Host","startTime":0.5,"endTime":2,"body":"Welcome"}]}"#;
        let cues = read_transcript(json, "application/json");
        assert_eq!(cues[0].speaker.as_deref(), Some("Host"));

        let ch = r#"{"version":"1.2.0","chapters":[{"startTime":90,"title":"Riemann"},{"startTime":0,"title":"Intro"},{"startTime":60,"title":"Hidden","toc":false}]}"#;
        let ch = read_chapters(ch);
        assert_eq!(
            ch.iter().map(|c| c.title.as_str()).collect::<Vec<_>>(),
            ["Intro", "Riemann"]
        );
        assert_eq!(audio_extension("https://x/a.M4A?x=1", None), "m4a");
        assert_eq!(audio_extension("https://x/a", Some("audio/mpeg")), "mp3");
    }

    #[test]
    fn reads_podcast_feeds() {
        let rss = r#"<?xml version="1.0"?>
<rss version="2.0" xmlns:itunes="http://www.itunes.com/dtds/podcast-1.0.dtd" xmlns:podcast="https://podcastindex.org/namespace/1.0">
<channel><title>The Proof Hour</title><itunes:author>Ada</itunes:author>
<itunes:image href="https://ex.org/art.jpg"/>
<item><title>214 · Primes</title><guid>ep214</guid>
<enclosure url="https://cdn.ex.org/214.mp3" type="audio/mpeg" length="123"/>
<itunes:duration>58:00</itunes:duration>
<podcast:transcript url="https://ex.org/214.html" type="text/html"/>
<podcast:transcript url="https://ex.org/214.vtt" type="text/vtt"/>
<podcast:chapters url="https://ex.org/214.json" type="application/json+chapters"/>
</item>
<item><title>213</title><guid>ep213</guid><enclosure url="https://cdn.ex.org/213.m4a" type="audio/x-m4a" length="1"/></item>
</channel></rss>"#;
        let p = crate::parse::parse(rss.as_bytes(), "https://ex.org/feed.xml").unwrap();
        assert_eq!(p.image.as_deref(), Some("https://ex.org/art.jpg"));
        assert_eq!(p.author.as_deref(), Some("Ada"));
        let e = &p.entries[0];
        assert_eq!(e.audio.as_deref(), Some("https://cdn.ex.org/214.mp3"));
        assert_eq!(e.duration, Some(3480.0));
        assert_eq!(e.transcript.as_deref(), Some("https://ex.org/214.vtt"));
        assert_eq!(e.chapters.as_deref(), Some("https://ex.org/214.json"));
        assert_eq!(p.entries[1].transcript, None);
        assert_eq!(p.entries[1].audio_type.as_deref(), Some("audio/x-m4a"));
    }
}
