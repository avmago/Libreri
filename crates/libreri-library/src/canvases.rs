//! Handwriting canvases: Excalidraw drawings kept as ordinary
//! `.excalidraw` files in `Notes/<profile>/Canvases/`, so they open in
//! excalidraw.com and other tools too. Libreri's own details (the book a
//! canvas belongs to, its paper) are in a `libreri` object that other tools
//! ignore; the book is a `libreri://book/<id>` link, like everywhere else.

use crate::{Error, Library, Result};
use libreri_core::BookId;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

pub const CANVAS_DIR: &str = "Canvases";
const EXT: &str = "excalidraw";
/// Largest canvas saved (pictures are kept inside the file).
const LARGEST: usize = 80 * 1024 * 1024;

/// Paper a canvas is drawn on.
pub const PAPERS: &[&str] = &["plain", "lined", "grid", "dotted"];

/// A canvas in the profile's notes folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CanvasEntry {
    pub title: String,
    /// Library-relative: "Notes/Jane Smith/Canvases/Optics sketches.excalidraw".
    pub rel_path: String,
    /// The book it belongs to (current id), if any.
    pub book_id: Option<BookId>,
    pub paper: String,
    /// Seconds since 1970.
    pub modified: i64,
    /// How many things are drawn on it.
    pub elements: u32,
}

#[derive(Deserialize, Default)]
struct Head {
    #[serde(default)]
    elements: Vec<serde::de::IgnoredAny>,
    #[serde(default)]
    libreri: Meta,
}

#[derive(Deserialize, Default)]
struct Meta {
    #[serde(default)]
    book: Option<String>,
    #[serde(default)]
    paper: Option<String>,
}

fn book_of(link: &str) -> Option<BookId> {
    let id = link.strip_prefix("libreri://book/")?;
    BookId::from_hex(id.split('#').next()?.trim()).ok()
}

impl Library {
    fn canvas_dir(&self) -> Result<PathBuf> {
        Ok(self.own_notes_dir()?.join(CANVAS_DIR))
    }

    /// The profile's canvases, newest first; only those for `book` when
    /// given. Canvases for books this profile cannot see stay hidden.
    pub fn canvases(&self, book: Option<&BookId>) -> Result<Vec<CanvasEntry>> {
        let Ok(dir) = self.canvas_dir() else {
            return Ok(Vec::new());
        };
        let want = book.map(|b| self.book(b).map(|x| x.id)).transpose()?;
        let mut out = Vec::new();
        for e in walkdir::WalkDir::new(&dir)
            .max_depth(2)
            .into_iter()
            .flatten()
        {
            let path = e.path();
            if !e.file_type().is_file() || path.extension().is_none_or(|x| x != EXT) {
                continue;
            }
            let Ok(text) = fs::read_to_string(path) else {
                continue;
            };
            let head: Head = serde_json::from_str(&text).unwrap_or_default();
            let linked = head
                .libreri
                .book
                .as_deref()
                .and_then(book_of)
                .and_then(|id| self.with_db(|db| db.resolve_book_id(&id)).ok().flatten());
            if let Some(id) = &linked {
                match self.record(id) {
                    Ok(b) if self.may_open(&b.rel_path) => {}
                    _ => continue,
                }
            }
            if want.is_some() && linked != want {
                continue;
            }
            let Some(rel) = crate::paths::rel_of(self.layout(), path) else {
                continue;
            };
            out.push(CanvasEntry {
                title: path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                rel_path: rel,
                book_id: linked,
                paper: head.libreri.paper.unwrap_or_else(|| "plain".into()),
                modified: fs::metadata(path).map_or(0, |m| crate::paths::mtime_secs(&m)),
                elements: head.elements.len() as u32,
            });
        }
        out.sort_by_key(|c| std::cmp::Reverse(c.modified));
        Ok(out)
    }

    /// Starts a canvas, for a book or on its own. Returns its path.
    pub fn create_canvas(&self, title: &str, book: Option<&BookId>, paper: &str) -> Result<String> {
        if !PAPERS.contains(&paper) {
            return Err(Error::InvalidInput("unknown paper".into()));
        }
        let book = book.map(|b| self.book(b)).transpose()?;
        let dir = self.notes_folder()?.join(CANVAS_DIR);
        fs::create_dir_all(&dir)?;
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        let title = if title.is_empty() {
            book.as_ref()
                .map(|b| format!("{} sketches", b.metadata.title))
                .unwrap_or_else(|| "Canvas".into())
        } else {
            title
        };
        let file = crate::paths::unique_path(
            &dir,
            &format!("{}.{EXT}", crate::reading::notes_folder_name(&title)),
        );
        let doc = serde_json::json!({
            "type": "excalidraw",
            "version": 2,
            "source": "Libreri",
            "elements": [],
            "appState": { "viewBackgroundColor": "#ffffff", "gridSize": 20 },
            "files": {},
            "libreri": {
                "book": book.map(|b| format!("libreri://book/{}", b.id)),
                "paper": paper,
            },
        });
        crate::paths::write_atomic(&file, serde_json::to_string_pretty(&doc)?.as_bytes())?;
        crate::paths::rel_of(self.layout(), &file).ok_or(Error::BookNotFound)
    }

