//! The catalogue as a table (CSV, Excel) or as JSON.

use crate::{content_type_label, status_label, Entry};
use libreri_core::annotation::book_link;
use libreri_core::AnnotationKind;
use serde_json::{json, Value};

/// Options shared by the table and JSON exports.
#[derive(Debug, Clone, Copy, Default)]
pub struct TableOptions {
    /// Add the reader's status, rating, favourite and progress.
    pub personal: bool,
    /// JSON only: add highlights, bookmarks and the notebook.
    pub notes: bool,
}

struct Column {
    name: &'static str,
    width: f64,
    value: fn(&Entry) -> Cell,
}

enum Cell {
    Text(String),
    Number(f64),
    Empty,
}

fn text(s: impl Into<String>) -> Cell {
    let s = s.into();
    if s.is_empty() {
        Cell::Empty
    } else {
        Cell::Text(s)
    }
}

fn opt(s: &Option<String>) -> Cell {
    s.as_deref().map_or(Cell::Empty, |s| text(s.trim()))
}

fn count(e: &Entry, kind: AnnotationKind) -> Cell {
    Cell::Number(e.annotations.iter().filter(|a| a.kind == kind).count() as f64)
}

const COLUMNS: &[Column] = &[
    Column {
        name: "Title",
        width: 40.0,
        value: |e| text(e.book.metadata.title.clone()),
    },
    Column {
        name: "Subtitle",
        width: 30.0,
        value: |e| opt(&e.book.metadata.subtitle),
    },
    Column {
        name: "Authors",
        width: 30.0,
        value: |e| text(e.book.metadata.authors.join("; ")),
    },
    Column {
        name: "Contributors",
        width: 20.0,
        value: |e| text(e.book.metadata.contributors.join("; ")),
    },
    Column {
        name: "Year",
        width: 7.0,
        value: |e| {
            e.book
                .metadata
                .year
                .map_or(Cell::Empty, |y| Cell::Number(f64::from(y)))
        },
    },
    Column {
        name: "Publisher",
        width: 20.0,
        value: |e| opt(&e.book.metadata.publisher),
    },
    Column {
        name: "Pages",
        width: 7.0,
        value: |e| {
            e.book
                .metadata
                .pages
                .map_or(Cell::Empty, |p| Cell::Number(f64::from(p)))
        },
    },
    Column {
        name: "ISBN-13",
        width: 15.0,
        value: |e| opt(&e.book.metadata.isbn13),
    },
    Column {
        name: "ISBN-10",
        width: 12.0,
        value: |e| opt(&e.book.metadata.isbn10),
    },
    Column {
        name: "DOI",
        width: 20.0,
        value: |e| opt(&e.book.metadata.doi),
    },
    Column {
        name: "arXiv",
        width: 12.0,
        value: |e| opt(&e.book.metadata.arxiv_id),
    },
    Column {
        name: "Edition",
        width: 8.0,
        value: |e| opt(&e.book.metadata.edition),
    },
    Column {
        name: "Language",
        width: 9.0,
        value: |e| opt(&e.book.metadata.language),
    },
    Column {
        name: "Type",
        width: 14.0,
        value: |e| text(content_type_label(e.book.metadata.content_type)),
    },
    Column {
        name: "Series",
        width: 20.0,
        value: |e| opt(&e.book.metadata.series),
    },
    Column {
        name: "Series number",
        width: 8.0,
        value: |e| {
            e.book
                .metadata
                .series_number
                .map_or(Cell::Empty, Cell::Number)
        },
    },
    Column {
        name: "Journal",
        width: 20.0,
        value: |e| opt(&e.book.metadata.journal),
    },
    Column {
        name: "Volume",
        width: 8.0,
        value: |e| opt(&e.book.metadata.volume),
    },
    Column {
        name: "Issue",
        width: 8.0,
        value: |e| opt(&e.book.metadata.issue),
    },
    Column {
        name: "URL",
        width: 25.0,
        value: |e| opt(&e.book.metadata.url),
    },
    Column {
        name: "Tags",
        width: 25.0,
        value: |e| text(e.book.metadata.tags.join("; ")),
    },
    Column {
        name: "Categories",
        width: 25.0,
        value: |e| text(e.book.metadata.categories.join("; ")),
    },
    Column {
        name: "About",
        width: 40.0,
        value: |e| opt(&e.book.metadata.about),
    },
    Column {
        name: "File type",
        width: 8.0,
        value: |e| text(e.book.file_type.as_str().to_uppercase()),
    },
    Column {
        name: "File size (bytes)",
        width: 12.0,
        value: |e| Cell::Number(e.book.file_size as f64),
    },
    Column {
        name: "File",
        width: 40.0,
        value: |e| text(e.book.rel_path.clone()),
    },
    Column {
        name: "File missing",
        width: 8.0,
        value: |e| text(if e.book.missing { "yes" } else { "" }),
    },
    Column {
        name: "Added",
        width: 20.0,
        value: |e| text(e.book.added_at.clone()),
    },
    Column {
        name: "Libreri link",
        width: 30.0,
        value: |e| text(book_link(&e.book.id, None)),
    },
];

