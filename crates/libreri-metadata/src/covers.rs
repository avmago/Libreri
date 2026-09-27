//! Downloading covers. Only hosts the sources use for images are allowed,
//! so a cover link can never make the app fetch an arbitrary address.

use crate::Http;

/// Hosts covers are fetched from (and their subdomains).
const COVER_HOSTS: &[&str] = &[
    "covers.openlibrary.org",
    "archive.org",
    "books.google.com",
    "books.googleusercontent.com",
    "comicvine.gamespot.com",
    "images.isbndb.com",
];

/// Smallest file taken as a real cover; sources send tiny placeholders
/// when they have none.
const MIN_BYTES: usize = 1200;

/// A downloaded cover.
pub struct Cover {
    pub bytes: Vec<u8>,
    /// "jpg", "png", "webp" or "gif".
    pub extension: &'static str,
}

fn host(url: &str) -> Option<&str> {
    let rest = url.strip_prefix("https://")?;
    let host = rest.split(['/', '?', '#']).next()?;
    (!host.contains(['@', ':'])).then_some(host)
}

/// True if covers may be fetched from this address.
pub fn cover_allowed(url: &str) -> bool {
    host(url).is_some_and(|h| {
        let h = h.to_ascii_lowercase();
        COVER_HOSTS
            .iter()
            .any(|a| h == *a || h.ends_with(&format!(".{a}")))
    })
}

fn kind(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        Some("png")
    } else if bytes.len() > 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        Some("webp")
    } else if bytes.starts_with(b"GIF8") {
        Some("gif")
    } else {
        None
    }
}

/// Downloads a cover, checking the address and that the answer is an image.
pub fn fetch_cover(http: &dyn Http, url: &str) -> Result<Cover, String> {
    let url = url.replacen("http://", "https://", 1);
    if !cover_allowed(&url) {
        return Err("covers are only downloaded from the book sources".into());
    }
    let res = http.get(&url, &[("Accept", "image/*")])?;
    if res.status == 404 {
        return Err("the source has no cover for it".into());
    }
    if !res.ok() {
        return Err(format!(
            "the cover could not be downloaded ({})",
            res.status
        ));
    }
    let extension = kind(&res.body).ok_or("the cover is not an image")?;
    if res.body.len() < MIN_BYTES {
        return Err("the source has no cover for it".into());
    }
    Ok(Cover {
        bytes: res.body,
        extension,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::FakeHttp;

    #[test]
    fn only_source_hosts_are_allowed() {
        assert!(cover_allowed("https://covers.openlibrary.org/b/id/1-L.jpg"));
        assert!(cover_allowed("https://ia800.us.archive.org/x.jpg"));
        assert!(cover_allowed("https://books.google.com/books/content?id=x"));
        assert!(!cover_allowed("http://covers.openlibrary.org/b/id/1-L.jpg"));
        assert!(!cover_allowed("https://evil.example/archive.org/x.jpg"));
        assert!(!cover_allowed("https://notarchive.org/x.jpg"));
        assert!(!cover_allowed("https://archive.org@evil.example/x.jpg"));
        assert!(!cover_allowed("https://127.0.0.1/x.jpg"));
    }

    #[test]
    fn checks_the_answer_is_an_image() {
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        jpeg.resize(5000, 0);
        let mut http = FakeHttp::default();
        http.answers.insert("big".into(), (200, jpeg));
        http.answers
            .insert("tiny".into(), (200, vec![b'G', b'I', b'F', b'8', 0, 0]));
        http.answers
            .insert("html".into(), (200, b"<html>".repeat(500)));
        let c = fetch_cover(&http, "http://covers.openlibrary.org/big.jpg").unwrap();
        assert_eq!(c.extension, "jpg");
        assert!(fetch_cover(&http, "https://covers.openlibrary.org/tiny.gif").is_err());
        assert!(fetch_cover(&http, "https://covers.openlibrary.org/html").is_err());
        assert!(fetch_cover(&http, "https://example.com/big.jpg").is_err());
        assert!(http
            .asked
            .lock()
            .unwrap()
            .iter()
            .all(|u| u.starts_with("https://covers.")));
    }
}
