//! Audiobooks (Phase 7a): length and chapters, and the link between an
//! audiobook and the book it reads, with sync points ("this moment is this
//! place in the text").
//!
//! Links are about files, not people, so they belong to the library:
//! `.library-data/audio-links/<audiobook id>.json`. They travel in backups
//! and archives and follow the audiobook when its id changes.

use crate::paths::write_atomic;
use crate::{Error, Library, Result};
use libreri_core::{BookId, LibraryLayout};
use libreri_formats::audio::AudioInfo;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const LINK_VERSION: u32 = 1;

/// A moment of the audiobook matched to a place in the text.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPoint {
    /// Seconds into the audiobook.
    pub t: f64,
    /// The place in the text book (a reader locator, as JSON).
    pub locator: String,
    /// How far through the text book (0–1), for going between points.
    pub progress: f64,
    #[serde(default)]
    pub label: Option<String>,
    /// Found by listening rather than set by hand (Phase 7b).
    #[serde(default)]
    pub auto: bool,
}

/// An audiobook's link to its text.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioLink {
    #[serde(default)]
    pub format_version: u32,
    /// The book it reads.
    pub text: Option<BookId>,
    /// Sorted by time.
    #[serde(default)]
    pub points: Vec<SyncPoint>,
}

pub(crate) fn link_path(layout: &LibraryLayout, audio: &BookId) -> PathBuf {
    layout
        .data_dir()
        .join("audio-links")
        .join(format!("{audio}.json"))
}

pub(crate) fn rename(layout: &LibraryLayout, old: &BookId, new: &BookId) {
    let _ = fs::rename(link_path(layout, old), link_path(layout, new));
}

impl Library {
    /// Length and chapters of an audiobook.
    pub fn audio_info(&self, id: &BookId) -> Result<AudioInfo> {
        let path = self.audio_file(id)?;
        libreri_formats::audio::info(&path).map_err(Error::InvalidInput)
    }

    /// The audiobook's file.
    pub fn audio_file(&self, id: &BookId) -> Result<PathBuf> {
        let book = self.book(id)?;
        if !book.file_type.is_audio() {
            return Err(Error::InvalidInput("that book is not an audiobook".into()));
        }
        if !self.may_open(&book.rel_path) {
            return Err(Error::BookNotFound);
        }
        self.layout()
            .resolve_relative(&book.rel_path)
            .filter(|p| p.is_file())
            .ok_or(Error::BookNotFound)
    }

    /// Every word of a book with text, with where it is (for finding places
    /// in its audiobook). Pages without text use saved OCR text.
    pub fn sync_words(&self, text: &BookId) -> Result<Vec<PlacedWord>> {
        let book = self.book(text)?;
        if !self.may_open(&book.rel_path) {
            return Err(Error::BookNotFound);
        }
        let path = self
            .layout()
            .resolve_relative(&book.rel_path)
            .filter(|p| p.is_file())
            .ok_or(Error::BookNotFound)?;
        let mut t =
            libreri_formats::text::book_text(&path, book.file_type).map_err(Error::InvalidInput)?;
        if t.pages > 0 {
            if let Some(ocr) = self.ocr_text(&book.id).ok().flatten() {
                for p in &t.empty_pages {
                    if let Some(o) = ocr.page(*p) {
                        t.chunks.retain(|c| c.page != Some(*p));
                        t.chunks.push(libreri_formats::text::TextChunk {
                            page: Some(*p),
                            text: o.text.clone(),
                            ..Default::default()
                        });
                    }
                }
            }
            t.chunks.sort_by_key(|c| c.page);
        }
        Ok(place_words(&t))
    }

    /// Puts sync points found by listening in place of the ones found
    /// before. Points set by hand win: found points too near one, or on the
    /// wrong side of one, are left out.
    pub fn add_auto_points(&self, audio: &BookId, found: Vec<SyncPoint>) -> Result<AudioLink> {
        let link = self.audio_link(audio)?;
        let manual: Vec<SyncPoint> = link.points.into_iter().filter(|p| !p.auto).collect();
        let mut points: Vec<SyncPoint> = found
            .into_iter()
            .map(|p| SyncPoint { auto: true, ..p })
            .filter(|p| {
                manual
                    .iter()
                    .all(|m| (p.t - m.t).abs() > 20.0 && ((p.t < m.t) == (p.progress < m.progress)))
            })
            .collect();
        points.extend(manual);
        self.set_sync_points(audio, points)
    }

