//! Libreri archives: writing them (Export, backups) and importing them into
//! this library, re-linking every note to its book
//! (docs/data-portability.md).
//!
//! Importing matches each book of the archive by content hash (or an old
//! id), then, when the archive has no file for it, by ISBN, DOI, arXiv id
//! or title and author ("another copy": its notes find their place by the
//! quoted text). Books found nowhere are added as *file missing* records,
//! so their notes are kept and reconnect when the file is added. Nothing is
//! duplicated: notes are matched by id and the newer version wins.

use crate::export::ExportReport;
use crate::paths::{self, unique_path, write_atomic};
use crate::profiles::CollectionBackup;
use crate::reading::{read_backup, PersonalBackup};
use crate::sidecar::{self, Sidecar};
use crate::{covers, now, Error, Library, Progress, Result};
use libreri_core::{
    Alias, AliasKind, Book, BookId, BookMetadata, BookUserState, Profile, ProfileId, ProfileKind,
};
use libreri_db::CollectionRecord;
use libreri_export::archive::{
    ArchiveBook, ArchiveError, ArchiveKind, ArchiveLibrary, ArchiveNote, ArchiveNotebook,
    ArchiveProfile, ArchiveReader, ArchiveWriter, Manifest, DATABASE,
};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

/// What to put in an archive.
#[derive(Debug, Clone)]
pub struct ArchiveOptions {
    /// `None` = the whole library (also adds every note file and a copy of
    /// the catalogue).
    pub books: Option<Vec<BookId>>,
    /// Whose personal data; `None` = everyone who keeps data.
    pub profiles: Option<Vec<ProfileId>>,
    /// Personal data: status, positions, highlights, notebooks, collections.
    pub notes: bool,
    pub book_files: bool,
    /// Backups keep PIN hashes (a restore brings them back); exports never do.
    pub kind: ArchiveKind,
    pub app_version: String,
}

/// Where the notes of one archive profile go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProfileTarget {
    Existing(ProfileId),
    New,
    Skip,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileChoice {
    pub archive: ArchiveProfile,
    pub suggestion: ProfileTarget,
}

/// What an archive holds and what importing it would do, before importing.
#[derive(Debug, Clone, PartialEq)]
pub struct ArchiveSummary {
    pub kind: ArchiveKind,
    pub created_at: String,
    pub created_by: String,
    pub library_name: String,
    pub includes_book_files: bool,
    pub includes_pins: bool,
    pub books: u32,
    pub notes: u32,
    pub notebooks: u32,
    /// Already in this library (same file).
    pub linked: u32,
    /// Added with their files from the archive.
    pub with_file: u32,
    /// Another copy or edition of the book is here.
    pub other_file: u32,
    /// Not here and no file in the archive: kept as "file missing".
    pub missing: u32,
    pub profiles: Vec<ProfileChoice>,
}

/// How to import.
#[derive(Debug, Clone, Default)]
pub struct ArchiveImport {
    /// Archive profile → target. Profiles not listed use the suggestion.
    pub profiles: Vec<(ProfileId, ProfileTarget)>,
    /// Restoring into a new library: the archive's owner replaces this
    /// library's owner name, colour and preferences (and PIN, from backups).
    pub adopt_owner: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArchiveImportReport {
    pub linked: u32,
    pub added: u32,
    pub other_file: u32,
    pub missing: u32,
    /// Files put back for books whose file was missing here.
    pub files_restored: u32,
    pub details_updated: u32,
    pub notes_added: u32,
    pub notes_updated: u32,
    /// Notes where this library had the newer version.
    pub notes_kept: u32,
    pub note_files_added: u32,
    /// Note files that differed from one here; imported under a new name.
    pub note_conflicts: Vec<String>,
    pub profiles_created: Vec<String>,
    pub missing_books: Vec<BookId>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Match {
    Exact(BookId),
    OtherFile(BookId),
    WithFile,
    Missing,
}

fn archive_err(e: ArchiveError) -> Error {
    match e {
        ArchiveError::Io(e) => Error::Io(e),
        other => Error::InvalidInput(other.to_string()),
    }
}

fn io_err(e: std::io::Error) -> Error {
    Error::Io(e)
}

/// Compares RFC 3339 times written by Libreri (UTC, same format).
fn newer(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a > b,
        (Some(_), None) => true,
        _ => false,
    }
}

fn first_family(authors: &[String]) -> Option<String> {
    let a = authors.first()?;
    Some(
        libreri_export::split_name(a)
            .family()
            .to_lowercase()
            .trim()
            .to_owned(),
    )
}

impl Library {
    // ---------- writing ----------

