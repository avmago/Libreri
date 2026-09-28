//! What a link points to, read from its address alone (no network): a
//! YouTube or Vimeo video and where to start it, a web page, a file.

use serde::{Deserialize, Serialize};
use url::Url;

/// A video site Libreri knows how to embed without asking the site.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "provider")]
pub enum KnownVideo {
    Youtube { id: String },
    Vimeo { id: String, hash: Option<String> },
}

/// An address, tidied, with what could be read from it.
#[derive(Debug, Clone, PartialEq)]
pub struct Address {
    pub url: String,
    /// The site's name for showing ("youtube.com", "en.wikipedia.org").
    pub site: String,
    pub video: Option<KnownVideo>,
    /// A start time written in the address (`t=1m30s`, `#t=90`).
    pub start: Option<f64>,
}

/// Seconds from "90", "1:30", "1:02:03", "1m30s", "1h2m3s" or "90s".
pub fn parse_time(s: &str) -> Option<f64> {
    let s = s.trim().trim_end_matches('s');
    if s.is_empty() {
        return None;
    }
    if let Ok(n) = s.parse::<f64>() {
        return (n >= 0.0).then_some(n);
    }
    if s.contains(':') {
        let mut total = 0.0;
        for part in s.split(':') {
            total = total * 60.0 + part.trim().parse::<f64>().ok()?;
        }
        return Some(total);
    }
    // 1h2m3s (the final "s" is already gone).
    let mut total = 0.0;
    let mut num = String::new();
    for c in s.chars() {
        match c {
            '0'..='9' | '.' => num.push(c),
            'h' | 'm' | 's' => {
                let n: f64 = num.parse().ok()?;
                num.clear();
                total += n * if c == 'h' {
                    3600.0
                } else if c == 'm' {
                    60.0
                } else {
                    1.0
                };
            }
            _ => return None,
        }
    }
    if !num.is_empty() {
        total += num.parse::<f64>().ok()?;
    }
    Some(total)
}

/// "1:05:09" or "4:07" for seconds.
pub fn format_time(secs: f64) -> String {
    let s = secs.max(0.0).round() as u64;
    let (h, m, s) = (s / 3600, s / 60 % 60, s % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

fn youtube_id(u: &Url) -> Option<String> {
    let host = u
        .host_str()?
        .trim_start_matches("www.")
        .trim_start_matches("m.");
    let id = match host {
        "youtu.be" => u.path_segments()?.next()?.to_owned(),
        "youtube.com" | "music.youtube.com" | "youtube-nocookie.com" => {
            let mut segs = u.path_segments()?;
            match segs.next()? {
                "watch" => u.query_pairs().find(|(k, _)| k == "v")?.1.into_owned(),
                "embed" | "shorts" | "live" | "v" => segs.next()?.to_owned(),
                _ => return None,
            }
        }
        _ => return None,
    };
    let ok = (6..=20).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ok.then_some(id)
}

fn vimeo(u: &Url) -> Option<KnownVideo> {
    let host = u.host_str()?.trim_start_matches("www.");
    let segs: Vec<&str> = u.path_segments()?.filter(|s| !s.is_empty()).collect();
    let (id, hash) = match host {
        "vimeo.com" => {
            // vimeo.com/123, vimeo.com/123/abcdef (unlisted), vimeo.com/channels/x/123
            let i = segs
                .iter()
                .position(|s| s.chars().all(|c| c.is_ascii_digit()))?;
            (segs[i], segs.get(i + 1).copied())
        }
        "player.vimeo.com" if segs.first() == Some(&"video") => (*segs.get(1)?, None),
        _ => return None,
    };
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let hash = hash
        .map(str::to_owned)
        .or_else(|| {
            u.query_pairs()
                .find(|(k, _)| k == "h")
                .map(|(_, v)| v.into_owned())
        })
        .filter(|h| h.chars().all(|c| c.is_ascii_alphanumeric()));
    Some(KnownVideo::Vimeo {
        id: id.to_owned(),
        hash,
    })
}

fn start_of(u: &Url) -> Option<f64> {
    let from_query = u
        .query_pairs()
        .find(|(k, _)| k == "t" || k == "start" || k == "time_continue")
        .and_then(|(_, v)| parse_time(&v));
    let from_fragment = u.fragment().and_then(|f| {
        f.split('&')
            .find_map(|p| p.strip_prefix("t=").or_else(|| p.strip_prefix("at=")))
            .and_then(parse_time)
    });
    from_query.or(from_fragment).filter(|t| *t > 0.0)
}

/// Reads an address typed or pasted by someone. Addresses without a scheme
/// ("example.com/page") get `https://`.
pub fn read(input: &str) -> Result<Address, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("type or paste an address".into());
    }
    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let u = Url::parse(&with_scheme).map_err(|_| "that is not a web address".to_owned())?;
    if u.scheme() != "https" && u.scheme() != "http" {
        return Err("only web addresses (http or https) can be fetched".into());
    }
    let host = u.host_str().ok_or("that address has no site")?;
    if !host.contains('.') && host != "localhost" && u.port().is_none() {
        return Err("that is not a web address".into());
    }
    let video = youtube_id(&u)
        .map(|id| KnownVideo::Youtube { id })
        .or_else(|| vimeo(&u));
    Ok(Address {
        site: host.trim_start_matches("www.").to_owned(),
        start: start_of(&u),
        video,
        url: u.to_string(),
    })
}

