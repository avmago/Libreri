//! Reading: positions, highlights and bookmarks, notebooks and the open tabs.
//!
//! Annotations live in the database and are also written, one JSON file per
//! book and profile, to `.library-data/annotations/<profile>/<book>.json`.
//! Those files bring them back when the database is rebuilt or a trashed book
//! is restored. Notebooks are ordinary Markdown files in `Notes/<profile>/`.

use crate::paths::{self, unique_path, write_atomic};
use crate::{now, Error, Library, Result};
use libreri_core::annotation::book_link;
use libreri_core::{Annotation, BookId, BookUserState, ProfileId, ReadingStatus};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// A profile's Markdown notebook for one book.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notebook {
    /// Library-relative path, e.g. "Notes/Jane Smith/Linear Algebra.md".
    pub rel_path: String,
    pub content: String,
}

fn annotations_file(lib: &Library, profile: &ProfileId, book: &BookId) -> PathBuf {
    lib.layout()
        .data_dir()
        .join("annotations")
        .join(profile.to_string())
        .join(format!("{book}.json"))
}

/// Characters allowed in a file name on every OS; the rest become spaces.
fn safe_file_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|') || c.is_control() {
                ' '
            } else {
                c
            }
        })
        .collect();
    let cleaned = cleaned
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches('.')
        .chars()
        .take(120)
        .collect::<String>();
    // Cut by bytes too: 120 CJK characters are 360 bytes, over the usual
    // 255-byte limit on file names.
    let cleaned = libreri_export::truncate_bytes(&cleaned, libreri_export::MAX_NAME_BYTES)
        .trim_end()
        .trim_matches('.')
        .to_owned();
    if paths::validate_name(&cleaned).is_ok() {
        cleaned
    } else {
        "Untitled".to_owned()
    }
}

/// The book a notebook belongs to, from its front matter line
/// `book: libreri://book/<id>`.
pub(crate) fn linked_book(content: &str) -> Option<BookId> {
    notebook_book_id(content)
}

fn notebook_book_id(content: &str) -> Option<BookId> {
    let front = content.strip_prefix("---")?;
    let end = front.find("\n---")?;
    front[..end].lines().find_map(|l| {
        let v = l.trim().strip_prefix("book:")?.trim();
        let id = v.strip_prefix("libreri://book/")?;
        BookId::from_hex(id.split('#').next()?.trim()).ok()
    })
}

/// One profile's data about one book, as backed up in
/// `.library-data/annotations/<profile>/<book>.json`. Version 1 files hold
/// only the list of annotations.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct PersonalBackup {
    pub format_version: u32,
    #[serde(default)]
    pub state: Option<BookUserState>,
    #[serde(default)]
    pub position: Option<String>,
    #[serde(default)]
    pub annotations: Vec<Annotation>,
}

pub(crate) const PERSONAL_BACKUP_VERSION: u32 = 2;

pub(crate) fn read_backup(text: &str) -> Option<PersonalBackup> {
    if let Ok(list) = serde_json::from_str::<Vec<Annotation>>(text) {
        return Some(PersonalBackup {
            format_version: 1,
            annotations: list,
            ..Default::default()
        });
    }
    let b: PersonalBackup = serde_json::from_str(text).ok()?;
    (b.format_version <= PERSONAL_BACKUP_VERSION).then_some(b)
}

/// The folder name under `Notes/` for a profile called `name`.
pub(crate) fn notes_folder_name(name: &str) -> String {
    safe_file_name(name)
}

/// Two profiles whose notes folders would be the same folder on a
/// case-insensitive disk (macOS, Windows): "Sam" and "Sam.", "Élise" and
/// "élise", or two names that both become "Untitled".
pub(crate) fn same_notes_folder(a: &str, b: &str) -> bool {
    notes_folder_name(a).to_lowercase() == notes_folder_name(b).to_lowercase()
}

impl Library {
    /// The last saved place in a book for the current profile (JSON).
    pub fn position(&self, book: &BookId) -> Result<Option<String>> {
        let profile = self.profile()?;
        let book = self.book(book)?;
        self.with_db(|db| db.position(&book.id, &profile))
    }

