//! The Notes hub: all of a profile's highlights, comments, bookmarks and
//! notebooks across the library.

use crate::{Library, Result};
use libreri_core::{Annotation, BookId};
use std::fs;

/// An annotation with the book it belongs to.
#[derive(Debug, Clone, PartialEq)]
pub struct NoteEntry {
    pub annotation: Annotation,
    pub book_title: String,
    pub file_type: libreri_core::FileType,
}

/// A Markdown notebook in the profile's notes folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NotebookEntry {
    /// The book it is linked to, if any (standalone notes have none).
    pub book_id: Option<BookId>,
    pub title: String,
    /// Library-relative: "Notes/Jane Smith/Linear Algebra.md".
    pub rel_path: String,
    /// Seconds since 1970.
    pub modified: i64,
    /// The first words after the front matter and title.
    pub excerpt: String,
    pub words: u32,
}

/// The body of a notebook: without front matter and the first heading.
fn body(content: &str) -> &str {
    let mut rest = content;
    if let Some(front) = rest.strip_prefix("---") {
        if let Some(end) = front.find("\n---") {
            rest = &front[end + 4..];
        }
    }
    let rest = rest.trim_start();
    match rest.strip_prefix("# ") {
        Some(after) => after.split_once('\n').map_or("", |(_, b)| b),
        None => rest,
    }
}