    /// Writes an archive. Permission checks are the caller's.
    pub(crate) fn write_archive(
        &self,
        dest: &Path,
        opts: &ArchiveOptions,
        progress: &dyn Progress,
    ) -> Result<ExportReport> {
        let stamp = now();
        let mut manifest = Manifest::new(
            opts.kind,
            &stamp,
            &opts.app_version,
            ArchiveLibrary {
                id: self.info().id.to_string(),
                name: self.info().name.clone(),
            },
        );
        manifest.includes_book_files = opts.book_files;
        manifest.includes_pins = opts.kind == ArchiveKind::Backup;
        if let Some(dir) = dest.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut zip = ArchiveWriter::create(dest)?;
        let mut report = ExportReport {
            path: dest.to_path_buf(),
            ..Default::default()
        };

        // Who.
        let profiles: Vec<Profile> = if opts.notes {
            self.with_db(|db| db.profiles())?
                .into_iter()
                .filter(|p| p.kind.keeps_data())
                .filter(|p| {
                    opts.profiles
                        .as_ref()
                        .is_none_or(|list| list.contains(&p.id))
                })
                .collect()
        } else {
            Vec::new()
        };

        // Which books.
        let ids: Vec<BookId> = match &opts.books {
            Some(ids) => ids.clone(),
            None => self.with_db(|db| db.book_ids())?,
        };
        let chosen: HashSet<&BookId> = ids.iter().collect();
        let viewer = self.viewer();
        let total = ids.len() as u64;
        for (i, id) in ids.iter().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            let Some(book) = self.with_db(|db| db.book(id, &viewer))? else {
                continue;
            };
            progress.report(i as u64, total, &book.metadata.title);
            let aliases = self.with_db(|db| db.aliases_of(id))?;
            let sc = Sidecar {
                format_version: sidecar::SIDECAR_VERSION,
                id: book.id.clone(),
                rel_path: book.rel_path.clone(),
                file_type: book.file_type,
                file_size: book.file_size,
                added_at: book.added_at.clone(),
                modified_at: book.modified_at.clone(),
                metadata: book.metadata.clone(),
                aliases: aliases.clone(),
            };
            let json = serde_json::to_vec_pretty(&sc).map_err(std::io::Error::other)?;
            zip.add_bytes(&format!(".library-data/metadata/{id}.json"), &json)?;
            let cover = covers::cover_path(self.layout(), id);
            if book.has_cover && cover.is_file() {
                zip.add_file(&format!(".library-data/covers/{id}.jpg"), &cover)?;
            }
            let mut included = false;
            if opts.book_files && !book.missing {
                if let Some(src) = self.layout().resolve_relative(&book.rel_path) {
                    if src.is_file() && !zip.contains(&book.rel_path) {
                        zip.add_file(&book.rel_path, &src)?;
                        included = true;
                        report.files += 1;
                    }
                }
                if !included {
                    report
                        .warnings
                        .push(format!("{}: the file is missing", book.metadata.title));
                }
            }
            for p in &profiles {
                let Some(personal) = self.personal_backup(&p.id, id)? else {
                    continue;
                };
                for a in &personal.annotations {
                    manifest.notes.push(ArchiveNote {
                        id: a.id.clone(),
                        profile: p.id,
                        book: id.clone(),
                        kind: a.kind,
                        label: a.label.clone(),
                        link: a.link(),
                    });
                }
                report.notes += personal.annotations.len() as u32;
                let json = serde_json::to_vec_pretty(&personal).map_err(std::io::Error::other)?;
                zip.add_bytes(
                    &format!(".library-data/annotations/{}/{id}.json", p.id),
                    &json,
                )?;
            }
            let m = &book.metadata;
            manifest.books.push(ArchiveBook {
                id: id.clone(),
                aliases,
                rel_path: book.rel_path.clone(),
                file_type: book.file_type,
                file_size: book.file_size,
                file_included: included,
                title: m.title.clone(),
                authors: m.authors.clone(),
                isbn13: m.isbn13.clone(),
                isbn10: m.isbn10.clone(),
                doi: m.doi.clone(),
                arxiv_id: m.arxiv_id.clone(),
                pages: m.pages,
            });
            report.books += 1;
        }