    /// Saves the reading position. Opening a book without a status marks it
    /// as being read.
    pub fn save_position(&self, book: &BookId, locator: &str, progress: f32) -> Result<()> {
        serde_json::from_str::<serde_json::Value>(locator)
            .map_err(|_| Error::InvalidInput("the reading position is not valid".into()))?;
        let profile = self.profile()?;
        let current = self.book(book)?;
        self.with_db(|db| db.set_position(&current.id, &profile, locator, progress, &now()))?;
        if current.user.status == ReadingStatus::None {
            let mut state = self.book(book)?.user;
            state.status = ReadingStatus::Reading;
            self.with_db(|db| db.set_user_state(&current.id, &profile, &state))?;
        }
        self.backup_personal(&current.id)
    }

    pub fn annotations(&self, book: &BookId) -> Result<Vec<Annotation>> {
        let profile = self.profile()?;
        let book = self.book(book)?;
        self.with_db(|db| db.annotations(&book.id, &profile))
    }

    /// Adds or updates a highlight or bookmark and refreshes its backup.
    pub fn save_annotation(&self, annotation: Annotation) -> Result<Annotation> {
        let mut a = annotation
            .validated()
            .map_err(|e| Error::InvalidInput(e.to_string()))?;
        let book = self.book(&a.book_id)?;
        a.book_id = book.id.clone();
        let profile = self.profile()?;
        let now = now();
        let existing = self.with_db(|db| db.annotation(&a.id))?;
        if let Some((old, owner)) = &existing {
            if owner != &profile {
                return Err(Error::InvalidInput(
                    "that note belongs to someone else".into(),
                ));
            }
            a.created_at = old.created_at.clone();
        } else {
            a.created_at = now.clone();
        }
        a.modified_at = now;
        self.with_db(|db| db.save_annotation(&a, &profile))?;
        self.backup_personal(&book.id)?;
        Ok(a)
    }

    pub fn delete_annotation(&self, id: &str) -> Result<()> {
        let profile = self.profile()?;
        let Some((a, owner)) = self.with_db(|db| db.annotation(id))? else {
            return Ok(());
        };
        if owner != profile {
            return Err(Error::InvalidInput(
                "that note belongs to someone else".into(),
            ));
        }
        self.with_db(|db| db.delete_annotation(id))?;
        // Recordings and captured pages stay in the notes folder: deleting
        // can be undone, and they are the person's own files.
        self.backup_personal(&a.book_id)
    }

    /// Writes the signed-in profile's backup for one book: status, rating,
    /// position and annotations. Guests leave no backups.
    pub(crate) fn backup_personal(&self, book: &BookId) -> Result<()> {
        let Some(session) = self.session_info() else {
            return Ok(());
        };
        if !session.kind.keeps_data() {
            return Ok(());
        }
        self.backup_personal_of(&session.id, book)
    }

    /// The personal data of `profile` for one book, as stored in its backup.
    /// `None` when there is nothing to keep.
    pub(crate) fn personal_backup(
        &self,
        profile: &ProfileId,
        book: &BookId,
    ) -> Result<Option<PersonalBackup>> {
        let profile = *profile;
        let (state, position, annotations) = self.with_db(|db| {
            let state = db.book(book, &profile)?.map(|b| b.user);
            Ok((
                state,
                db.position(book, &profile)?,
                db.annotations(book, &profile)?,
            ))
        })?;
        let state = state.filter(|s| s != &BookUserState::default());
        if state.is_none() && position.is_none() && annotations.is_empty() {
            return Ok(None);
        }
        Ok(Some(PersonalBackup {
            format_version: PERSONAL_BACKUP_VERSION,
            state,
            position,
            annotations,
        }))
    }

    /// Writes (or removes) the backup file of `profile` for one book,
    /// whoever is signed in. Callers check that the profile keeps data.
    pub(crate) fn backup_personal_of(&self, profile: &ProfileId, book: &BookId) -> Result<()> {
        let path = annotations_file(self, profile, book);
        let Some(backup) = self.personal_backup(profile, book)? else {
            let _ = fs::remove_file(&path);
            return Ok(());
        };
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let json = serde_json::to_vec_pretty(&backup).map_err(std::io::Error::other)?;
        write_atomic(&path, &json)?;
        Ok(())
    }