/// "[label](url)" becomes "label", so excerpts read as text.
fn strip_links(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('[') {
        let after = &rest[open + 1..];
        match after.find("](").and_then(|close| {
            after[close + 2..]
                .find(')')
                .map(|end| (close, close + 2 + end))
        }) {
            Some((close, end)) => {
                out.push_str(&rest[..open]);
                out.push_str(&after[..close]);
                rest = &after[end + 1..];
            }
            None => {
                out.push_str(&rest[..=open]);
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn excerpt(text: &str) -> String {
    let flat = text
        .lines()
        .map(|l| strip_links(l.trim_start_matches(['#', '>', '-', '*', ' ', '—'])))
        .filter(|l| !l.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let mut out: String = flat.chars().take(160).collect();
    if flat.chars().count() > 160 {
        out.push('…');
    }
    out
}

impl Library {
    /// The signed-in profile's notes folder, if it keeps data.
    fn own_notes_dir(&self) -> Result<std::path::PathBuf> {
        let session = self.session_info().ok_or(crate::Error::SignedOut)?;
        if !session.kind.keeps_data() {
            return Err(crate::Error::NotAllowed(
                "guests cannot keep notes; sign in to your own profile".into(),
            ));
        }
        let name = self
            .with_db(|db| db.profile_name(&session.id))?
            .unwrap_or_else(|| "Me".into());
        Ok(self
            .layout()
            .notes_dir()
            .join(crate::reading::notes_folder_name(&name)))
    }

    /// The signed-in profile's notes folder, created if needed.
    pub fn notes_folder(&self) -> Result<std::path::PathBuf> {
        let dir = self.own_notes_dir()?;
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// Bytes used by `Books/`, `Notes/` and `.library-data/`.
    pub fn storage_usage(&self) -> (u64, u64, u64) {
        let size = |dir: std::path::PathBuf| -> u64 {
            walkdir::WalkDir::new(dir)
                .into_iter()
                .flatten()
                .filter(|e| e.file_type().is_file())
                .filter_map(|e| e.metadata().ok())
                .map(|m| m.len())
                .sum()
        };
        (
            size(self.layout().books_dir()),
            size(self.layout().notes_dir()),
            size(self.layout().data_dir()),
        )
    }

    /// Resolves a notebook path, refusing anything outside the signed-in
    /// profile's notes folder.
    fn own_note_path(&self, rel: &str) -> Result<std::path::PathBuf> {
        let dir = self.own_notes_dir()?;
        let path = self
            .layout()
            .resolve_relative(rel)
            .filter(|p| p.starts_with(&dir) && p.extension().is_some_and(|e| e == "md"))
            .ok_or_else(|| crate::Error::NotAllowed("that note belongs to someone else".into()))?;
        Ok(path)
    }

    /// Reads one of the signed-in profile's notes.
    pub fn read_note(&self, rel: &str) -> Result<String> {
        Ok(fs::read_to_string(self.own_note_path(rel)?)?)
    }

    /// Saves one of the signed-in profile's notes.
    pub fn write_note(&self, rel: &str, content: &str) -> Result<()> {
        let path = self.own_note_path(rel)?;
        crate::paths::write_atomic(&path, content.as_bytes())?;
        Ok(())
    }

    /// Starts a note that is not about one book. Returns its path.
    pub fn create_note(&self, title: &str) -> Result<String> {
        let dir = self.own_notes_dir()?;
        fs::create_dir_all(&dir)?;
        let title = title.split_whitespace().collect::<Vec<_>>().join(" ");
        let title = if title.is_empty() {
            "Untitled".to_owned()
        } else {
            title
        };
        let file = crate::paths::unique_path(
            &dir,
            &format!("{}.md", crate::reading::notes_folder_name(&title)),
        );
        crate::paths::write_atomic(&file, format!("# {title}\n\n").as_bytes())?;
        crate::paths::rel_of(self.layout(), &file).ok_or(crate::Error::BookNotFound)
    }

    /// All annotations of the signed-in profile, newest first, in books it
    /// can see.
    pub fn all_notes(&self) -> Result<Vec<NoteEntry>> {
        let profile = self.profile()?;
        Ok(self
            .with_db(|db| db.all_annotations(&profile))?
            .into_iter()
            .filter(|(_, _, path)| self.may_open(path))
            .map(|(annotation, book_title, path)| NoteEntry {
                annotation,
                book_title,
                file_type: libreri_core::FileType::from_path(std::path::Path::new(&path))
                    .unwrap_or(libreri_core::FileType::Pdf),
            })
            .collect())
    }

    /// Every Markdown file in the signed-in profile's notes folder, newest
    /// first, with the book each is linked to.
    pub fn notebooks(&self) -> Result<Vec<NotebookEntry>> {
        let profile = self.profile()?;
        let Some(name) = self.with_db(|db| db.profile_name(&profile))? else {
            return Ok(Vec::new());
        };
        let dir = self
            .layout()
            .notes_dir()
            .join(crate::reading::notes_folder_name(&name));
        let mut out = Vec::new();
        for entry in walkdir::WalkDir::new(&dir)
            .max_depth(3)
            .into_iter()
            .flatten()
        {
            let path = entry.path();
            if !entry.file_type().is_file() || path.extension().is_none_or(|e| e != "md") {
                continue;
            }
            let Ok(content) = fs::read_to_string(path) else {
                continue;
            };
            let Some(rel) = crate::paths::rel_of(self.layout(), path) else {
                continue;
            };
            let linked = crate::reading::linked_book(&content)
                .and_then(|id| self.with_db(|db| db.resolve_book_id(&id)).ok().flatten());
            if let Some(id) = &linked {
                // A notebook for a book this profile cannot see stays hidden.
                match self.record(id) {
                    Ok(b) if self.may_open(&b.rel_path) => {}
                    _ => continue,
                }
            }
            let b = body(&content);
            out.push(NotebookEntry {
                book_id: linked,
                title: path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                rel_path: rel,
                modified: fs::metadata(path).map_or(0, |m| crate::paths::mtime_secs(&m)),
                excerpt: excerpt(b),
                words: b.split_whitespace().count() as u32,
            });
        }
        out.sort_by_key(|n| std::cmp::Reverse(n.modified));
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::*;
    use crate::*;
    use libreri_core::{Annotation, AnnotationKind, BookQuery, TextQuote};

    #[test]
    fn excerpts_skip_front_matter_and_title() {
        let text = "---\nbook: x\n---\n\n# Title\n\n> A quote\n\nMy **thought**.\n";
        assert_eq!(excerpt(body(text)), "A quote My **thought**.");
        assert_eq!(
            excerpt("> Every map\n>\n> — [p. 4](libreri://book/x#annotation=1)\n"),
            "Every map p. 4"
        );
        assert_eq!(strip_links("a [b] c [d](e"), "a [b] c [d](e");
    }

    #[test]
    fn lists_notes_and_notebooks_of_the_signed_in_profile() {
        let (_d, lib) = library();
        md_book(&lib.layout().books_dir(), "a.md", "Alpha");
        lib.scan(&NoProgress).unwrap();
        let book = lib.books(&BookQuery::default()).unwrap().remove(0);
        lib.save_annotation(Annotation {
            id: "0b7f5a3e-1f7e-4d4c-9d34-5d0a8d8f2c10".into(),
            book_id: book.id.clone(),
            kind: AnnotationKind::Highlight,
            color: None,
            locator: r#"{"type":"text","start":1,"end":4}"#.into(),
            quote: Some(TextQuote {
                exact: "Body".into(),
                ..Default::default()
            }),
            note: None,
            label: None,
            position: 0.1,
            created_at: String::new(),
            modified_at: String::new(),
        })
        .unwrap();
        let nb = lib.notebook(&book.id).unwrap();
        lib.save_notebook(&book.id, &format!("{}Some thoughts here\n", nb.content))
            .unwrap();
        std::fs::write(
            lib.layout()
                .root()
                .join(&nb.rel_path)
                .with_file_name("Loose.md"),
            "# Loose\nIdeas",
        )
        .unwrap();

        let notes = lib.all_notes().unwrap();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].book_title, "Alpha");
        let books: Vec<_> = lib.notebooks().unwrap();
        assert_eq!(books.len(), 2);
        let linked = books.iter().find(|n| n.book_id.is_some()).unwrap();
        assert_eq!(linked.excerpt, "Some thoughts here");
        assert_eq!(linked.words, 3);
        assert!(books
            .iter()
            .any(|n| n.title == "Loose" && n.book_id.is_none()));
        let loose = books.iter().find(|n| n.title == "Loose").unwrap();
        assert_eq!(lib.read_note(&loose.rel_path).unwrap(), "# Loose\nIdeas");
        lib.write_note(&loose.rel_path, "# Loose\nMore").unwrap();
        let fresh = lib.create_note("Reading plan").unwrap();
        assert!(fresh.ends_with("/Reading plan.md"), "{fresh}");
        assert!(lib.read_note("Books/a.md").is_err());
        assert!(lib.write_note(".library-data/x.md", "no").is_err());

        // Someone else sees none of it.
        let jane = lib
            .create_profile(
                "Jane Smith",
                "teal",
                libreri_core::ProfileKind::Standard,
                None,
            )
            .unwrap();
        lib.sign_in(&jane.id, None).unwrap();
        assert!(lib.all_notes().unwrap().is_empty());
        assert!(lib.notebooks().unwrap().is_empty());
        assert!(lib.read_note(&loose.rel_path).is_err(), "not Jane's");
    }
}
