//! Offline copies of web pages: the readable part of the page (as reader
//! views pick it out), cleaned of scripts, forms and anything that could
//! run, with its pictures inside, as one HTML file that opens anywhere.

use crate::fetch::{self, MAX_PICTURE};
use base64::Engine;
use regex::Regex;
use std::collections::HashMap;

/// At most this many pictures, and this much picture data, are kept.
const MAX_PICTURES: usize = 60;
const MAX_PICTURE_BYTES: u64 = 25 * 1024 * 1024;

fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

/// What the copy says about where it came from.
pub struct Source<'a> {
    pub url: &'a str,
    pub title: Option<&'a str>,
    pub site: &'a str,
    /// When the copy was made, for showing ("28 September 2026").
    pub saved: &'a str,
}

/// Makes the copy from a fetched page. `pictures` fetches a picture (or
/// not); it is given absolute addresses only.
pub fn make(
    page: &str,
    src: &Source,
    mut pictures: impl FnMut(&str) -> Option<(Vec<u8>, String)>,
) -> Result<String, String> {
    let cfg = dom_smoothie::Config {
        max_elements_to_parse: 60_000,
        ..Default::default()
    };
    let mut r = dom_smoothie::Readability::new(page, Some(src.url), Some(cfg))
        .map_err(|e| format!("the page could not be read: {e}"))?;
    let article = r
        .parse()
        .map_err(|_| "no readable text was found on the page".to_owned())?;
    let base = url::Url::parse(src.url).map_err(|e| e.to_string())?;
    // Only plain content: no scripts, styles, frames, forms or handlers;
    // links and pictures made absolute.
    let clean = ammonia::Builder::default()
        .url_relative(ammonia::UrlRelative::RewriteWithBase(base))
        .link_rel(Some("noopener noreferrer"))
        .add_generic_attributes(["dir", "lang"])
        .add_tag_attributes("a", ["target"])
        .set_tag_attribute_value("a", "target", "_blank")
        .clean(&article.content)
        .to_string();
    // Pictures inside the file.
    let re = Regex::new(r#"(<img\b[^>]*?\ssrc=")(https?://[^"]+)(")"#).expect("regex");
    let mut cache: HashMap<String, Option<String>> = HashMap::new();
    let (mut count, mut bytes) = (0usize, 0u64);
    let body = re
        .replace_all(&clean, |c: &regex::Captures| {
            let url = c[2].replace("&amp;", "&");
            let data = cache
                .entry(url.clone())
                .or_insert_with(|| {
                    if count >= MAX_PICTURES || bytes >= MAX_PICTURE_BYTES {
                        return None;
                    }
                    let (b, kind) = pictures(&url)?;
                    if b.len() as u64 > MAX_PICTURE
                        || !kind.starts_with("image/")
                        || kind.contains("svg")
                    {
                        return None;
                    }
                    count += 1;
                    bytes += b.len() as u64;
                    Some(format!(
                        "data:{kind};base64,{}",
                        base64::engine::general_purpose::STANDARD.encode(b)
                    ))
                })
                .clone();
            // A picture that could not be kept is left out, not fetched
            // later from the site.
            match data {
                Some(d) => format!("{}{}{}", &c[1], d, &c[3]),
                None => format!("{}data:,\" data-missing=\"1\"", &c[1]),
            }
        })
        .into_owned();
    let title = src
        .title
        .map(str::to_owned)
        .unwrap_or_else(|| article.title.clone());
    let byline = article
        .byline
        .as_deref()
        .map(|b| format!("<p class=\"by\">{}</p>", escape(b)))
        .unwrap_or_default();
    let dir = article.dir.as_deref().unwrap_or("auto");
    let lang = article.lang.as_deref().unwrap_or("");
    Ok(format!(
        r#"<!doctype html>
<html lang="{lang}" dir="{dir}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'">
<meta name="libreri:source" content="{url}">
<title>{title}</title>
<style>
  body {{ font: 17px/1.6 Georgia, "Iowan Old Style", serif; max-width: 42em; margin: 2em auto; padding: 0 1.2em; color: #1f2328; background: #fff; }}
  @media (prefers-color-scheme: dark) {{ body {{ color: #e6e6e6; background: #1b1b1d; }} a {{ color: #8ab4f8; }} }}
  img, video {{ max-width: 100%; height: auto; }}
  img[data-missing] {{ display: none; }}
  pre, code {{ font-family: ui-monospace, Menlo, monospace; font-size: 0.9em; }}
  pre {{ overflow-x: auto; }}
  table {{ border-collapse: collapse; }} td, th {{ border: 1px solid #8884; padding: 0.2em 0.5em; }}
  blockquote {{ margin-left: 0; padding-left: 1em; border-left: 3px solid #8886; }}
  .source {{ font: 13px/1.4 system-ui, sans-serif; color: #6b7280; border-bottom: 1px solid #8884; padding-bottom: 0.8em; margin-bottom: 1.5em; }}
  .by {{ font-style: italic; color: #6b7280; }}
</style>
</head>
<body>
<div class="source">Saved from <a href="{url}" target="_blank" rel="noopener noreferrer">{site}</a> on {saved} · an offline copy made by Libreri</div>
<h1>{title}</h1>
{byline}
{body}
</body>
</html>
"#,
        url = escape(src.url),
        site = escape(src.site),
        saved = escape(src.saved),
        title = escape(&title),
    ))
}

/// Fetches pictures from the web for [`make`].
pub fn web_pictures() -> impl FnMut(&str) -> Option<(Vec<u8>, String)> {
    let agent = fetch::agent();
    move |url: &str| fetch::get_picture(&agent, url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_text_and_pictures_without_scripts() {
        let para = "The keeper climbed the tower every evening to light the lamp, and wrote down the weather, the ships he saw and the state of the sea. ".repeat(6);
        let page = format!(
            r#"<html><head><title>The keeper's log</title><script>alert(1)</script></head>
            <body><nav>Home · About · Shop</nav>
            <article><h2>Evening</h2><p onclick="steal()">{para}</p>
            <p><img src="/pics/tower.png" alt="The tower"><img src="https://evil.example/big.png"></p>
            <p>{para}</p><iframe src="https://ads.example"></iframe><form><input></form>
            <p><a href="/next" onclick="x()">Next day</a> <a href="javascript:alert(2)">bad</a></p>
            <p>{para}</p></article><footer>© Someone</footer></body></html>"#
        );
        let png = b"\x89PNG\r\n\x1a\nrest".to_vec();
        let mut asked = Vec::new();
        let html = make(
            &page,
            &Source {
                url: "https://sea.example/log/1",
                title: None,
                site: "sea.example",
                saved: "29 September 2026",
            },
            |u| {
                asked.push(u.to_owned());
                u.ends_with("tower.png")
                    .then(|| (png.clone(), "image/png".into()))
            },
        )
        .unwrap();
        assert!(html.contains("The keeper climbed the tower"));
        assert!(
            html.contains("<title>The keeper&#39;s log</title>")
                || html.contains("<title>The keeper's log</title>")
        );
        assert!(html.contains("data:image/png;base64,"));
        assert!(html.contains("data-missing"));
        assert!(asked.contains(&"https://sea.example/pics/tower.png".to_owned()));
        assert!(html.contains("https://sea.example/next"));
        for bad in [
            "<script>alert",
            "onclick",
            "<iframe",
            "<form",
            "javascript:",
        ] {
            assert!(!html.contains(bad), "{bad} left in");
        }
        assert!(html.contains("default-src 'none'"));
    }
}