const PERSONAL: &[Column] = &[
    Column {
        name: "Status",
        width: 12.0,
        value: |e| text(status_label(e.book.user.status)),
    },
    Column {
        name: "Rating",
        width: 7.0,
        value: |e| {
            if e.book.user.rating == 0 {
                Cell::Empty
            } else {
                Cell::Number(f64::from(e.book.user.rating))
            }
        },
    },
    Column {
        name: "Favourite",
        width: 9.0,
        value: |e| text(if e.book.user.favorite { "yes" } else { "" }),
    },
    Column {
        name: "Progress (%)",
        width: 9.0,
        value: |e| Cell::Number((f64::from(e.book.user.progress) * 100.0).round()),
    },
    Column {
        name: "Last opened",
        width: 20.0,
        value: |e| opt(&e.book.user.last_opened),
    },
    Column {
        name: "Highlights",
        width: 9.0,
        value: |e| count(e, AnnotationKind::Highlight),
    },
    Column {
        name: "Bookmarks",
        width: 9.0,
        value: |e| count(e, AnnotationKind::Bookmark),
    },
];

fn columns(opts: TableOptions) -> Vec<&'static Column> {
    let mut cols: Vec<&Column> = COLUMNS.iter().collect();
    if opts.personal {
        cols.extend(PERSONAL.iter());
    }
    cols
}

/// Stops spreadsheet apps from running text as a formula.
fn defuse(s: &str) -> String {
    let risky = s.starts_with(['=', '+', '@', '\t', '\r'])
        || (s.starts_with('-') && s.parse::<f64>().is_err());
    if risky {
        format!("'{s}")
    } else {
        s.to_owned()
    }
}

/// CSV with a header row, UTF-8 with a byte-order mark so Excel reads
/// accents correctly.
pub fn csv(entries: &[Entry], opts: TableOptions) -> Result<Vec<u8>, String> {
    let cols = columns(opts);
    let mut w = csv::WriterBuilder::new()
        .terminator(csv::Terminator::CRLF)
        .from_writer(vec![0xEF, 0xBB, 0xBF]);
    w.write_record(cols.iter().map(|c| c.name))
        .map_err(|e| e.to_string())?;
    for e in entries {
        let row: Vec<String> = cols
            .iter()
            .map(|c| match (c.value)(e) {
                Cell::Text(s) => defuse(&s),
                Cell::Number(n) if n.fract() == 0.0 => format!("{}", n as i64),
                Cell::Number(n) => n.to_string(),
                Cell::Empty => String::new(),
            })
            .collect();
        w.write_record(&row).map_err(|e| e.to_string())?;
    }
    w.into_inner().map_err(|e| e.to_string())
}

/// An Excel workbook with one sheet, a frozen, filterable header row.
pub fn xlsx(entries: &[Entry], opts: TableOptions) -> Result<Vec<u8>, String> {
    use rust_xlsxwriter::{Format, Workbook};
    let cols = columns(opts);
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    let err = |e: rust_xlsxwriter::XlsxError| e.to_string();
    sheet.set_name("Books").map_err(err)?;
    let bold = Format::new().set_bold();
    for (c, col) in cols.iter().enumerate() {
        let c = c as u16;
        sheet
            .write_string_with_format(0, c, col.name, &bold)
            .map_err(err)?;
        sheet.set_column_width(c, col.width).map_err(err)?;
    }
    for (r, e) in entries.iter().enumerate() {
        let r = r as u32 + 1;
        for (c, col) in cols.iter().enumerate() {
            let c = c as u16;
            match (col.value)(e) {
                // Cells hold at most 32,767 characters.
                Cell::Text(s) => {
                    let s: String = s.chars().take(32_000).collect();
                    sheet.write_string(r, c, s).map_err(err)?;
                }
                Cell::Number(n) => {
                    sheet.write_number(r, c, n).map_err(err)?;
                }
                Cell::Empty => {}
            }
        }
    }
    sheet.set_freeze_panes(1, 0).map_err(err)?;
    if !entries.is_empty() {
        sheet
            .autofilter(0, 0, entries.len() as u32, cols.len() as u16 - 1)
            .map_err(err)?;
    }
    book.save_to_buffer().map_err(err)
}

