//! Goodreads and StoryGraph exports: what you read, want to read and how
//! you rated it. They have no book files; Libreri only uses them to update
//! books it already has.

use super::{ForeignBook, ForeignError, ForeignLibrary, ForeignPersonal, ForeignSource};
use libreri_core::{BookMetadata, ReadingStatus};
use std::collections::HashMap;
use std::path::Path;

fn rows(path: &Path) -> Result<(HashMap<String, usize>, Vec<csv::StringRecord>), ForeignError> {
    let bytes = std::fs::read(path)?;
    let text = String::from_utf8_lossy(&bytes);
    let text = text.trim_start_matches('\u{feff}');
    let mut r = csv::ReaderBuilder::new()
        .flexible(true)
        .from_reader(text.as_bytes());
    let header: HashMap<String, usize> = r
        .headers()
        .map_err(|e| ForeignError::Unreadable(e.to_string()))?
        .iter()
        .enumerate()
        .map(|(i, h)| (h.trim().to_owned(), i))
        .collect();
    let records = r
        .records()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| ForeignError::Unreadable(e.to_string()))?;
    Ok((header, records))
}

/// Goodreads writes ISBNs as `="0441013597"`.
fn isbn_cell(s: &str) -> Option<String> {
    let digits: String = s
        .chars()
        .filter(|c| c.is_ascii_digit() || *c == 'X' || *c == 'x')
        .collect();
    (digits.len() == 10 || digits.len() == 13).then_some(digits)
}

/// "2021/03/04" or "2021-03-04" → RFC 3339 at midnight UTC.
fn day(s: &str) -> Option<String> {
    let s = s.trim().replace('/', "-");
    chrono::NaiveDate::parse_from_str(s.get(..10)?, "%Y-%m-%d")
        .ok()
        .map(|d| format!("{d}T00:00:00Z"))
}

fn record(header: &HashMap<String, usize>, r: &csv::StringRecord, name: &str) -> String {
    header
        .get(name)
        .and_then(|i| r.get(*i))
        .unwrap_or("")
        .trim()
        .to_owned()
}

pub fn read_goodreads(path: &Path) -> Result<ForeignLibrary, ForeignError> {
    let (h, rows) = rows(path)?;
    if !h.contains_key("Exclusive Shelf") || !h.contains_key("Title") {
        return Err(ForeignError::NotRecognised(
            "this is not a Goodreads library export".into(),
        ));
    }
    let mut books = Vec::new();
    for r in rows {
        let get = |n: &str| record(&h, &r, n);
        let mut authors = vec![get("Author")];
        authors.extend(
            get("Additional Authors")
                .split(',')
                .map(|a| a.trim().to_owned()),
        );
        let status = match get("Exclusive Shelf").as_str() {
            "read" => Some(ReadingStatus::Finished),
            "currently-reading" => Some(ReadingStatus::Reading),
            "to-read" => Some(ReadingStatus::WantToRead),
            "did-not-finish" | "abandoned" | "dnf" => Some(ReadingStatus::Abandoned),
            _ => None,
        };
        let metadata = BookMetadata {
            title: get("Title"),
            authors: authors.into_iter().filter(|a| !a.is_empty()).collect(),
            isbn13: isbn_cell(&get("ISBN13")),
            isbn10: isbn_cell(&get("ISBN")),
            publisher: Some(get("Publisher")).filter(|p| !p.is_empty()),
            pages: get("Number of Pages").parse().ok(),
            year: get("Original Publication Year")
                .parse()
                .ok()
                .or_else(|| get("Year Published").parse().ok()),
            ..Default::default()
        };
        let mut b = ForeignBook::new(format!("goodreads:{}", get("Book Id")), metadata);
        b.personal = Some(ForeignPersonal {
            status,
            rating: get("My Rating").parse::<u8>().unwrap_or(0).min(5),
            favorite: get("Bookshelves")
                .split(',')
                .any(|s| matches!(s.trim(), "favorites" | "favourites")),
            last_read: day(&get("Date Read")),
        });
        b.added_at = day(&get("Date Added"));
        books.push(b);
    }
    Ok(ForeignLibrary {
        source: ForeignSource::Goodreads,
        books,
        warnings: Vec::new(),
    })
}