    /// Loads every profile's backup for a book that was just added (a
    /// rebuilt database, or a file restored from the trash).
    pub(crate) fn restore_annotations(&self, book: &BookId) -> Result<usize> {
        let dir = self.layout().data_dir().join("annotations");
        let Ok(profiles) = fs::read_dir(&dir) else {
            return Ok(0);
        };
        let mut restored = 0;
        for entry in profiles.flatten() {
            let Ok(profile) = entry.file_name().to_string_lossy().parse::<ProfileId>() else {
                continue;
            };
            let file = entry.path().join(format!("{book}.json"));
            let Ok(text) = fs::read_to_string(&file) else {
                continue;
            };
            let Some(backup) = read_backup(&text) else {
                continue;
            };
            let known = self.with_db(|db| db.profile_name(&profile))?.is_some();
            if !known {
                self.with_db(|db| db.insert_profile(&profile, "Restored profile", &now()))?;
            }
            if let Some(state) = &backup.state {
                self.with_db(|db| db.set_user_state(book, &profile, state))?;
            }
            if let Some(position) = &backup.position {
                let progress = backup.state.as_ref().map_or(0.0, |s| s.progress);
                let opened = backup
                    .state
                    .as_ref()
                    .and_then(|s| s.last_opened.clone())
                    .unwrap_or_else(now);
                self.with_db(|db| db.set_position(book, &profile, position, progress, &opened))?;
            }
            for mut a in backup.annotations {
                a.book_id = book.clone();
                self.with_db(|db| db.save_annotation(&a, &profile))?;
                restored += 1;
            }
        }
        Ok(restored)
    }

    /// Renames annotation backups when a book's id changes.
    pub(crate) fn rename_annotation_backups(&self, old: &BookId, new: &BookId) {
        let dir = self.layout().data_dir().join("annotations");
        if let Ok(profiles) = fs::read_dir(dir) {
            for entry in profiles.flatten() {
                let from = entry.path().join(format!("{old}.json"));
                if from.is_file() {
                    let _ = fs::rename(&from, entry.path().join(format!("{new}.json")));
                }
            }
        }
    }

    fn notes_dir_for(&self, profile: &ProfileId) -> Result<PathBuf> {
        if !self.session_info().is_some_and(|s| s.kind.keeps_data()) {
            return Err(Error::NotAllowed(
                "guests cannot keep a notebook; sign in to your own profile".into(),
            ));
        }
        let name = self
            .with_db(|db| db.profile_name(profile))?
            .unwrap_or_else(|| "Me".to_owned());
        let dir = self.layout().notes_dir().join(safe_file_name(&name));
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// The current profile's notebook for a book, created on first use.
    pub fn notebook(&self, book: &BookId) -> Result<Notebook> {
        let profile = self.profile()?;
        let book = self.book(book)?;
        if let Some(rel) = self.with_db(|db| db.notebook_path(&book.id, &profile))? {
            if let Some(path) = self.layout().resolve_relative(&rel) {
                if let Ok(content) = fs::read_to_string(&path) {
                    return Ok(Notebook {
                        rel_path: rel,
                        content,
                    });
                }
            }
        }
        let dir = self.notes_dir_for(&profile)?;

        // After a rebuild the database forgets notebooks: find it by its link.
        if let Ok(entries) = fs::read_dir(&dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.extension().is_some_and(|x| x == "md") {
                    if let Ok(content) = fs::read_to_string(&p) {
                        let matches = notebook_book_id(&content)
                            .and_then(|id| {
                                self.with_db(|db| db.resolve_book_id(&id)).ok().flatten()
                            })
                            .is_some_and(|id| id == book.id);
                        if matches {
                            let rel =
                                paths::rel_of(self.layout(), &p).ok_or(Error::BookNotFound)?;
                            self.with_db(|db| db.set_notebook_path(&book.id, &profile, &rel))?;
                            return Ok(Notebook {
                                rel_path: rel,
                                content,
                            });
                        }
                    }
                }
            }
        }

        let title = &book.metadata.title;
        let path = unique_path(&dir, &format!("{}.md", safe_file_name(title)));
        let content = format!(
            "---\nbook: {}\ntitle: \"{}\"\n---\n\n# {}\n\n",
            book_link(&book.id, None),
            title.replace('"', "'"),
            title
        );
        write_atomic(&path, content.as_bytes())?;
        let rel = paths::rel_of(self.layout(), &path).ok_or(Error::BookNotFound)?;
        self.with_db(|db| db.set_notebook_path(&book.id, &profile, &rel))?;
        Ok(Notebook {
            rel_path: rel,
            content,
        })
    }