    /// The audiobook's link (empty when there is none). The linked book's
    /// id is brought up to date if its file changed.
    pub fn audio_link(&self, audio: &BookId) -> Result<AudioLink> {
        let book = self.book(audio)?;
        let mut link: AudioLink = fs::read_to_string(link_path(self.layout(), &book.id))
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        if let Some(text) = &link.text {
            link.text = self.with_db(|db| db.resolve_book_id(text))?;
        }
        Ok(link)
    }

    fn write_link(&self, audio: &BookId, link: &AudioLink) -> Result<()> {
        let path = link_path(self.layout(), audio);
        if link.text.is_none() && link.points.is_empty() {
            let _ = fs::remove_file(&path);
            return Ok(());
        }
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(link).map_err(std::io::Error::other)?;
        write_atomic(&path, &json)?;
        Ok(())
    }

    /// Links an audiobook to the book it reads (or unlinks it). Sync points
    /// are kept only while the linked book stays the same.
    pub fn set_audio_link(&self, audio: &BookId, text: Option<&BookId>) -> Result<AudioLink> {
        self.require_edit()?;
        let book = self.book(audio)?;
        let mut link = self.audio_link(&book.id)?;
        let text = match text {
            Some(t) => {
                let t = self.book(t)?;
                if t.file_type.is_audio() {
                    return Err(Error::InvalidInput(
                        "link an audiobook to a book with text".into(),
                    ));
                }
                Some(t.id)
            }
            None => None,
        };
        if link.text != text {
            link.points.clear();
        }
        link.text = text;
        link.format_version = LINK_VERSION;
        self.write_link(&book.id, &link)?;
        Ok(link)
    }

    /// Replaces the sync points.
    pub fn set_sync_points(&self, audio: &BookId, mut points: Vec<SyncPoint>) -> Result<AudioLink> {
        self.require_edit()?;
        let book = self.book(audio)?;
        let mut link = self.audio_link(&book.id)?;
        if link.text.is_none() {
            return Err(Error::InvalidInput(
                "link the audiobook to its book first".into(),
            ));
        }
        points.retain(|p| p.t.is_finite() && p.t >= 0.0 && p.progress.is_finite());
        for p in &mut points {
            p.progress = p.progress.clamp(0.0, 1.0);
            serde_json::from_str::<serde_json::Value>(&p.locator)
                .map_err(|_| Error::InvalidInput("a sync point's place is not valid".into()))?;
        }
        points.sort_by(|a, b| a.t.total_cmp(&b.t));
        points.dedup_by(|b, a| (a.t - b.t).abs() < 0.5);
        link.points = points;
        link.format_version = LINK_VERSION;
        self.write_link(&book.id, &link)?;
        Ok(link)
    }

    /// Audiobooks linked to a book with text.
    pub fn audiobooks_for(&self, text: &BookId) -> Result<Vec<BookId>> {
        let text = self.book(text)?.id;
        let dir = self.layout().data_dir().join("audio-links");
        let Ok(entries) = fs::read_dir(dir) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            let Some(id) = name
                .strip_suffix(".json")
                .and_then(|s| BookId::from_hex(s).ok())
            else {
                continue;
            };
            let Ok(link) = self.audio_link(&id) else {
                continue;
            };
            if link.text.as_ref() == Some(&text) && self.book(&id).is_ok() {
                out.push(id);
            }
        }
        Ok(out)
    }

    /// Brings a link back from an archive (unless one is already here).
    pub(crate) fn restore_audio_link(&self, id: &BookId, bytes: Option<&[u8]>) {
        let Some(bytes) = bytes else { return };
        let path = link_path(self.layout(), id);
        if serde_json::from_slice::<AudioLink>(bytes).is_ok() && !path.exists() {
            if let Some(dir) = path.parent() {
                let _ = fs::create_dir_all(dir);
            }
            let _ = fs::write(&path, bytes);
        }
    }
}