pub fn read_storygraph(path: &Path) -> Result<ForeignLibrary, ForeignError> {
    let (h, rows) = rows(path)?;
    if !h.contains_key("Read Status") || !h.contains_key("Title") {
        return Err(ForeignError::NotRecognised(
            "this is not a StoryGraph export".into(),
        ));
    }
    let mut books = Vec::new();
    for (n, r) in rows.iter().enumerate() {
        let get = |name: &str| record(&h, r, name);
        let status = match get("Read Status").as_str() {
            "read" => Some(ReadingStatus::Finished),
            "currently-reading" => Some(ReadingStatus::Reading),
            "to-read" => Some(ReadingStatus::WantToRead),
            "did-not-finish" => Some(ReadingStatus::Abandoned),
            _ => None,
        };
        let isbn = isbn_cell(&get("ISBN/UID"));
        let (isbn13, isbn10) = match isbn {
            Some(i) if i.len() == 10 => (None, Some(i)),
            other => (other, None),
        };
        let metadata = BookMetadata {
            title: get("Title"),
            authors: get("Authors")
                .split(',')
                .map(|a| a.trim().to_owned())
                .filter(|a| !a.is_empty())
                .collect(),
            isbn13,
            isbn10,
            ..Default::default()
        };
        let mut b = ForeignBook::new(format!("storygraph:{n}"), metadata);
        let rating = get("Star Rating")
            .parse::<f64>()
            .map_or(0, |r| (r.round() as u8).min(5));
        b.personal = Some(ForeignPersonal {
            status,
            rating,
            favorite: false,
            last_read: day(&get("Last Date Read")),
        });
        b.added_at = day(&get("Date Added"));
        books.push(b);
    }
    Ok(ForeignLibrary {
        source: ForeignSource::StoryGraph,
        books,
        warnings: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_goodreads() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("goodreads_library_export.csv");
        std::fs::write(
            &p,
            "Book Id,Title,Author,Author l-f,Additional Authors,ISBN,ISBN13,My Rating,Average Rating,Publisher,Binding,Number of Pages,Year Published,Original Publication Year,Date Read,Date Added,Bookshelves,Bookshelves with positions,Exclusive Shelf\n\
             234225,Dune,Frank Herbert,\"Herbert, Frank\",,\"=\"\"0441013597\"\"\",\"=\"\"9780441013593\"\"\",5,4.25,Ace,Paperback,658,2005,1965,2021/03/04,2020/01/01,\"favorites, sci-fi\",,read\n\
             1,Unknown,Someone,,,\"=\"\"\"\"\",\"=\"\"\"\"\",0,3,,,,,,,2020/01/01,,,to-read\n",
        )
        .unwrap();
        assert_eq!(crate::foreign::detect(&p), Some(ForeignSource::Goodreads));
        let lib = crate::foreign::read(&p, ForeignSource::Goodreads).unwrap();
        assert_eq!(lib.books.len(), 2);
        let dune = &lib.books[0];
        assert_eq!(dune.metadata.isbn13.as_deref(), Some("9780441013593"));
        assert_eq!(dune.metadata.year, Some(1965));
        let p = dune.personal.as_ref().unwrap();
        assert_eq!(p.status, Some(ReadingStatus::Finished));
        assert_eq!(p.rating, 5);
        assert!(p.favorite);
        assert_eq!(p.last_read.as_deref(), Some("2021-03-04T00:00:00Z"));
        let other = &lib.books[1];
        assert_eq!(other.metadata.isbn13, None);
        assert_eq!(
            other.personal.as_ref().unwrap().status,
            Some(ReadingStatus::WantToRead)
        );
    }

    #[test]
    fn reads_storygraph() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sg.csv");
        std::fs::write(
            &p,
            "Title,Authors,Contributors,ISBN/UID,Format,Read Status,Date Added,Last Date Read,Dates Read,Read Count,Star Rating\n\
             Dune,Frank Herbert,,9780441013593,paperback,read,2020/01/01,2021/03/04,,1,4.5\n\
             Emma,Jane Austen,,,ebook,did-not-finish,2020/01/01,,,0,\n",
        )
        .unwrap();
        let lib = crate::foreign::read(&p, ForeignSource::StoryGraph).unwrap();
        assert_eq!(lib.books[0].personal.as_ref().unwrap().rating, 5);
        assert_eq!(
            lib.books[1].personal.as_ref().unwrap().status,
            Some(ReadingStatus::Abandoned)
        );
        assert!(read_goodreads(&p).is_err());
    }
}