/// One book as JSON: details, file, and optionally personal data and notes.
pub fn json_entry(e: &Entry, opts: TableOptions) -> Value {
    let b = &e.book;
    let mut v = json!({
        "id": b.id,
        "link": book_link(&b.id, None),
        "file": {
            "path": b.rel_path,
            "type": b.file_type,
            "size": b.file_size,
            "missing": b.missing,
        },
        "addedAt": b.added_at,
        "modifiedAt": b.modified_at,
        "details": b.metadata,
    });
    if opts.personal {
        v["personal"] = json!(b.user);
    }
    if opts.notes {
        let notes: Vec<Value> = e
            .annotations
            .iter()
            .map(|a| {
                let locator: Value =
                    serde_json::from_str(&a.locator).unwrap_or(Value::String(a.locator.clone()));
                json!({
                    "id": a.id,
                    "link": a.link(),
                    "kind": a.kind,
                    "color": a.color,
                    "quote": a.quote,
                    "note": a.note,
                    "label": a.label,
                    "position": a.position,
                    "locator": locator,
                    "createdAt": a.created_at,
                    "modifiedAt": a.modified_at,
                })
            })
            .collect();
        v["annotations"] = Value::Array(notes);
        if let Some((path, content)) = &e.notebook {
            v["notebook"] = json!({ "path": path, "markdown": content });
        }
    }
    v
}

/// The whole export as one JSON document.
pub fn json(
    entries: &[Entry],
    opts: TableOptions,
    library: &str,
    profile: Option<&str>,
    exported_at: &str,
) -> String {
    let books: Vec<Value> = entries.iter().map(|e| json_entry(e, opts)).collect();
    let doc = json!({
        "format": "libreri-books",
        "formatVersion": 1,
        "exportedAt": exported_at,
        "library": library,
        "profile": profile,
        "books": books,
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|_| "{}".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{book, highlight};

    fn entries() -> Vec<Entry> {
        let mut b = book("=HYPERLINK(\"x\")");
        b.metadata.about = Some("line one\nline \"two\"".into());
        let mut e = Entry::new(b.clone());
        e.annotations
            .push(highlight(&b, "light bends", Some("nice")));
        e.notebook = Some(("Notes/Me/Optics.md".into(), "# Optics".into()));
        vec![e, Entry::new(book("Plain"))]
    }

    #[test]
    fn csv_is_excel_friendly_and_safe() {
        let bytes = csv(
            &entries(),
            TableOptions {
                personal: true,
                notes: false,
            },
        )
        .unwrap();
        assert_eq!(&bytes[..3], &[0xEF, 0xBB, 0xBF]);
        let text = String::from_utf8(bytes[3..].to_vec()).unwrap();
        let mut r = csv::Reader::from_reader(text.as_bytes());
        let header = r.headers().unwrap().clone();
        assert_eq!(&header[0], "Title");
        assert!(header.iter().any(|h| h == "Rating"));
        let rows: Vec<csv::StringRecord> = r.records().map(Result::unwrap).collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(&rows[0][0], "'=HYPERLINK(\"x\")");
        let about = header.iter().position(|h| h == "About").unwrap();
        assert_eq!(&rows[0][about], "line one\nline \"two\"");
        let hl = header.iter().position(|h| h == "Highlights").unwrap();
        assert_eq!(&rows[0][hl], "1");

        let plain = csv(&entries(), TableOptions::default()).unwrap();
        assert!(!String::from_utf8_lossy(&plain).contains("Rating"));
    }

    #[test]
    fn xlsx_is_a_zip_workbook() {
        let bytes = xlsx(
            &entries(),
            TableOptions {
                personal: true,
                notes: false,
            },
        )
        .unwrap();
        assert_eq!(&bytes[..2], b"PK");
        let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert!(zip.by_name("xl/worksheets/sheet1.xml").is_ok());
    }

    #[test]
    fn json_carries_notes_only_when_asked() {
        let doc: Value = serde_json::from_str(&json(
            &entries(),
            TableOptions {
                personal: true,
                notes: true,
            },
            "Home",
            Some("Jane"),
            "2026-09-28T10:00:00Z",
        ))
        .unwrap();
        assert_eq!(doc["format"], "libreri-books");
        let first = &doc["books"][0];
        assert_eq!(first["details"]["year"], 2019);
        assert_eq!(first["personal"]["rating"], 4);
        assert_eq!(first["annotations"][0]["locator"]["page"], 4);
        assert!(first["annotations"][0]["link"]
            .as_str()
            .unwrap()
            .starts_with("libreri://book/"));
        assert_eq!(first["notebook"]["markdown"], "# Optics");

        let doc: Value = serde_json::from_str(&json(
            &entries(),
            TableOptions::default(),
            "Home",
            None,
            "t",
        ))
        .unwrap();
        assert!(doc["books"][0].get("annotations").is_none());
        assert!(doc["books"][0].get("personal").is_none());
    }
}
