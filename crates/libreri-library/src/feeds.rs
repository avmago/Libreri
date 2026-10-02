//! Feeds (ADR 0027): each profile's subscriptions and downloads live in
//! `Feeds/<profile>/`, apart from `Books/` and `Notes/`. A download joins
//! the library when it is added to a folder of `Books/`.

use crate::import::{ImportMode, ImportRequest};
use crate::{Error, Library, Result};
use libreri_core::{BookId, BookMetadata};
use std::fs;
use std::path::PathBuf;

/// The subscriptions file, hidden in the profile's feeds folder.
const STATE_FILE: &str = ".feeds.json";

/// Only Libreri's own state files: hidden, `.json`, a plain name.
fn state_name(name: &str) -> Result<&str> {
    let ok = name.starts_with('.')
        && name.ends_with(".json")
        && name.len() < 64
        && name[1..]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-');
    if ok {
        Ok(name)
    } else {
        Err(Error::InvalidInput("not a feeds state file".into()))
    }
}

impl Library {
    /// The signed-in profile's feeds folder (guests have none).
    pub(crate) fn own_feeds_dir(&self) -> Result<PathBuf> {
        let session = self.session_info().ok_or(Error::SignedOut)?;
        if !session.kind.keeps_data() {
            return Err(Error::NotAllowed(
                "guests cannot follow feeds; sign in to your own profile".into(),
            ));
        }
        let name = self
            .with_db(|db| db.profile_name(&session.id))?
            .unwrap_or_else(|| "Me".into());
        Ok(self
            .layout()
            .feeds_dir()
            .join(crate::reading::notes_folder_name(&name)))
    }

    /// The feeds folder, created if needed.
    pub fn feeds_folder(&self) -> Result<PathBuf> {
        let dir = self.own_feeds_dir()?;
        fs::create_dir_all(&dir)?;
        Ok(dir)
    }

    /// The profile's subscriptions (JSON), or None before the first one.
    pub fn read_feeds(&self) -> Result<Option<String>> {
        self.read_feeds_file(STATE_FILE)
    }

