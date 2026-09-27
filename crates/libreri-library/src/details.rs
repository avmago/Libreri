//! Book details found online (see `libreri-metadata`): looking a book up,
//! saving the details the reader picked, and filling in missing details
//! for many books at once.

use crate::{Error, Library, Progress, Result};
use libreri_core::{Book, BookId, BookMetadata};
use libreri_metadata::{Http, Lookup, Query, Settings};

/// What "Fill in missing details" did.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FillReport {
    /// Books that got at least one new detail or a cover.
    pub filled: Vec<BookId>,
    /// Books with no sure match; they need "Find details" one by one.
    pub unsure: Vec<BookId>,
    /// Books already complete, or with no match at all.
    pub unchanged: u32,
    /// `(title, reason)` for books whose sources could not be reached.
    pub failed: Vec<(String, String)>,
}

impl Library {
    /// What to look a book up by: its details, plus an ISBN, DOI or arXiv
    /// id found in its pages when the details have none.
    pub fn details_query(&self, id: &BookId) -> Result<Query> {
        let book = self.book(id)?;
        let mut q = Query::for_book(&book.metadata);
        if q.isbn13().is_none() && q.doi().is_none() && q.arxiv().is_none() {
            if let Some(path) = self.layout().resolve_relative(&book.rel_path) {
                let found = libreri_formats::find_identifiers(&path, book.file_type);
                q.isbn = found.isbn13;
                q.doi = found.doi;
                q.arxiv_id = found.arxiv_id;
            }
        }
        Ok(q)
    }

    /// Asks the online sources about one book (or about `query`, when the
    /// reader typed their own).
    pub fn find_details(
        &self,
        id: &BookId,
        query: Option<Query>,
        settings: &Settings,
        http: &dyn Http,
    ) -> Result<Lookup> {
        self.require_edit()?;
        let q = match query {
            Some(q) => q,
            None => self.details_query(id)?,
        };
        if q.is_empty() {
            return Err(Error::InvalidInput(
                "give a title, ISBN, DOI or arXiv id to look for".into(),
            ));
        }
        Ok(libreri_metadata::lookup(&q, settings, http))
    }

    /// Saves the details the reader picked and, if a cover was picked,
    /// downloads it. The details are saved even if the cover fails; the
    /// second value then says why.
    pub fn apply_details(
        &self,
        id: &BookId,
        metadata: BookMetadata,
        cover_url: Option<&str>,
        http: &dyn Http,
    ) -> Result<(Book, Option<String>)> {
        let book = self.update_metadata(id, metadata)?;
        let cover_error = match cover_url {
            Some(url) => self.download_cover(&book.id, url, http).err(),
            None => None,
        };
        Ok((self.book(&book.id)?, cover_error))
    }

    fn download_cover(
        &self,
        id: &BookId,
        url: &str,
        http: &dyn Http,
    ) -> std::result::Result<(), String> {
        let cover = libreri_metadata::fetch_cover(http, url)?;
        self.set_cover(id, &cover.bytes).map_err(|e| e.to_string())
    }

    /// For each book, takes a sure match (an identifier, or a close title
    /// and author) and fills only the details that are empty, plus the
    /// cover if the book has none. Nothing the reader entered is replaced.
    pub fn fill_missing_details(
        &self,
        ids: &[BookId],
        settings: &Settings,
        http: &dyn Http,
        progress: &dyn Progress,
    ) -> Result<FillReport> {
        self.require_edit()?;
        let mut report = FillReport::default();
        let total = ids.len() as u64;
        for (i, id) in ids.iter().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            let Ok(book) = self.book(id) else {
                continue;
            };
            progress.report(i as u64, total, &book.metadata.title);
            let found = match self.find_details(&book.id, None, settings, http) {
                Ok(found) => found,
                Err(Error::InvalidInput(_)) => {
                    report.unchanged += 1;
                    continue;
                }
                Err(e) => return Err(e),
            };
            let Some(best) = found.sure_match() else {
                if found.candidates.is_empty() && !found.errors.is_empty() {
                    let why = found
                        .errors
                        .iter()
                        .map(|e| format!("{}: {}", e.source.name(), e.message))
                        .collect::<Vec<_>>()
                        .join("; ");
                    report.failed.push((book.metadata.title.clone(), why));
                } else if found.candidates.is_empty() {
                    report.unchanged += 1;
                } else {
                    report.unsure.push(book.id.clone());
                }
                continue;
            };
            let mut metadata = book.metadata.clone();
            let changed = !libreri_metadata::fill_empty(&mut metadata, &best.metadata).is_empty();
            let mut current = book.id.clone();
            if changed {
                match self.update_metadata(&book.id, metadata) {
                    Ok(saved) => current = saved.id,
                    Err(e) => {
                        report
                            .failed
                            .push((book.metadata.title.clone(), e.to_string()));
                        continue;
                    }
                }
            }
            let covered = !book.has_cover
                && found
                    .candidates
                    .iter()
                    .filter(|c| c.score >= libreri_metadata::SURE)
                    .filter_map(|c| c.cover_url.as_deref())
                    .any(|url| self.download_cover(&current, url, http).is_ok());
            if changed || covered {
                report.filled.push(current);
            } else {
                report.unchanged += 1;
            }
        }
        progress.report(total, total, "");
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::BookQuery;
    use libreri_metadata::{Http, Response, Settings};
    use std::sync::Mutex;