/// The address a known video plays from, in the privacy-enhanced form for
/// YouTube. `start` is in seconds.
pub fn embed_url(video: &KnownVideo, start: Option<f64>) -> String {
    let start = start.filter(|s| *s > 0.0).map(|s| s.round() as u64);
    match video {
        KnownVideo::Youtube { id } => {
            let mut u = format!("https://www.youtube-nocookie.com/embed/{id}?autoplay=1&rel=0");
            if let Some(s) = start {
                u.push_str(&format!("&start={s}"));
            }
            u
        }
        KnownVideo::Vimeo { id, hash } => {
            let mut u = format!("https://player.vimeo.com/video/{id}?autoplay=1&dnt=1");
            if let Some(h) = hash {
                u.push_str(&format!("&h={h}"));
            }
            if let Some(s) = start {
                u.push_str(&format!("#t={s}s"));
            }
            u
        }
    }
}

/// The video's own page, at the start time (for "Open in browser").
pub fn watch_url(video: &KnownVideo, start: Option<f64>) -> String {
    let start = start.filter(|s| *s > 0.0).map(|s| s.round() as u64);
    match video {
        KnownVideo::Youtube { id } => match start {
            Some(s) => format!("https://www.youtube.com/watch?v={id}&t={s}s"),
            None => format!("https://www.youtube.com/watch?v={id}"),
        },
        KnownVideo::Vimeo { id, hash } => {
            let base = match hash {
                Some(h) => format!("https://vimeo.com/{id}/{h}"),
                None => format!("https://vimeo.com/{id}"),
            };
            match start {
                Some(s) => format!("{base}#t={s}s"),
                None => base,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_times() {
        assert_eq!(parse_time("90"), Some(90.0));
        assert_eq!(parse_time("1:30"), Some(90.0));
        assert_eq!(parse_time("1:02:03"), Some(3723.0));
        assert_eq!(parse_time("1m30s"), Some(90.0));
        assert_eq!(parse_time("1h2m3s"), Some(3723.0));
        assert_eq!(parse_time("2m"), Some(120.0));
        assert_eq!(parse_time("x"), None);
        assert_eq!(format_time(3723.0), "1:02:03");
        assert_eq!(format_time(67.0), "1:07");
    }

    #[test]
    fn finds_youtube_videos() {
        for u in [
            "https://www.youtube.com/watch?v=dQw4w9WgXcQ&t=1m30s",
            "youtu.be/dQw4w9WgXcQ?t=90",
            "https://m.youtube.com/shorts/dQw4w9WgXcQ?start=90",
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?start=90",
        ] {
            let a = read(u).unwrap();
            assert_eq!(
                a.video,
                Some(KnownVideo::Youtube {
                    id: "dQw4w9WgXcQ".into()
                }),
                "{u}"
            );
            assert_eq!(a.start, Some(90.0), "{u}");
        }
        let v = KnownVideo::Youtube {
            id: "dQw4w9WgXcQ".into(),
        };
        assert_eq!(
            embed_url(&v, Some(90.0)),
            "https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ?autoplay=1&rel=0&start=90"
        );
    }

    #[test]
    fn finds_vimeo_videos() {
        let a = read("https://vimeo.com/76979871#t=30s").unwrap();
        assert_eq!(
            a.video,
            Some(KnownVideo::Vimeo {
                id: "76979871".into(),
                hash: None
            })
        );
        assert_eq!(a.start, Some(30.0));
        let a = read("https://vimeo.com/76979871/abc123").unwrap();
        assert_eq!(
            a.video,
            Some(KnownVideo::Vimeo {
                id: "76979871".into(),
                hash: Some("abc123".into())
            })
        );
        assert!(embed_url(a.video.as_ref().unwrap(), Some(5.0)).ends_with("&h=abc123#t=5s"));
    }

    #[test]
    fn reads_web_pages() {
        let a = read("en.wikipedia.org/wiki/Lighthouse").unwrap();
        assert_eq!(a.url, "https://en.wikipedia.org/wiki/Lighthouse");
        assert_eq!(a.site, "en.wikipedia.org");
        assert!(a.video.is_none());
        assert!(read("javascript:alert(1)").is_err());
        assert!(read("file:///etc/passwd").is_err());
        assert!(read("hello").is_err());
    }
}