        // Profiles, collections and note files.
        for p in &profiles {
            let mut copy = p.clone();
            if opts.kind == ArchiveKind::Export {
                copy.pin_hash = None;
                copy.recovery_hash = None;
            }
            let json = serde_json::to_vec_pretty(&copy).map_err(std::io::Error::other)?;
            zip.add_bytes(&format!(".library-data/profiles/{}.json", p.id), &json)?;
            let collections = self
                .profiles_dir()
                .join(format!("{}.collections.json", p.id));
            if collections.is_file() {
                zip.add_file(
                    &format!(".library-data/profiles/{}.collections.json", p.id),
                    &collections,
                )?;
            }
            let (_, files) = self.note_files(&p.id)?;
            let folder = crate::reading::notes_folder_name(&p.name);
            for f in files {
                let keep =
                    opts.books.is_none() || f.book.as_ref().is_some_and(|b| chosen.contains(b));
                if !keep {
                    continue;
                }
                let name = format!("Notes/{folder}/{}", f.inner);
                if zip.contains(&name) {
                    continue;
                }
                zip.add_file(&name, &f.abs)?;
                report.files += 1;
                if let Some(book) = f.book {
                    if f.inner.ends_with(".md") {
                        manifest.notebooks.push(ArchiveNotebook {
                            profile: p.id,
                            book,
                            path: name,
                        });
                    }
                }
            }
            manifest.profiles.push(ArchiveProfile {
                id: p.id,
                name: p.name.clone(),
                kind: p.kind,
                notes_folder: folder,
            });
        }

        // A copy of the catalogue, for reference and for other tools.
        if opts.books.is_none() {
            let tmp = dest.with_file_name(format!(
                ".{}-catalogue.tmp",
                dest.file_stem().unwrap_or_default().to_string_lossy()
            ));
            let _ = fs::remove_file(&tmp);
            let only = match &opts.profiles {
                Some(list) if list.len() == 1 => Some(list[0]),
                _ => None,
            };
            let scope = libreri_db::SnapshotScope {
                only_profile: only,
                strip_pins: opts.kind == ArchiveKind::Export,
            };
            let made = self.with_db(|db| db.snapshot(&tmp, &scope));
            if made.is_ok() {
                let added = zip.add_file(DATABASE, &tmp);
                let _ = fs::remove_file(&tmp);
                added?;
            } else {
                let _ = fs::remove_file(&tmp);
                report
                    .warnings
                    .push("the copy of the catalogue could not be made".into());
            }
        }
        progress.report(total, total, "");
        report.bytes = zip.finish(manifest)?;
        Ok(report)
    }

    /// Writes a full backup (every book, everyone's data, PINs kept). Needs
    /// no one to be signed in, so scheduled backups run in the background.
    pub fn backup_to(
        &self,
        dest: &Path,
        book_files: bool,
        app_version: &str,
        progress: &dyn Progress,
    ) -> Result<ExportReport> {
        self.write_archive(
            dest,
            &ArchiveOptions {
                books: None,
                profiles: None,
                notes: true,
                book_files,
                kind: ArchiveKind::Backup,
                app_version: app_version.to_owned(),
            },
            progress,
        )
    }

    // ---------- reading ----------

    fn match_book(&self, ab: &ArchiveBook) -> Result<Match> {
        for id in std::iter::once(&ab.id).chain(ab.aliases.iter().map(|a| &a.id)) {
            if let Some(found) = self.with_db(|db| db.resolve_book_id(id))? {
                return Ok(Match::Exact(found));
            }
        }
        if ab.file_included {
            return Ok(Match::WithFile);
        }
        let found = self.with_db(|db| {
            db.find_book_by_identifiers(
                ab.isbn13.as_deref(),
                ab.isbn10.as_deref(),
                ab.doi.as_deref(),
                ab.arxiv_id.as_deref(),
            )
        })?;
        if let Some(found) = found {
            return Ok(Match::OtherFile(found));
        }
        if !ab.title.trim().is_empty() {
            let author = first_family(&ab.authors);
            for (id, authors, pages) in self.with_db(|db| db.books_titled(&ab.title))? {
                let same_author = author.is_none() || first_family(&authors) == author;
                let similar_length = match (ab.pages, pages) {
                    (Some(a), Some(b)) => a.abs_diff(b) * 10 <= a.max(b),
                    _ => true,
                };
                if same_author && similar_length {
                    return Ok(Match::OtherFile(id));
                }
            }
        }
        Ok(Match::Missing)
    }