/// A word of a book's text and where it is.
#[derive(Debug, Clone, PartialEq)]
pub struct PlacedWord {
    pub word: String,
    /// 0–1 through the book, as the reader counts it (by page for PDF and
    /// DjVu, by length for flowing books).
    pub progress: f64,
    /// 1-based page, for page-based books.
    pub page: Option<u32>,
}

impl PlacedWord {
    /// A sync point at this word.
    pub fn sync_point(&self, t: f64) -> SyncPoint {
        let locator = match self.page {
            Some(page) => serde_json::json!({ "type": "pdf", "page": page }),
            None => serde_json::json!({ "type": "scroll", "fraction": self.progress }),
        };
        SyncPoint {
            t,
            locator: locator.to_string(),
            progress: self.progress,
            label: self.page.map(|p| format!("p. {p}")),
            auto: true,
        }
    }
}

fn place_words(t: &libreri_formats::text::BookText) -> Vec<PlacedWord> {
    let mut out = Vec::new();
    if t.pages > 0 {
        let pages = f64::from(t.pages);
        // Words of each page, spread over the page.
        let mut i = 0;
        while i < t.chunks.len() {
            let page = t.chunks[i].page.unwrap_or(1);
            let mut words: Vec<&str> = Vec::new();
            while i < t.chunks.len() && t.chunks[i].page.unwrap_or(1) == page {
                words.extend(t.chunks[i].text.split_whitespace());
                i += 1;
            }
            let n = words.len().max(1) as f64;
            for (k, w) in words.into_iter().enumerate() {
                out.push(PlacedWord {
                    word: w.to_owned(),
                    progress: ((f64::from(page) - 1.0 + k as f64 / n) / pages).clamp(0.0, 1.0),
                    page: Some(page),
                });
            }
        }
    } else {
        let total: usize = t.chunks.iter().map(|c| c.text.len()).sum();
        let total = total.max(1) as f64;
        let mut before = 0usize;
        for c in &t.chunks {
            let base = c.text.as_ptr() as usize;
            for w in c.text.split_whitespace() {
                let at = before + (w.as_ptr() as usize - base);
                out.push(PlacedWord {
                    word: w.to_owned(),
                    progress: at as f64 / total,
                    page: None,
                });
            }
            before += c.text.len();
        }
    }
    out
}