    /// Answers Open Library ISBN requests; everything else is offline.
    struct Fake {
        asked: Mutex<Vec<String>>,
    }

    fn png() -> Vec<u8> {
        let img = image::RgbImage::from_fn(200, 300, |x, y| {
            image::Rgb([(x % 255) as u8, (y % 255) as u8, 90])
        });
        let mut buf = Vec::new();
        img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    impl Http for Fake {
        fn get(&self, url: &str, _: &[(&str, &str)]) -> std::result::Result<Response, String> {
            self.asked.lock().unwrap().push(url.to_owned());
            let body = if url.contains("openlibrary.org/api/books") {
                serde_json::json!({
                    "ISBN:9780306406157": {
                        "title": "Found Title",
                        "authors": [{"name": "John Smith"}],
                        "publishers": [{"name": "Acme Press"}],
                        "publish_date": "1999",
                        "subjects": [{"name": "Algebra"}],
                        "cover": {"large": "https://covers.openlibrary.org/b/id/1-L.jpg"}
                    }
                })
                .to_string()
                .into_bytes()
            } else if url.starts_with("https://covers.openlibrary.org/") {
                png()
            } else {
                return Err("offline".into());
            };
            Ok(Response {
                status: 200,
                body,
                content_type: None,
            })
        }
    }

    #[test]
    fn fills_only_missing_details_from_sure_matches() {
        let (_d, lib) = library();
        let dir = lib.layout().books_dir();
        std::fs::write(
            dir.join("a.md"),
            "---\ntitle: My Title\nisbn: 978-0-306-40615-7\n---\nBody",
        )
        .unwrap();
        md_book(&dir, "b.md", "Unknown Thing");
        lib.scan(&NoProgress).unwrap();
        let books = lib.books(&BookQuery::default()).unwrap();
        let ids: Vec<_> = books.iter().map(|b| b.id.clone()).collect();
        let http = Fake {
            asked: Mutex::default(),
        };

        let report = lib
            .fill_missing_details(&ids, &Settings::default(), &http, &NoProgress)
            .unwrap();
        assert_eq!(report.filled.len(), 1, "{report:?}");
        let a = lib.book(&report.filled[0]).unwrap();
        assert_eq!(a.metadata.title, "My Title", "the reader's title stays");
        assert_eq!(a.metadata.publisher.as_deref(), Some("Acme Press"));
        assert_eq!(a.metadata.year, Some(1999));
        assert_eq!(a.metadata.tags, ["Algebra"]);
        assert!(a.has_cover);
        // "Unknown Thing" has no identifier; the title search finds nothing
        // because every source is offline.
        assert_eq!(report.failed.len(), 1);

        // Picking details by hand replaces them.
        let mut picked = a.metadata.clone();
        picked.title = "Found Title".into();
        let (saved, cover_error) = lib
            .apply_details(&a.id, picked, Some("https://example.com/x.jpg"), &http)
            .unwrap();
        assert_eq!(saved.metadata.title, "Found Title");
        assert!(cover_error.is_some(), "not a source host");
        assert!(!http
            .asked
            .lock()
            .unwrap()
            .iter()
            .any(|u| u.contains("example.com")));
    }

    #[test]
    fn guests_cannot_look_up() {
        let (_d, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "Alpha");
        lib.scan(&NoProgress).unwrap();
        let id = lib.books(&BookQuery::default()).unwrap()[0].id.clone();
        let guest = lib
            .create_profile("Guest", "graphite", libreri_core::ProfileKind::Guest, None)
            .unwrap();
        lib.sign_in(&guest.id, None).unwrap();
        let http = Fake {
            asked: Mutex::default(),
        };
        assert!(matches!(
            lib.find_details(&id, None, &Settings::default(), &http),
            Err(Error::NotAllowed(_))
        ));
        assert!(http.asked.lock().unwrap().is_empty());
    }
}