    fn suggest_profiles(&self, manifest: &Manifest) -> Result<Vec<ProfileChoice>> {
        let here = self.with_db(|db| db.profiles())?;
        let owner = here.iter().find(|p| p.kind == ProfileKind::Owner);
        Ok(manifest
            .profiles
            .iter()
            .map(|ap| {
                let suggestion = if let Some(p) = here.iter().find(|p| p.id == ap.id) {
                    ProfileTarget::Existing(p.id)
                } else if let Some(p) = here
                    .iter()
                    .find(|p| p.kind.keeps_data() && p.name.eq_ignore_ascii_case(&ap.name))
                {
                    ProfileTarget::Existing(p.id)
                } else if ap.kind == ProfileKind::Owner {
                    owner.map_or(ProfileTarget::New, |o| ProfileTarget::Existing(o.id))
                } else {
                    ProfileTarget::New
                };
                ProfileChoice {
                    archive: ap.clone(),
                    suggestion,
                }
            })
            .collect())
    }

    /// What an archive holds and what importing it would do. Owner only.
    pub fn inspect_archive(&self, path: &Path) -> Result<ArchiveSummary> {
        self.require_owner()?;
        let reader = ArchiveReader::open(path).map_err(archive_err)?;
        let m = &reader.manifest;
        let mut s = ArchiveSummary {
            kind: m.kind,
            created_at: m.created_at.clone(),
            created_by: m.created_by.clone(),
            library_name: m.library.name.clone(),
            includes_book_files: m.includes_book_files,
            includes_pins: m.includes_pins,
            books: m.books.len() as u32,
            notes: m.notes.len() as u32,
            notebooks: m.notebooks.len() as u32,
            linked: 0,
            with_file: 0,
            other_file: 0,
            missing: 0,
            profiles: self.suggest_profiles(m)?,
        };
        for ab in &m.books {
            match self.match_book(ab)? {
                Match::Exact(_) => s.linked += 1,
                Match::WithFile => s.with_file += 1,
                Match::OtherFile(_) => s.other_file += 1,
                Match::Missing => s.missing += 1,
            }
        }
        Ok(s)
    }

    // ---------- importing ----------

    /// Imports an archive into this library. Owner only.
    pub fn import_archive(
        &self,
        path: &Path,
        choice: &ArchiveImport,
        progress: &dyn Progress,
    ) -> Result<ArchiveImportReport> {
        self.require_owner()?;
        let _busy = self.busy();
        let mut zip = ArchiveReader::open(path).map_err(archive_err)?;
        let manifest = zip.manifest.clone();
        let mut report = ArchiveImportReport::default();

        let targets = self.resolve_profiles(&manifest, choice, &mut zip, &mut report)?;

        let total = manifest.books.len() as u64;
        let mut placed: HashMap<BookId, BookId> = HashMap::new();
        for (i, ab) in manifest.books.iter().enumerate() {
            if progress.cancelled() {
                return Err(Error::Cancelled);
            }
            progress.report(i as u64, total, &ab.title);
            match self.import_book(ab, &mut zip, &mut report) {
                Ok(target) => {
                    placed.insert(ab.id.clone(), target);
                }
                Err(e) => report.warnings.push(format!("{}: {e}", ab.title)),
            }
        }

        // Personal data, per profile and book.
        for (archive_profile, target) in &targets {
            for (from, to) in &placed {
                let name = format!(".library-data/annotations/{archive_profile}/{from}.json");
                if !zip.has(&name) {
                    continue;
                }
                let bytes = zip.read(&name).map_err(archive_err)?;
                let Some(backup) = read_backup(&String::from_utf8_lossy(&bytes)) else {
                    report.warnings.push(format!("{name} could not be read"));
                    continue;
                };
                self.merge_personal(target, to, backup, &mut report)?;
            }
            self.merge_collections(archive_profile, target, &mut zip)?;
        }

        self.import_note_files(&manifest, &targets, &mut zip, &mut report)?;
        self.backup_all_profiles()?;
        progress.report(total, total, "");
        Ok(report)
    }