    /// Resolves a canvas path, refusing anything outside the signed-in
    /// profile's canvases.
    fn own_canvas(&self, rel: &str) -> Result<PathBuf> {
        let dir = self.canvas_dir()?;
        self.layout()
            .resolve_relative(rel)
            .filter(|p| p.starts_with(&dir) && p.extension().is_some_and(|e| e == EXT))
            .ok_or_else(|| Error::NotAllowed("that canvas belongs to someone else".into()))
    }

    pub fn read_canvas(&self, rel: &str) -> Result<String> {
        Ok(fs::read_to_string(self.own_canvas(rel)?)?)
    }

    /// Saves a canvas. It must be an Excalidraw file; Libreri's details are
    /// kept even if the interface left them out.
    pub fn write_canvas(&self, rel: &str, content: &str) -> Result<()> {
        let path = self.own_canvas(rel)?;
        if content.len() > LARGEST {
            return Err(Error::InvalidInput(
                "the canvas is too large (pictures are kept inside it)".into(),
            ));
        }
        let mut doc: serde_json::Value = serde_json::from_str(content)
            .map_err(|_| Error::InvalidInput("the canvas could not be read".into()))?;
        if doc.get("type").and_then(|t| t.as_str()) != Some("excalidraw") {
            return Err(Error::InvalidInput(
                "that is not an Excalidraw drawing".into(),
            ));
        }
        if doc.get("libreri").is_none() {
            if let Ok(old) = fs::read_to_string(&path) {
                if let Some(meta) = serde_json::from_str::<serde_json::Value>(&old)
                    .ok()
                    .and_then(|o| o.get("libreri").cloned())
                {
                    doc["libreri"] = meta;
                }
            }
        }
        crate::paths::write_atomic(&path, serde_json::to_string(&doc)?.as_bytes())?;
        Ok(())
    }

    /// Changes a canvas's paper.
    pub fn set_canvas_paper(&self, rel: &str, paper: &str) -> Result<()> {
        if !PAPERS.contains(&paper) {
            return Err(Error::InvalidInput("unknown paper".into()));
        }
        let path = self.own_canvas(rel)?;
        let mut doc: serde_json::Value = serde_json::from_str(&fs::read_to_string(&path)?)?;
        doc["libreri"]["paper"] = serde_json::Value::String(paper.to_owned());
        crate::paths::write_atomic(&path, serde_json::to_string(&doc)?.as_bytes())?;
        Ok(())
    }

    /// Renames a canvas. Returns its new path.
    pub fn rename_canvas(&self, rel: &str, title: &str) -> Result<String> {
        let path = self.own_canvas(rel)?;
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        if title.is_empty() {
            return Err(Error::InvalidInput("give the canvas a name".into()));
        }
        let dir = path.parent().ok_or(Error::BookNotFound)?;
        let name = format!("{}.{EXT}", crate::reading::notes_folder_name(&title));
        if path
            .file_name()
            .is_some_and(|n| n.to_string_lossy() == name)
        {
            return Ok(rel.to_owned());
        }
        let to = crate::paths::unique_path(dir, &name);
        fs::rename(&path, &to)?;
        crate::paths::rel_of(self.layout(), &to).ok_or(Error::BookNotFound)
    }

    /// Moves a canvas to the system trash.
    pub fn delete_canvas(&self, rel: &str) -> Result<()> {
        let path = self.own_canvas(rel)?;
        // No fallback to a permanent delete: if there is no trash, say so.
        trash::delete(&path).map_err(|e| Error::Trash(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::{library, md_book};
    use crate::NoProgress;
    use libreri_core::BookQuery;

    #[test]
    fn keeps_canvases_as_excalidraw_files() {
        let (_d, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "The Lighthouse");
        lib.scan(&NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap().remove(0);

        let rel = lib.create_canvas("", Some(&book.id), "lined").unwrap();
        assert!(
            rel.ends_with("Canvases/The Lighthouse sketches.excalidraw"),
            "{rel}"
        );
        let other = lib.create_canvas("Loose ideas", None, "grid").unwrap();
        assert!(lib.create_canvas("x", None, "tartan").is_err());

        let text = lib.read_canvas(&rel).unwrap();
        let mut doc: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(doc["type"], "excalidraw");
        // The interface saves without Libreri's details: they are kept.
        doc.as_object_mut().unwrap().remove("libreri");
        doc["elements"] = serde_json::json!([{ "type": "freedraw" }, { "type": "text" }]);
        lib.write_canvas(&rel, &doc.to_string()).unwrap();
        assert!(lib.write_canvas(&rel, "{\"type\":\"other\"}").is_err());
        assert!(lib
            .read_canvas("Notes/Someone/Canvases/x.excalidraw")
            .is_err());

        let for_book = lib.canvases(Some(&book.id)).unwrap();
        assert_eq!(for_book.len(), 1);
        assert_eq!(
            (for_book[0].paper.as_str(), for_book[0].elements),
            ("lined", 2)
        );
        assert_eq!(for_book[0].book_id.as_ref(), Some(&book.id));
        assert_eq!(lib.canvases(None).unwrap().len(), 2);

        lib.set_canvas_paper(&other, "dotted").unwrap();
        let renamed = lib.rename_canvas(&other, "Loose  sketches").unwrap();
        assert!(renamed.ends_with("Loose sketches.excalidraw"));
        let all = lib.canvases(None).unwrap();
        assert!(all
            .iter()
            .any(|c| c.paper == "dotted" && c.title == "Loose sketches"));
        lib.delete_canvas(&renamed).unwrap();
        assert_eq!(lib.canvases(None).unwrap().len(), 1);
    }
}