    /// Saves the notebook text.
    pub fn save_notebook(&self, book: &BookId, content: &str) -> Result<Notebook> {
        let current = self.notebook(book)?;
        let path = self
            .layout()
            .resolve_relative(&current.rel_path)
            .ok_or(Error::BookNotFound)?;
        write_atomic(&path, content.as_bytes())?;
        Ok(Notebook {
            rel_path: current.rel_path,
            content: content.to_owned(),
        })
    }

    /// The tabs that were open, as saved by the interface (JSON).
    pub fn session(&self) -> Result<Option<String>> {
        let key = format!("session:{}", self.profile()?);
        self.with_db(|db| db.meta_get(&key))
    }

    pub fn save_session(&self, json: &str) -> Result<()> {
        serde_json::from_str::<serde_json::Value>(json)
            .map_err(|_| Error::InvalidInput("the session is not valid JSON".into()))?;
        let key = format!("session:{}", self.profile()?);
        self.with_db(|db| db.meta_set(&key, json))
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use crate::*;
    use libreri_core::{Annotation, AnnotationKind, BookQuery, ReadingStatus, TextQuote};

    fn book(lib: &Library) -> libreri_core::Book {
        md_book(&lib.layout().books_dir(), "a.md", "Linear: Algebra?");
        lib.scan(&NoProgress).unwrap();
        lib.books(&BookQuery::default()).unwrap().remove(0)
    }

    fn highlight(book: &libreri_core::BookId, id: &str) -> Annotation {
        Annotation {
            id: id.into(),
            book_id: book.clone(),
            kind: AnnotationKind::Highlight,
            color: None,
            locator: r#"{"type":"text","start":10,"end":15}"#.into(),
            quote: Some(TextQuote {
                exact: "Body".into(),
                ..Default::default()
            }),
            note: Some("Remember this".into()),
            label: None,
            position: 0.2,
            created_at: String::new(),
            modified_at: String::new(),
        }
    }

    const ID: &str = "0b7f5a3e-1f7e-4d4c-9d34-5d0a8d8f2c10";

    #[test]
    fn positions_mark_books_as_reading() {
        let (_d, lib) = library();
        let b = book(&lib);
        lib.save_position(&b.id, r#"{"type":"text","offset":3}"#, 0.4)
            .unwrap();
        assert_eq!(
            lib.position(&b.id).unwrap().as_deref(),
            Some(r#"{"type":"text","offset":3}"#)
        );
        assert_eq!(lib.book(&b.id).unwrap().user.status, ReadingStatus::Reading);
        assert!(lib.save_position(&b.id, "nope", 0.1).is_err());
    }

    #[test]
    fn annotations_survive_a_rebuild() {
        let (_d, lib) = library();
        let b = book(&lib);
        let saved = lib.save_annotation(highlight(&b.id, ID)).unwrap();
        assert!(!saved.created_at.is_empty());
        assert_eq!(lib.annotations(&b.id).unwrap().len(), 1);

        lib.rebuild_index(&NoProgress).unwrap();
        let back = lib.annotations(&b.id).unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].note.as_deref(), Some("Remember this"));

        lib.delete_annotation(ID).unwrap();
        assert!(lib.annotations(&b.id).unwrap().is_empty());
        lib.rebuild_index(&NoProgress).unwrap();
        assert!(
            lib.annotations(&b.id).unwrap().is_empty(),
            "backup removed too"
        );
    }

    #[test]
    fn notebooks_are_markdown_files_found_again_after_a_rebuild() {
        let (_d, lib) = library();
        let b = book(&lib);
        let nb = lib.notebook(&b.id).unwrap();
        assert!(nb.rel_path.starts_with("Notes/"));
        assert!(
            nb.rel_path.ends_with("Linear Algebra.md"),
            "{}",
            nb.rel_path
        );
        assert!(nb
            .content
            .contains(&format!("book: libreri://book/{}", b.id)));

        lib.save_notebook(&b.id, &format!("{}\nMy notes\n", nb.content))
            .unwrap();
        lib.rebuild_index(&NoProgress).unwrap();
        let again = lib.notebook(&b.id).unwrap();
        assert_eq!(again.rel_path, nb.rel_path);
        assert!(again.content.ends_with("My notes\n"));
    }

    #[test]
    fn sessions_round_trip() {
        let (_d, lib) = library();
        assert_eq!(lib.session().unwrap(), None);
        lib.save_session(r#"{"tabs":[]}"#).unwrap();
        assert_eq!(lib.session().unwrap().as_deref(), Some(r#"{"tabs":[]}"#));
        assert!(lib.save_session("{").is_err());
    }
}
