//! Zotero's data folder: `zotero.sqlite` and `storage/<key>/<file>`.
//!
//! Zotero keeps its database locked while it runs, so a copy is read.
//! Items in the trash are left out. Collections become categories, tags stay
//! tags, child notes become notebook text, and PDF highlights and notes
//! (Zotero 6 and later) come along with their page and rectangles.

use super::{
    foreign_file, nearest_colour, sql_time, stable_uuid, ForeignAnnotation, ForeignBook,
    ForeignError, ForeignLibrary, ForeignMark, ForeignSource,
};
use libreri_core::{BookMetadata, ContentType, FileType};
use libreri_formats::xml::strip_html;
use libreri_metadata::normalise;
use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A copy of the database in the temp folder, removed when dropped.
struct TempCopy(PathBuf);

impl Drop for TempCopy {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

fn open(dir: &Path) -> Result<(Connection, TempCopy), ForeignError> {
    let db = dir.join("zotero.sqlite");
    if !db.is_file() {
        return Err(ForeignError::NotRecognised(
            "this folder is not Zotero's data folder (there is no zotero.sqlite)".into(),
        ));
    }
    let tmp = std::env::temp_dir().join(format!(
        "libreri-zotero-{}-{}.sqlite",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or(0)
    ));
    std::fs::copy(&db, &tmp)?;
    let copy = TempCopy(tmp);
    let conn = Connection::open_with_flags(
        &copy.0,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )?;
    Ok((conn, copy))
}

fn content_type(zotero_type: &str) -> Option<ContentType> {
    Some(match zotero_type {
        "book" | "bookSection" => ContentType::Book,
        "journalArticle" => ContentType::ResearchPaper,
        "conferencePaper" => ContentType::ConferencePaper,
        "preprint" => ContentType::Preprint,
        "thesis" => ContentType::Thesis,
        "report" => ContentType::TechnicalReport,
        "magazineArticle" | "newspaperArticle" | "blogPost" | "webpage" => ContentType::Article,
        "presentation" => ContentType::Slides,
        "standard" => ContentType::Standard,
        "manuscript" | "document" | "letter" => ContentType::Other,
        "attachment" | "note" | "annotation" => return None,
        _ => ContentType::Other,
    })
}

fn has_table(conn: &Connection, name: &str) -> Result<bool, ForeignError> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type='table' AND name=?1",
            [name],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// "arXiv: 1706.03762" in Extra, an arXiv URL, or an arXiv DOI.
fn arxiv_id(extra: Option<&str>, url: Option<&str>, doi: Option<&str>) -> Option<String> {
    if let Some(e) = extra {
        for line in e.lines() {
            if let Some(rest) = line.trim().strip_prefix("arXiv:") {
                let id = rest.trim();
                if !id.is_empty() {
                    return Some(id.to_owned());
                }
            }
        }
    }
    if let Some(u) = url {
        if let Some(i) = u.find("arxiv.org/abs/") {
            let id = &u[i + "arxiv.org/abs/".len()..];
            return Some(
                id.trim_end_matches(".pdf")
                    .split(['?', '#'])
                    .next()?
                    .to_owned(),
            );
        }
    }
    doi.and_then(|d| {
        let lower = d.to_lowercase();
        lower
            .strip_prefix("10.48550/arxiv.")
            .map(|_| d["10.48550/arXiv.".len()..].to_owned())
    })
}

struct Attachment {
    item: i64,
    parent: i64,
    path: PathBuf,
}

pub fn read(dir: &Path) -> Result<ForeignLibrary, ForeignError> {
    let (conn, _copy) = open(dir)?;
    let mut warnings = Vec::new();

    // Items in the trash.
    let deleted: std::collections::HashSet<i64> = if has_table(&conn, "deletedItems")? {
        let mut stmt = conn.prepare("SELECT itemID FROM deletedItems")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect::<rusqlite::Result<_>>()?
    } else {
        Default::default()
    };

    // Every field of every item.
    let mut fields: HashMap<i64, HashMap<String, String>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT d.itemID, f.fieldName, v.value FROM itemData d
             JOIN fields f ON f.fieldID = d.fieldID
             JOIN itemDataValues v ON v.valueID = d.valueID",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, rusqlite::types::Value>(2)?,
            ))
        })?;
        for row in rows {
            let (item, field, value) = row?;
            let value = match value {
                rusqlite::types::Value::Text(s) => s,
                rusqlite::types::Value::Integer(i) => i.to_string(),
                rusqlite::types::Value::Real(f) => f.to_string(),
                _ => continue,
            };
            fields.entry(item).or_default().insert(field, value);
        }
    }

    // Creators, in order.
    let mut creators: HashMap<i64, Vec<(String, String)>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT ic.itemID, c.firstName, c.lastName, c.fieldMode, t.creatorType
             FROM itemCreators ic JOIN creators c ON c.creatorID = ic.creatorID
             JOIN creatorTypes t ON t.creatorTypeID = ic.creatorTypeID
             ORDER BY ic.itemID, ic.orderIndex",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, Option<String>>(1)?.unwrap_or_default(),
                r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                r.get::<_, Option<i64>>(3)?.unwrap_or(0),
                r.get::<_, String>(4)?,
            ))
        })?;
        for row in rows {
            let (item, first, last, mode, kind) = row?;
            let name = if mode == 1 || first.is_empty() {
                last
            } else {
                format!("{first} {last}")
            };
            creators.entry(item).or_default().push((kind, name));
        }
    }

    // Tags.
    let mut tags: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare(
            "SELECT it.itemID, t.name FROM itemTags it JOIN tags t ON t.tagID = it.tagID ORDER BY t.name",
        )?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?;
        for row in rows {
            let (item, t) = row?;
            tags.entry(item).or_default().push(t);
        }
    }

    // Collections as "Parent/Child" paths.
    let mut collection_path: HashMap<i64, String> = HashMap::new();
    {
        let mut stmt = conn
            .prepare("SELECT collectionID, collectionName, parentCollectionID FROM collections")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Option<i64>>(2)?,
            ))
        })?;
        let all: HashMap<i64, (String, Option<i64>)> = rows
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .map(|(id, name, parent)| (id, (name.replace('/', "-"), parent)))
            .collect();
        for id in all.keys() {
            let mut parts = Vec::new();
            let mut at = Some(*id);
            while let Some(c) = at {
                let Some((name, parent)) = all.get(&c) else {
                    break;
                };
                parts.push(name.clone());
                at = *parent;
                if parts.len() > 20 {
                    break;
                }
            }
            parts.reverse();
            collection_path.insert(*id, parts.join("/"));
        }
    }
    let mut categories: HashMap<i64, Vec<String>> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT itemID, collectionID FROM collectionItems")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        for row in rows {
            let (item, c) = row?;
            if let Some(p) = collection_path.get(&c) {
                categories.entry(item).or_default().push(p.clone());
            }
        }
    }

    // Attachments that are files on this computer.
    let mut attachments: Vec<Attachment> = Vec::new();
    let mut item_keys: HashMap<i64, (i64, String)> = HashMap::new();
    {
        let mut stmt = conn.prepare("SELECT itemID, libraryID, key FROM items")?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, lib, key) = row?;
            item_keys.insert(id, (lib, key));
        }
    }
    {
        let mut stmt = conn.prepare(
            "SELECT itemID, parentItemID, linkMode, path FROM itemAttachments
             WHERE parentItemID IS NOT NULL AND path IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                r.get::<_, String>(3)?,
            ))
        })?;
        for row in rows {
            let (item, parent, mode, path) = row?;
            if deleted.contains(&item) {
                continue;
            }
            let file = if let Some(name) = path.strip_prefix("storage:") {
                let Some((_, key)) = item_keys.get(&item) else {
                    continue;
                };
                dir.join("storage").join(key).join(name)
            } else if path.starts_with("attachments:") {
                warnings.push(format!(
                    "{path}: linked files relative to Zotero's base folder are not imported"
                ));
                continue;
            } else if mode == 2 {
                PathBuf::from(path)
            } else {
                continue;
            };
            attachments.push(Attachment {
                item,
                parent,
                path: file,
            });
        }
    }

    // Highlights and notes on attachments (Zotero 6+).
    let mut marks: HashMap<i64, Vec<ForeignAnnotation>> = HashMap::new();
    if has_table(&conn, "itemAnnotations")? {
        let path_of: HashMap<i64, &PathBuf> =
            attachments.iter().map(|a| (a.item, &a.path)).collect();
        let mut stmt = conn.prepare(
            "SELECT a.itemID, a.parentItemID, a.type, a.text, a.comment, a.color, a.pageLabel,
                    a.position, i.dateAdded, i.dateModified
             FROM itemAnnotations a JOIN items i ON i.itemID = a.itemID",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, Option<String>>(3)?,
                r.get::<_, Option<String>>(4)?,
                r.get::<_, Option<String>>(5)?,
                r.get::<_, Option<String>>(6)?,
                r.get::<_, String>(7)?,
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
            ))
        })?;
        for row in rows {
            let (item, parent, kind, text, comment, colour, label, position, added, modified) =
                row?;
            if deleted.contains(&item) {
                continue;
            }
            // 1 highlight, 2 note, 5 underline; images, ink and text boxes are skipped.
            let mark = match kind {
                1 | 5 => ForeignMark::Highlight,
                2 => ForeignMark::Note,
                _ => continue,
            };
            let Some(file) = path_of.get(&parent) else {
                continue;
            };
            let Ok(pos) = serde_json::from_str::<serde_json::Value>(&position) else {
                continue;
            };
            let Some(page_index) = pos.get("pageIndex").and_then(serde_json::Value::as_u64) else {
                continue;
            };
            let rects: Vec<[f64; 4]> = pos
                .get("rects")
                .and_then(|r| r.as_array())
                .map(|list| {
                    list.iter()
                        .filter_map(|r| {
                            let a = r.as_array()?;
                            let n = |i: usize| a.get(i)?.as_f64();
                            Some([n(0)?, n(1)?, n(2)?, n(3)?])
                        })
                        .collect()
                })
                .unwrap_or_default();
            let (lib, key) = item_keys
                .get(&item)
                .cloned()
                .unwrap_or((0, item.to_string()));
            let added = added.as_deref().and_then(sql_time).unwrap_or_default();
            marks.entry(parent).or_default().push(ForeignAnnotation {
                id: stable_uuid(&format!("zotero:{lib}:{key}")),
                file: (*file).clone(),
                mark,
                color: nearest_colour(colour.as_deref().unwrap_or("#ffd400")),
                text: text.filter(|t| !t.trim().is_empty()),
                comment: comment.filter(|t| !t.trim().is_empty()),
                page_index: page_index as u32,
                page_label: label.filter(|l| !l.trim().is_empty()),
                rects,
                modified_at: modified
                    .as_deref()
                    .and_then(sql_time)
                    .unwrap_or_else(|| added.clone()),
                created_at: added,
            });
        }
    }

    // Child notes.
    let mut notes: HashMap<i64, Vec<String>> = HashMap::new();
    if has_table(&conn, "itemNotes")? {
        let mut stmt = conn.prepare(
            "SELECT itemID, parentItemID, note FROM itemNotes WHERE parentItemID IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, Option<String>>(2)?,
            ))
        })?;
        for row in rows {
            let (item, parent, note) = row?;
            if deleted.contains(&item) {
                continue;
            }
            let text = strip_html(&note.unwrap_or_default());
            let text = text.trim();
            if !text.is_empty() {
                notes.entry(parent).or_default().push(text.to_owned());
            }
        }
    }

    // The items themselves.
    let mut books = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT i.itemID, i.libraryID, i.key, t.typeName, i.dateAdded FROM items i
         JOIN itemTypes t ON t.itemTypeID = i.itemTypeID ORDER BY i.itemID",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, i64>(1)?,
            r.get::<_, String>(2)?,
            r.get::<_, String>(3)?,
            r.get::<_, Option<String>>(4)?,
        ))
    })?;
    for row in rows {
        let (id, lib, key, type_name, added) = row?;
        if deleted.contains(&id) {
            continue;
        }
        let Some(ct) = content_type(&type_name) else {
            continue;
        };
        let f = fields.remove(&id).unwrap_or_default();
        let get = |k: &str| {
            f.get(k)
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
        };
        let people = creators.remove(&id).unwrap_or_default();
        let authors: Vec<String> = people
            .iter()
            .filter(|(k, _)| k == "author" || k == "presenter" || k == "inventor")
            .map(|(_, n)| n.clone())
            .collect();
        let contributors: Vec<String> = people
            .iter()
            .filter(|(k, _)| !(k == "author" || k == "presenter" || k == "inventor"))
            .map(|(k, n)| format!("{n} ({k})"))
            .collect();
        let doi = get("DOI");
        let url = get("url");
        let isbn = get("ISBN").map(|s| {
            s.split([' ', ','])
                .next()
                .unwrap_or_default()
                .replace('-', "")
        });
        let (isbn13, isbn10) = match isbn {
            Some(s) if s.len() == 10 => (None, Some(s)),
            Some(s) => (Some(s), None),
            None => (None, None),
        };
        let metadata = BookMetadata {
            title: get("title").unwrap_or_default(),
            authors,
            contributors,
            about: get("abstractNote").and_then(|a| normalise::about(&a)),
            tags: tags.remove(&id).unwrap_or_default(),
            categories: categories.remove(&id).unwrap_or_default(),
            year: get("date").as_deref().and_then(normalise::year),
            publisher: get("publisher")
                .or_else(|| get("university"))
                .or_else(|| get("institution")),
            pages: get("numPages").and_then(|p| p.trim().parse().ok()),
            isbn13,
            isbn10,
            edition: get("edition"),
            language: get("language").and_then(|l| normalise::language(&l)),
            content_type: ct,
            series: get("series"),
            series_number: get("seriesNumber").and_then(|n| n.parse().ok()),
            arxiv_id: arxiv_id(get("extra").as_deref(), url.as_deref(), doi.as_deref()),
            doi,
            journal: get("publicationTitle")
                .or_else(|| get("proceedingsTitle"))
                .or_else(|| get("bookTitle"))
                .or_else(|| get("conferenceName")),
            volume: get("volume"),
            issue: get("issue"),
            url,
            ..Default::default()
        };
        let mut book = ForeignBook::new(format!("zotero:{lib}:{key}"), metadata);
        // Files: the one with the most highlights first, then PDFs, then the rest.
        let mut files: Vec<(usize, bool, super::ForeignFile, i64)> = attachments
            .iter()
            .filter(|a| a.parent == id)
            .filter_map(|a| {
                let f = foreign_file(a.path.clone())?;
                let n = marks.get(&a.item).map_or(0, Vec::len);
                Some((n, f.file_type == FileType::Pdf, f, a.item))
            })
            .collect();
        files.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
        for (_, _, f, att) in files {
            book.annotations
                .extend(marks.remove(&att).unwrap_or_default());
            book.files.push(f);
        }
        book.notes = notes.remove(&id).unwrap_or_default();
        book.added_at = added.as_deref().and_then(sql_time);
        books.push(book);
    }
    Ok(ForeignLibrary {
        source: ForeignSource::Zotero,
        books,
        warnings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use libreri_core::HighlightColor;

    #[test]
    fn reads_items_files_highlights_and_notes() {
        let dir = tempfile::tempdir().unwrap();
        crate::foreign::zotero_fixture(dir.path());
        let lib = crate::foreign::read(dir.path(), ForeignSource::Zotero).unwrap();
        assert_eq!(lib.books.len(), 2, "trashed item left out");
        let paper = lib
            .books
            .iter()
            .find(|b| b.metadata.year == Some(2017))
            .unwrap();
        let m = &paper.metadata;
        assert_eq!(m.title, "Attention Is All You Need");
        assert_eq!(m.content_type, ContentType::ConferencePaper);
        assert_eq!(m.authors, ["Ashish Vaswani"]);
        assert_eq!(m.contributors, ["Google Brain (editor)"]);
        assert_eq!(
            m.journal.as_deref(),
            Some("Advances in Neural Information Processing Systems")
        );
        assert_eq!(m.arxiv_id.as_deref(), Some("1706.03762"));
        assert_eq!(m.tags, ["transformers"]);
        assert_eq!(m.categories, ["ML/NLP"]);
        assert_eq!(paper.files.len(), 1);
        assert_eq!(paper.annotations.len(), 1, "ink is skipped");
        let a = &paper.annotations[0];
        assert_eq!(a.text.as_deref(), Some("attention function"));
        assert_eq!(a.comment.as_deref(), Some("Key idea"));
        assert_eq!(a.color, HighlightColor::Green);
        assert_eq!(a.page_index, 2);
        assert_eq!(a.rects, vec![[100.0, 600.0, 300.0, 612.0]]);
        assert_eq!(a.modified_at, "2024-01-04T03:04:05Z");
        assert_eq!(paper.notes, ["Read section 3 again."]);

        let dune = lib
            .books
            .iter()
            .find(|b| b.metadata.title == "Dune")
            .unwrap();
        assert!(dune.files.is_empty());
        assert_eq!(dune.metadata.isbn13.as_deref(), Some("9780441013593"));
    }

    #[test]
    fn finds_arxiv_ids() {
        assert_eq!(
            arxiv_id(None, Some("https://arxiv.org/abs/2101.00001v2"), None).as_deref(),
            Some("2101.00001v2")
        );
        assert_eq!(
            arxiv_id(None, None, Some("10.48550/arXiv.1706.03762")).as_deref(),
            Some("1706.03762")
        );
        assert_eq!(arxiv_id(Some("PMID: 1"), None, None), None);
    }
}