    /// Decides where each archive profile's notes go, creating profiles as
    /// asked. Returns archive profile → target (skipped ones left out).
    fn resolve_profiles(
        &self,
        manifest: &Manifest,
        choice: &ArchiveImport,
        zip: &mut ArchiveReader,
        report: &mut ArchiveImportReport,
    ) -> Result<Vec<(ProfileId, ProfileId)>> {
        let suggestions = self.suggest_profiles(manifest)?;
        let mut out = Vec::new();
        for s in suggestions {
            let ap = &s.archive;
            let target = choice
                .profiles
                .iter()
                .find(|(id, _)| id == &ap.id)
                .map(|(_, t)| t.clone())
                .unwrap_or(s.suggestion);
            let from_archive: Option<Profile> = zip
                .read(&format!(".library-data/profiles/{}.json", ap.id))
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok());
            match target {
                ProfileTarget::Skip => {}
                ProfileTarget::Existing(id) => {
                    let Some(mut p) = self.with_db(|db| db.profile(&id))? else {
                        report.warnings.push(format!(
                            "the profile chosen for {} no longer exists",
                            ap.name
                        ));
                        continue;
                    };
                    if !p.kind.keeps_data() {
                        continue;
                    }
                    if choice.adopt_owner
                        && ap.kind == ProfileKind::Owner
                        && p.kind == ProfileKind::Owner
                    {
                        if let Some(src) = &from_archive {
                            if p.name != src.name {
                                self.rename_notes_folder(&p, &src.name)?;
                                p.name = src.name.clone();
                            }
                            p.colour = src.colour.clone();
                            p.prefs = src.prefs.clone();
                            if manifest.includes_pins {
                                p.pin_hash = src.pin_hash.clone();
                                p.recovery_hash = src.recovery_hash.clone();
                            }
                            self.with_db(|db| db.save_profile(&p))?;
                            self.profile_backup(&p)?;
                        }
                    }
                    out.push((ap.id, p.id));
                }
                ProfileTarget::New => {
                    if ap.kind == ProfileKind::Guest {
                        continue;
                    }
                    let here = self.with_db(|db| db.profiles())?;
                    let mut name = ap.name.clone();
                    let mut n = 2;
                    while here.iter().any(|p| p.name.eq_ignore_ascii_case(&name)) {
                        name = format!("{} ({n})", ap.name);
                        n += 1;
                    }
                    let id = if here.iter().any(|p| p.id == ap.id) {
                        ProfileId::new()
                    } else {
                        ap.id
                    };
                    let mut p = Profile::new(
                        name.clone(),
                        from_archive
                            .as_ref()
                            .map_or("graphite".to_owned(), |s| s.colour.clone()),
                        if ap.kind == ProfileKind::Owner {
                            ProfileKind::Standard
                        } else {
                            ap.kind
                        },
                        &now(),
                    );
                    p.id = id;
                    if let Some(src) = &from_archive {
                        p.prefs = src.prefs.clone();
                        p.allowed_folders = src.allowed_folders.clone();
                        if manifest.includes_pins {
                            p.pin_hash = src.pin_hash.clone();
                        }
                    }
                    self.with_db(|db| db.save_profile(&p))?;
                    self.profile_backup(&p)?;
                    report.profiles_created.push(name);
                    out.push((ap.id, p.id));
                }
            }
        }
        Ok(out)
    }

    /// Brings one book in; returns its id in this library.
    fn import_book(
        &self,
        ab: &ArchiveBook,
        zip: &mut ArchiveReader,
        report: &mut ArchiveImportReport,
    ) -> Result<BookId> {
        let sc: Option<Sidecar> = zip
            .read(&format!(".library-data/metadata/{}.json", ab.id))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .filter(|s: &Sidecar| s.id == ab.id);
        let cover = zip
            .read(&format!(".library-data/covers/{}.jpg", ab.id))
            .ok();
        match self.match_book(ab)? {
            Match::Exact(target) => {
                report.linked += 1;
                self.merge_details(&target, sc.as_ref(), ab, cover.as_deref(), report)?;
                // Put the file back if it was missing here.
                let here = self.record(&target)?;
                if here.missing && ab.file_included {
                    let dest = self.free_book_path(&here.rel_path, None)?;
                    zip.extract(&ab.rel_path, &dest).map_err(archive_err)?;
                    if paths::hash_file(&dest)? == target {
                        let rel = paths::rel_of(self.layout(), &dest).ok_or(Error::BookNotFound)?;
                        let meta = fs::metadata(&dest)?;
                        self.with_db(|db| {
                            db.set_book_path(&target, &rel)?;
                            db.set_file_stamp(&target, meta.len(), paths::mtime_secs(&meta))
                        })?;
                        sidecar::write(self, &self.record(&target)?)?;
                        report.files_restored += 1;
                    } else {
                        let _ = fs::remove_file(&dest);
                    }
                }
                Ok(target)
            }
            Match::OtherFile(target) => {
                report.other_file += 1;
                self.with_db(|db| {
                    db.add_alias(&ab.id, &target, AliasKind::OtherFile)?;
                    for a in &ab.aliases {
                        db.add_alias(&a.id, &target, AliasKind::OtherFile)?;
                    }
                    Ok(())
                })?;
                sidecar::write(self, &self.record(&target)?)?;
                Ok(target)
            }
            Match::WithFile => {
                let dest = self.free_book_path(&ab.rel_path, Some(ab.file_type))?;
                zip.extract(&ab.rel_path, &dest).map_err(archive_err)?;
                let id = paths::hash_file(&dest)?;
                if id != ab.id {
                    report.warnings.push(format!(
                        "{}: the file in the archive has changed; it was added as a new book",
                        ab.title
                    ));
                    if let Some(existing) = self.with_db(|db| db.resolve_book_id(&id))? {
                        let _ = fs::remove_file(&dest);
                        report.linked += 1;
                        return Ok(existing);
                    }
                }
                let metadata = sc
                    .as_ref()
                    .map(|s| s.metadata.clone())
                    .unwrap_or_else(|| metadata_from(ab));
                let book = Book {
                    id: id.clone(),
                    rel_path: String::new(),
                    file_type: ab.file_type,
                    file_size: ab.file_size,
                    has_cover: false,
                    missing: false,
                    added_at: sc.as_ref().map_or_else(now, |s| s.added_at.clone()),
                    modified_at: sc.as_ref().map_or_else(now, |s| s.modified_at.clone()),
                    metadata,
                    user: BookUserState::default(),
                };
                let mut aliases = sc.as_ref().map(|s| s.aliases.clone()).unwrap_or_default();
                if id != ab.id {
                    aliases.push(Alias {
                        id: ab.id.clone(),
                        kind: AliasKind::OtherFile,
                    });
                }
                sidecar::write_with(self.layout(), &book, aliases)?;
                if let Some(bytes) = &cover {
                    if let Err(e) = covers::store(self.layout(), &id, bytes) {
                        report.warnings.push(format!("{}: cover: {e}", ab.title));
                    }
                }
                let registered =
                    self.register(&dest, id.clone(), ab.file_type, &mut report.warnings);
                if let Err(e) = registered {
                    let _ = fs::remove_file(&dest);
                    return Err(e);
                }
                report.added += 1;
                Ok(id)
            }
            Match::Missing => {
                let metadata = sc
                    .as_ref()
                    .map(|s| s.metadata.clone())
                    .unwrap_or_else(|| metadata_from(ab));
                let rel = self.free_book_path(&ab.rel_path, Some(ab.file_type))?;
                let rel = paths::rel_of(self.layout(), &rel).ok_or(Error::BookNotFound)?;
                let mut book = Book {
                    id: ab.id.clone(),
                    rel_path: rel,
                    file_type: ab.file_type,
                    file_size: ab.file_size,
                    has_cover: false,
                    missing: true,
                    added_at: sc.as_ref().map_or_else(now, |s| s.added_at.clone()),
                    modified_at: sc.as_ref().map_or_else(now, |s| s.modified_at.clone()),
                    metadata,
                    user: BookUserState::default(),
                };
                if let Some(bytes) = &cover {
                    book.has_cover = covers::store(self.layout(), &book.id, bytes).is_ok();
                }
                self.with_db(|db| {
                    db.insert_book(&book, 0)?;
                    for a in &ab.aliases {
                        db.add_alias(&a.id, &book.id, a.kind)?;
                    }
                    Ok(())
                })?;
                sidecar::write(self, &book)?;
                report.missing += 1;
                report.missing_books.push(book.id.clone());
                Ok(book.id)
            }
        }
    }

    /// A path for a book file under `Books/` that is free on disk and in the
    /// catalogue: the archive's own path if possible.
    fn free_book_path(
        &self,
        rel: &str,
        file_type: Option<libreri_core::FileType>,
    ) -> Result<PathBuf> {
        let wanted = self
            .layout()
            .resolve_relative(rel)
            .filter(|_| rel.starts_with("Books/"))
            .unwrap_or_else(|| {
                let name = Path::new(rel)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| {
                        format!("Imported book.{}", file_type.map_or("pdf", |t| t.as_str()))
                    });
                self.layout().books_dir().join(name)
            });
        let dir = wanted
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.layout().books_dir());
        fs::create_dir_all(&dir)?;
        let name = wanted
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut path = unique_path(&dir, &name);
        // Also free in the catalogue (a missing book may own the path).
        let mut n = 2;
        while let Some(r) = paths::rel_of(self.layout(), &path) {
            if self
                .with_db(|db| db.book_by_path(&r, &self.viewer()))?
                .is_none()
            {
                break;
            }
            let (stem, ext) = match name.rsplit_once('.') {
                Some((s, e)) => (s.to_owned(), format!(".{e}")),
                None => (name.clone(), String::new()),
            };
            path = unique_path(&dir, &format!("{stem} ({n}){ext}"));
            n += 1;
        }
        Ok(path)
    }

    /// Book details: the newer version wins; tags and categories are added
    /// together; a missing cover is filled in.
    fn merge_details(
        &self,
        target: &BookId,
        sc: Option<&Sidecar>,
        ab: &ArchiveBook,
        cover: Option<&[u8]>,
        report: &mut ArchiveImportReport,
    ) -> Result<()> {
        let here = self.record(target)?;
        let mut changed = false;
        if let Some(sc) = sc {
            let mut m = if sc.modified_at > here.modified_at {
                sc.metadata.clone()
            } else {
                here.metadata.clone()
            };
            let union = |a: &[String], b: &[String]| {
                let mut out: Vec<String> = a.to_vec();
                for x in b {
                    if !out.iter().any(|y| y.eq_ignore_ascii_case(x)) {
                        out.push(x.clone());
                    }
                }
                out
            };
            m.tags = union(&here.metadata.tags, &sc.metadata.tags);
            m.categories = union(&here.metadata.categories, &sc.metadata.categories);
            if m != here.metadata {
                if let Ok(m) = m.normalized() {
                    self.with_db(|db| db.update_metadata(target, &m, &now()))?;
                    changed = true;
                }
            }
        }
        let aliases: Vec<&Alias> = ab.aliases.iter().collect();
        if !aliases.is_empty() {
            self.with_db(|db| {
                for a in &aliases {
                    db.add_alias(&a.id, target, a.kind)?;
                }
                Ok(())
            })?;
        }
        if !here.has_cover {
            if let Some(bytes) = cover {
                if covers::store(self.layout(), target, bytes).is_ok() {
                    self.with_db(|db| db.set_has_cover(target, true, &now()))?;
                    changed = true;
                }
            }
        }
        if changed {
            report.details_updated += 1;
        }
        sidecar::write(self, &self.record(target)?)?;
        Ok(())
    }

    fn merge_personal(
        &self,
        profile: &ProfileId,
        book: &BookId,
        backup: PersonalBackup,
        report: &mut ArchiveImportReport,
    ) -> Result<()> {
        for mut a in backup.annotations {
            a.book_id = book.clone();
            let Ok(a) = a.validated() else {
                continue;
            };
            match self.with_db(|db| db.annotation(&a.id))? {
                None => {
                    self.with_db(|db| db.save_annotation(&a, profile))?;
                    report.notes_added += 1;
                }
                Some((_, owner)) if &owner != profile => {
                    report
                        .warnings
                        .push(format!("a note ({}) belongs to someone else here", a.id));
                }
                Some((old, _)) if a.modified_at > old.modified_at => {
                    self.with_db(|db| db.save_annotation(&a, profile))?;
                    report.notes_updated += 1;
                }
                Some(_) => report.notes_kept += 1,
            }
        }
        let current = self.with_db(|db| db.book(book, profile))?.map(|b| b.user);
        let current_pos = self.with_db(|db| db.position(book, profile))?;
        if let Some(state) = backup.state {
            let mut merged = current.clone().unwrap_or_default();
            if newer(state.last_opened.as_deref(), merged.last_opened.as_deref())
                || merged == BookUserState::default()
            {
                merged.status = state.status;
                merged.progress = state.progress;
                merged.last_opened = state.last_opened.clone();
            }
            if merged.rating == 0 {
                merged.rating = state.rating;
            }
            merged.favorite |= state.favorite;
            if Some(&merged) != current.as_ref() {
                self.with_db(|db| db.set_user_state(book, profile, &merged))?;
            }
            if let Some(pos) = backup.position {
                let take = current_pos.is_none()
                    || newer(
                        state.last_opened.as_deref(),
                        current.as_ref().and_then(|c| c.last_opened.as_deref()),
                    );
                if take {
                    let opened = merged.last_opened.clone().unwrap_or_else(now);
                    self.with_db(|db| {
                        db.set_position(book, profile, &pos, merged.progress, &opened)
                    })?;
                }
            }
        } else if let (Some(pos), None) = (backup.position, current_pos) {
            self.with_db(|db| db.set_position(book, profile, &pos, 0.0, &now()))?;
        }
        self.backup_personal_of(profile, book)
    }

    fn merge_collections(
        &self,
        archive_profile: &ProfileId,
        target: &ProfileId,
        zip: &mut ArchiveReader,
    ) -> Result<()> {
        let name = format!(".library-data/profiles/{archive_profile}.collections.json");
        if !zip.has(&name) {
            return Ok(());
        }
        let bytes = zip.read(&name).map_err(archive_err)?;
        let Ok(list) = serde_json::from_slice::<Vec<CollectionBackup>>(&bytes) else {
            return Ok(());
        };
        let existing = self.with_db(|db| db.collections(target))?;
        let stamp = now();
        let mut added = false;
        for c in list {
            if existing.iter().any(|e| e.id == c.id) {
                continue;
            }
            let record = CollectionRecord {
                id: c.id,
                name: c.name,
                query: c.query,
                position: c.position,
            };
            self.with_db(|db| db.save_collection(target, &record, &stamp))?;
            added = true;
        }
        if added {
            self.backup_collections(target)?;
        }
        Ok(())
    }

    /// Notebooks, notes and attachments: new files are added; identical
    /// ones skipped; different ones kept under a new name.
    fn import_note_files(
        &self,
        manifest: &Manifest,
        targets: &[(ProfileId, ProfileId)],
        zip: &mut ArchiveReader,
        report: &mut ArchiveImportReport,
    ) -> Result<()> {
        let folders: HashMap<&str, &ProfileId> = manifest
            .profiles
            .iter()
            .map(|p| (p.notes_folder.as_str(), &p.id))
            .collect();
        let target_of: HashMap<&ProfileId, &ProfileId> =
            targets.iter().map(|(a, t)| (a, t)).collect();
        let stamp = chrono::Utc::now().format("%Y-%m-%d").to_string();
        for name in zip.names() {
            let Some(rest) = name.strip_prefix("Notes/") else {
                continue;
            };
            let Some((folder, inner)) = rest.split_once('/') else {
                continue;
            };
            let Some(target) = folders.get(folder).and_then(|a| target_of.get(a)) else {
                continue;
            };
            let Some(tname) = self.with_db(|db| db.profile_name(target))? else {
                continue;
            };
            let base = self
                .layout()
                .notes_dir()
                .join(crate::reading::notes_folder_name(&tname));
            let Some(dest) = libreri_core::LibraryLayout::new(&base).resolve_relative(inner) else {
                continue;
            };
            let bytes_new = zip.read(&name).map_err(archive_err)?;
            let dest = if dest.exists() {
                if fs::read(&dest).map_err(io_err)? == bytes_new {
                    continue;
                }
                let file = dest
                    .file_name()
                    .map(|f| f.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let (stem, ext) = match file.rsplit_once('.') {
                    Some((s, e)) => (s.to_owned(), format!(".{e}")),
                    None => (file.clone(), String::new()),
                };
                report.note_conflicts.push(format!("{folder}/{inner}"));
                unique_path(
                    dest.parent().unwrap_or(&base),
                    &format!("{stem} (imported {stamp}){ext}"),
                )
            } else {
                dest
            };
            if let Some(dir) = dest.parent() {
                fs::create_dir_all(dir)?;
            }
            write_atomic(&dest, &bytes_new)?;
            report.note_files_added += 1;
            if dest.extension().is_some_and(|e| e == "md") {
                let linked = crate::reading::linked_book(&String::from_utf8_lossy(&bytes_new))
                    .and_then(|id| self.with_db(|db| db.resolve_book_id(&id)).ok().flatten());
                if let (Some(book), Some(rel)) = (linked, paths::rel_of(self.layout(), &dest)) {
                    if self
                        .with_db(|db| db.notebook_path(&book, target))?
                        .is_none()
                    {
                        self.with_db(|db| db.set_notebook_path(&book, target, &rel))?;
                    }
                }
            }
        }
        Ok(())
    }
}

/// Details for a book whose sidecar is not in the archive.
fn metadata_from(ab: &ArchiveBook) -> BookMetadata {
    BookMetadata {
        title: if ab.title.trim().is_empty() {
            "Untitled".into()
        } else {
            ab.title.clone()
        },
        authors: ab.authors.clone(),
        isbn13: ab.isbn13.clone(),
        isbn10: ab.isbn10.clone(),
        doi: ab.doi.clone(),
        arxiv_id: ab.arxiv_id.clone(),
        pages: ab.pages,
        content_type: ab.file_type.default_content_type(),
        ..Default::default()
    }
}