/// Where the text is at `t` seconds, going in a straight line between the
/// sync points (before the first and after the last, from the book's
/// start and end). Returns 0–1 through the text.
pub fn text_progress(points: &[SyncPoint], t: f64, duration: f64) -> f64 {
    let mut prev = (0.0, 0.0);
    for p in points {
        if p.t >= t {
            let span = p.t - prev.0;
            let k = if span > 0.0 { (t - prev.0) / span } else { 1.0 };
            return prev.1 + (p.progress - prev.1) * k.clamp(0.0, 1.0);
        }
        prev = (p.t, p.progress);
    }
    let span = duration - prev.0;
    let k = if span > 0.0 { (t - prev.0) / span } else { 1.0 };
    (prev.1 + (1.0 - prev.1) * k.clamp(0.0, 1.0)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{library, md_book};
    use crate::NoProgress;
    use libreri_core::BookQuery;

    fn point(t: f64, progress: f64) -> SyncPoint {
        SyncPoint {
            t,
            locator: "{\"type\":\"scroll\",\"fraction\":0}".into(),
            progress,
            label: None,
            auto: false,
        }
    }

    #[test]
    fn places_words() {
        use libreri_formats::text::{BookText, TextChunk};
        let paged = BookText {
            chunks: vec![
                TextChunk {
                    page: Some(1),
                    text: "one two".into(),
                    ..Default::default()
                },
                TextChunk {
                    page: Some(2),
                    text: "three four".into(),
                    ..Default::default()
                },
            ],
            pages: 2,
            empty_pages: vec![],
        };
        let w = place_words(&paged);
        let got: Vec<(f64, Option<u32>)> = w.iter().map(|w| (w.progress, w.page)).collect();
        assert_eq!(
            got,
            [
                (0.0, Some(1)),
                (0.25, Some(1)),
                (0.5, Some(2)),
                (0.75, Some(2))
            ]
        );
        assert_eq!(w[2].sync_point(9.0).locator, r#"{"page":2,"type":"pdf"}"#);
        let flowing = BookText {
            chunks: vec![TextChunk {
                text: "aaaa bbbb".into(),
                ..Default::default()
            }],
            pages: 0,
            empty_pages: vec![],
        };
        let w = place_words(&flowing);
        assert_eq!(w[1].progress, 5.0 / 9.0);
        assert!(w[1].sync_point(1.0).locator.contains("scroll"));
    }

    #[test]
    fn goes_between_sync_points() {
        let pts = [point(100.0, 0.2), point(200.0, 0.4)];
        assert!((text_progress(&pts, 50.0, 1000.0) - 0.1).abs() < 1e-9);
        assert!((text_progress(&pts, 150.0, 1000.0) - 0.3).abs() < 1e-9);
        assert!((text_progress(&pts, 600.0, 1000.0) - 0.7).abs() < 1e-9);
        assert!((text_progress(&[], 500.0, 1000.0) - 0.5).abs() < 1e-9);
    }

    #[test]
    fn links_an_audiobook_to_its_book() {
        let (_d, lib) = library();
        let books = lib.layout().books_dir();
        md_book(&books, "text.md", "The Lighthouse");
        std::fs::write(books.join("The Lighthouse.mp3"), b"not really audio").unwrap();
        lib.scan(&NoProgress).unwrap();
        let all = lib.books(&BookQuery::default()).unwrap();
        let text = all
            .iter()
            .find(|b| !b.file_type.is_audio())
            .unwrap()
            .clone();
        let audio = all.iter().find(|b| b.file_type.is_audio()).unwrap().clone();

        assert!(lib
            .set_sync_points(&audio.id, vec![point(1.0, 0.1)])
            .is_err());
        lib.set_audio_link(&audio.id, Some(&text.id)).unwrap();
        let link = lib
            .set_sync_points(&audio.id, vec![point(90.0, 0.5), point(10.0, 0.1)])
            .unwrap();
        assert_eq!(
            link.points.iter().map(|p| p.t).collect::<Vec<_>>(),
            [10.0, 90.0]
        );
        assert_eq!(
            lib.audiobooks_for(&text.id).unwrap(),
            std::slice::from_ref(&audio.id)
        );
        assert!(lib.set_audio_link(&text.id, Some(&audio.id)).is_err());
        // Found points replace found points; ones that clash with points
        // set by hand are left out.
        let auto = |t, p| SyncPoint {
            auto: true,
            ..point(t, p)
        };
        lib.add_auto_points(&audio.id, vec![auto(50.0, 0.3), auto(95.0, 0.55)])
            .unwrap();
        let link2 = lib
            .add_auto_points(
                &audio.id,
                vec![auto(40.0, 0.2), auto(60.0, 0.05), auto(200.0, 0.8)],
            )
            .unwrap();
        assert_eq!(
            link2
                .points
                .iter()
                .map(|p| (p.t, p.auto))
                .collect::<Vec<_>>(),
            [(10.0, false), (40.0, true), (90.0, false), (200.0, true)]
        );
        let words = lib.sync_words(&text.id).unwrap();
        assert!(words.iter().any(|w| w.word == "Body"));
        assert!(lib.audio_file(&audio.id).is_ok() && lib.audio_file(&text.id).is_err());
        // Unlinking forgets the points and the file.
        lib.set_audio_link(&audio.id, None).unwrap();
        assert_eq!(lib.audio_link(&audio.id).unwrap(), AudioLink::default());
        assert!(!link_path(lib.layout(), &audio.id).exists());
    }
}