    /// A state file in the profile's feeds folder (`.feeds.json`,
    /// `.podcasts.json`), or None before the first save.
    pub fn read_feeds_file(&self, name: &str) -> Result<Option<String>> {
        let path = self.own_feeds_dir()?.join(state_name(name)?);
        match fs::read_to_string(&path) {
            Ok(s) => Ok(Some(s)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Saves the subscriptions (written aside first, so a crash never
    /// leaves half a file).
    pub fn write_feeds(&self, json: &str) -> Result<()> {
        self.write_feeds_file(STATE_FILE, json)
    }

    pub fn write_feeds_file(&self, name: &str, json: &str) -> Result<()> {
        let name = state_name(name)?;
        let dir = self.feeds_folder()?;
        // Written aside (a name of its own) and synced, then swapped in.
        crate::paths::write_atomic(&dir.join(name), json.as_bytes())?;
        Ok(())
    }

    /// A new file for a download: `Feeds/<profile>/<folders…>/<title>.<ext>`
    /// (a free name). Returns its full path and its path in the library.
    pub fn new_feed_file(
        &self,
        folders: &[String],
        title: &str,
        ext: &str,
    ) -> Result<(PathBuf, String)> {
        let mut dir = self.feeds_folder()?;
        for f in folders {
            let name = crate::reading::safe_file_name(f);
            if !name.is_empty() && name != "." && name != ".." {
                dir.push(name);
            }
        }
        fs::create_dir_all(&dir)?;
        let mut stem = crate::reading::safe_file_name(title);
        if stem.is_empty() {
            stem = "Untitled".into();
        }
        if stem.chars().count() > 120 {
            stem = stem.chars().take(120).collect::<String>().trim().to_owned();
        }
        let path = crate::paths::unique_path(&dir, &format!("{stem}.{ext}"));
        let rel = crate::paths::rel_of(self.layout(), &path)
            .ok_or_else(|| Error::InvalidInput("the download could not be saved".into()))?;
        Ok((path, rel))
    }

    /// Saves a download in `Feeds/<profile>/<folders…>/<title>.<ext>`.
    /// Returns its path in the library.
    pub fn save_feed_file(
        &self,
        folders: &[String],
        title: &str,
        ext: &str,
        bytes: &[u8],
    ) -> Result<String> {
        let (path, rel) = self.new_feed_file(folders, title, ext)?;
        crate::paths::write_atomic(&path, bytes)?;
        Ok(rel)
    }

    /// A file in the signed-in profile's own feeds folder.
    pub fn own_feed_file(&self, rel: &str) -> Result<PathBuf> {
        let dir = self.own_feeds_dir()?;
        if rel.split('/').any(|p| p == "..") {
            return Err(Error::NotAllowed(
                "that file belongs to someone else".into(),
            ));
        }
        self.layout()
            .resolve_relative(rel)
            .filter(|p| p.starts_with(&dir))
            .ok_or_else(|| Error::NotAllowed("that file belongs to someone else".into()))
    }

    pub(crate) fn is_own_feed_file(&self, rel: &str) -> bool {
        self.own_feed_file(rel).is_ok()
    }

    /// Deletes a download (it can be downloaded again).
    pub fn delete_feed_file(&self, rel: &str) -> Result<()> {
        let path = self.own_feed_file(rel)?;
        match fs::remove_file(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        // Folders left empty go too (not the profile's own folder).
        let top = self.own_feeds_dir()?;
        let mut cur = path.parent().map(PathBuf::from);
        while let Some(d) = cur {
            if d == top || !d.starts_with(&top) || fs::remove_dir(&d).is_err() {
                break;
            }
            cur = d.parent().map(PathBuf::from);
        }
        Ok(())
    }

    /// Moves a download into `folder` (relative to `Books/`) as a book,
    /// with `details` from the feed filling what the file does not say.
    /// A file already in the library is not added twice: that book is
    /// returned, and the download removed.
    pub fn add_feed_file_to_library(
        &self,
        rel: &str,
        folder: &str,
        details: &BookMetadata,
    ) -> Result<BookId> {
        self.require_edit()?;
        let path = self.own_feed_file(rel)?;
        if !path.is_file() {
            return Err(Error::InvalidInput(
                "the download is no longer there".into(),
            ));
        }
        let report = self.import(
            &ImportRequest {
                sources: vec![path.clone()],
                folder: folder.to_owned(),
                mode: ImportMode::Move,
            },
            &crate::NoProgress,
        )?;
        if let Some(d) = report.duplicates.first() {
            let _ = self.delete_feed_file(rel);
            return Ok(d.existing_id.clone());
        }
        let Some(id) = report.added_ids.first().cloned() else {
            let why = report
                .failed
                .first()
                .map(|(_, e)| e.clone())
                .unwrap_or_else(|| "Libreri cannot read this file".into());
            return Err(Error::InvalidInput(why));
        };
        // The feed knows the title, authors and abstract better than most
        // PDFs' own details; keep what the file says where the feed is silent.
        let book = self.book(&id)?;
        let mut m = book.metadata.clone();
        let d = details;
        if !d.title.trim().is_empty() {
            m.title = d.title.clone();
        }
        if !d.authors.is_empty() {
            m.authors = d.authors.clone();
        }
        m.about = d.about.clone().or(m.about);
        m.year = d.year.or(m.year);
        m.publisher = d.publisher.clone().or(m.publisher);
        m.doi = d.doi.clone().or(m.doi);
        m.arxiv_id = d.arxiv_id.clone().or(m.arxiv_id);
        m.url = d.url.clone().or(m.url);
        m.journal = d.journal.clone().or(m.journal);
        for c in &d.categories {
            if !m.categories.iter().any(|x| x.eq_ignore_ascii_case(c)) {
                m.categories.push(c.clone());
            }
        }
        for t in &d.tags {
            if !m.tags.iter().any(|x| x.eq_ignore_ascii_case(t)) {
                m.tags.push(t.clone());
            }
        }
        m.content_type = d.content_type;
        match self.update_metadata(&id, m) {
            Ok(_) => {}
            // Details the library would refuse are left as the file had them.
            Err(Error::InvalidInput(_)) => {}
            Err(e) => return Err(e),
        }
        Ok(id)
    }

    /// `Feeds/<old name>` follows a renamed profile.
    pub(crate) fn rename_feeds_folder(&self, old_name: &str, new_name: &str) -> Result<()> {
        let from = self
            .layout()
            .feeds_dir()
            .join(crate::reading::notes_folder_name(old_name));
        let to = self
            .layout()
            .feeds_dir()
            .join(crate::reading::notes_folder_name(new_name));
        if from == to || !from.is_dir() {
            return Ok(());
        }
        if to.exists() {
            return Err(Error::NameTaken(format!(
                "Feeds/{}",
                crate::reading::notes_folder_name(new_name)
            )));
        }
        fs::rename(&from, &to)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::testutil::*;
    use libreri_core::{BookMetadata, ContentType};

    #[test]
    fn downloads_live_apart_and_join_the_library() {
        let (_d, lib) = library();
        assert_eq!(lib.read_feeds().unwrap(), None);
        lib.write_feeds("{\"version\":1}").unwrap();
        assert_eq!(
            lib.read_feeds().unwrap().as_deref(),
            Some("{\"version\":1}")
        );
        let md = b"---\ntitle: \"Keeping the light\"\n---\n\n# Keeping the light\n\nText.\n";
        let rel = lib
            .save_feed_file(
                &["Science".into(), "Sea/Ships".into()],
                "Keeping: the light",
                "md",
                md,
            )
            .unwrap();
        assert!(rel.starts_with("Feeds/"), "{rel}");
        assert!(
            rel.ends_with("/Science/Sea Ships/Keeping the light.md"),
            "{rel}"
        );
        assert!(lib.may_open(&rel));
        assert!(lib.own_feed_file("Feeds/Someone/x.md").is_err());
        assert!(lib
            .own_feed_file(&format!("{rel}/../../../../Books/x"))
            .is_err());

        let details = BookMetadata {
            title: "Keeping the Light".into(),
            authors: vec!["Jane Smith".into()],
            about: Some("About keepers.".into()),
            tags: vec!["sea".into()],
            categories: vec!["Science/Oceans".into()],
            journal: Some("Sea Letters".into()),
            year: Some(2026),
            doi: Some("10.1000/sea.1".into()),
            url: Some("https://example.org/keeping".into()),
            content_type: ContentType::Article,
            ..Default::default()
        };
        let id = lib.add_feed_file_to_library(&rel, "", &details).unwrap();
        let book = lib.book(&id).unwrap();
        assert_eq!(book.metadata.title, "Keeping the Light");
        assert_eq!(book.metadata.authors, ["Jane Smith"]);
        assert_eq!(book.metadata.content_type, ContentType::Article);
        assert_eq!(book.metadata.about.as_deref(), Some("About keepers."));
        assert!(book.metadata.tags.iter().any(|t| t == "sea"));
        assert_eq!(book.metadata.categories, ["Science/Oceans"]);
        assert_eq!(book.metadata.journal.as_deref(), Some("Sea Letters"));
        assert_eq!(book.metadata.year, Some(2026));
        assert_eq!(book.metadata.doi.as_deref(), Some("10.1000/sea.1"));
        assert_eq!(
            book.metadata.url.as_deref(),
            Some("https://example.org/keeping")
        );
        assert!(book.rel_path.starts_with("Books/"));
        assert!(lib.own_feed_file(&rel).map(|p| !p.exists()).unwrap());
    }
}
