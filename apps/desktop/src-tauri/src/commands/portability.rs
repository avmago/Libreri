//! Export, citations, Libreri archives, restoring, the health check,
//! locating missing files and backups.

use crate::backup_store::AutoExport;
use crate::dto::LibrarySummary;
use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use libreri_core::FileType;
use libreri_core::{BookId, ProfileId, ProfileKind};
use libreri_export::archive::ArchiveKind;
use libreri_export::citation::Citation;
use libreri_export::foreign::ForeignSource;
use libreri_export::{CitationStyle, ExportFormat};
use libreri_library::{
    ArchiveImport, ExportRequest, ForeignImport, ImportMode, Library, LocateOutcome, ProfileTarget,
};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn book_id(id: &str) -> AppResult<BookId> {
    BookId::from_hex(id).map_err(|e| AppError::invalid(e.to_string()))
}

fn profile_id(id: &str) -> AppResult<ProfileId> {
    id.parse()
        .map_err(|_| AppError::invalid("that profile is not valid"))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}

/// What the export dialog asks for (board 24).
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequestDto {
    pub format: ExportFormat,
    /// `None` = every book.
    pub book_ids: Option<Vec<String>>,
    pub dest: String,
    pub personal: bool,
    pub notes: bool,
    pub book_files: bool,
    pub everyone: bool,
}

/// Starts an export. Returns the job id; the result arrives as an
/// `ExportFinished` event.
#[tauri::command]
#[specta::specta]
pub fn export_books(state: State<'_, AppState>, request: ExportRequestDto) -> AppResult<String> {
    let books = request
        .book_ids
        .map(|ids| {
            ids.iter()
                .map(|i| book_id(i))
                .collect::<AppResult<Vec<_>>>()
        })
        .transpose()?;
    state.library()?.require_export()?;
    if request.dest.trim().is_empty() {
        return Err(AppError::invalid("choose where to save the export"));
    }
    let req = ExportRequest {
        format: request.format,
        books,
        dest: PathBuf::from(request.dest),
        personal: request.personal,
        notes: request.notes,
        book_files: request.book_files,
        everyone: request.everyone,
        app_version: APP_VERSION.to_owned(),
    };
    Ok(state.start_export(req)?.to_string())
}

/// References for books, ready to paste.
#[tauri::command]
#[specta::specta]
pub async fn copy_citation(
    state: State<'_, AppState>,
    ids: Vec<String>,
    style: CitationStyle,
) -> AppResult<Citation> {
    let library = state.library()?;
    let ids = ids
        .iter()
        .map(|i| book_id(i))
        .collect::<AppResult<Vec<_>>>()?;
    blocking(move || Ok(library.citations(&ids, style)?)).await
}

/// Where one archive profile's notes go.
#[derive(Debug, Clone, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ProfileTargetDto {
    #[serde(rename_all = "camelCase")]
    Existing {
        profile_id: String,
    },
    New,
    Skip,
}

impl From<&ProfileTarget> for ProfileTargetDto {
    fn from(t: &ProfileTarget) -> Self {
        match t {
            ProfileTarget::Existing(id) => Self::Existing {
                profile_id: id.to_string(),
            },
            ProfileTarget::New => Self::New,
            ProfileTarget::Skip => Self::Skip,
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveProfileDto {
    pub archive_id: String,
    pub name: String,
    pub kind: ProfileKind,
    pub suggestion: ProfileTargetDto,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSummaryDto {
    pub kind: ArchiveKind,
    pub created_at: String,
    pub created_by: String,
    pub library_name: String,
    pub includes_book_files: bool,
    pub includes_pins: bool,
    pub books: u32,
    pub notes: u32,
    pub notebooks: u32,
    pub linked: u32,
    pub with_file: u32,
    pub other_file: u32,
    pub missing: u32,
    pub profiles: Vec<ArchiveProfileDto>,
}

/// What a Libreri archive holds and what importing it would do.
#[tauri::command]
#[specta::specta]
pub async fn inspect_archive(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<ArchiveSummaryDto> {
    let library = state.library()?;
    blocking(move || {
        let s = library.inspect_archive(Path::new(&path))?;
        Ok(ArchiveSummaryDto {
            kind: s.kind,
            created_at: s.created_at,
            created_by: s.created_by,
            library_name: s.library_name,
            includes_book_files: s.includes_book_files,
            includes_pins: s.includes_pins,
            books: s.books,
            notes: s.notes,
            notebooks: s.notebooks,
            linked: s.linked,
            with_file: s.with_file,
            other_file: s.other_file,
            missing: s.missing,
            profiles: s
                .profiles
                .iter()
                .map(|p| ArchiveProfileDto {
                    archive_id: p.archive.id.to_string(),
                    name: p.archive.name.clone(),
                    kind: p.archive.kind,
                    suggestion: (&p.suggestion).into(),
                })
                .collect(),
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProfileMappingDto {
    pub archive_id: String,
    pub target: ProfileTargetDto,
}

fn mappings(list: Vec<ProfileMappingDto>) -> AppResult<Vec<(ProfileId, ProfileTarget)>> {
    list.into_iter()
        .map(|m| {
            let target = match m.target {
                ProfileTargetDto::Existing { profile_id: id } => {
                    ProfileTarget::Existing(profile_id(&id)?)
                }
                ProfileTargetDto::New => ProfileTarget::New,
                ProfileTargetDto::Skip => ProfileTarget::Skip,
            };
            Ok((profile_id(&m.archive_id)?, target))
        })
        .collect()
}

/// Imports a Libreri archive into the open library (owner only). Returns
/// the job id; the report arrives as an `ArchiveImported` event.
#[tauri::command]
#[specta::specta]
pub fn import_archive(
    state: State<'_, AppState>,
    path: String,
    profiles: Vec<ProfileMappingDto>,
) -> AppResult<String> {
    state.library()?.require_owner_profile()?;
    let choice = ArchiveImport {
        profiles: mappings(profiles)?,
        adopt_owner: false,
    };
    Ok(state
        .start_archive_import(PathBuf::from(path), choice)?
        .to_string())
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RestoredLibrary {
    pub library: LibrarySummary,
    pub job_id: String,
}

/// Makes a new library in `folder` (new or empty) and restores a backup or
/// export into it, owner included.
#[tauri::command]
#[specta::specta]
pub fn restore_library(
    state: State<'_, AppState>,
    archive: String,
    folder: String,
) -> AppResult<RestoredLibrary> {
    let archive = PathBuf::from(archive);
    // Refuse a file that is not an archive before making anything.
    libreri_export::archive::ArchiveReader::open(&archive)
        .map_err(|e| AppError::invalid(e.to_string()))?;
    state.close_library();
    let library = Library::create(Path::new(&folder), None, APP_VERSION)?;
    let summary = crate::commands::library::adopt(&state, library)?;
    let job = state.start_archive_import(
        archive,
        ArchiveImport {
            profiles: Vec::new(),
            adopt_owner: true,
        },
    )?;
    Ok(RestoredLibrary {
        library: summary,
        job_id: job.to_string(),
    })
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BookRef {
    pub id: String,
    pub title: String,
    pub rel_path: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CountedBook {
    pub book: BookRef,
    pub notes: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub key: String,
    pub books: Vec<BookRef>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BrokenLinkDto {
    pub note: String,
    pub line: u32,
    pub link: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HealthReportDto {
    pub checked_at: String,
    pub books: u32,
    pub missing_files: Vec<BookRef>,
    pub other_file_notes: Vec<CountedBook>,
    pub duplicates: Vec<DuplicateGroup>,
    pub broken_links: Vec<BrokenLinkDto>,
    pub missing_sidecars: u32,
    pub stale_notebooks: u32,
    pub unused_covers: u32,
    #[specta(type = u32)]
    pub unused_cover_bytes: u64,
    pub unreadable_backups: Vec<String>,
    pub database: Vec<String>,
    pub fixable: bool,
}

fn book_ref(library: &Library, id: &BookId) -> BookRef {
    match library.book(id) {
        Ok(b) => BookRef {
            id: b.id.to_string(),
            title: b.metadata.title,
            rel_path: b.rel_path,
        },
        Err(_) => BookRef {
            id: id.to_string(),
            title: "(not visible to you)".into(),
            rel_path: String::new(),
        },
    }
}

/// Checks the library for missing files, broken links and other problems.
#[tauri::command]
#[specta::specta]
pub async fn health_check(state: State<'_, AppState>) -> AppResult<HealthReportDto> {
    let library = state.library()?;
    blocking(move || {
        let r = library.health_check()?;
        Ok(HealthReportDto {
            checked_at: r.checked_at.clone(),
            books: r.books,
            missing_files: r
                .missing_files
                .iter()
                .map(|id| book_ref(&library, id))
                .collect(),
            other_file_notes: r
                .other_file_notes
                .iter()
                .map(|(id, n)| CountedBook {
                    book: book_ref(&library, id),
                    notes: *n,
                })
                .collect(),
            duplicates: r
                .duplicates
                .iter()
                .map(|(key, ids)| DuplicateGroup {
                    key: key.clone(),
                    books: ids.iter().map(|id| book_ref(&library, id)).collect(),
                })
                .collect(),
            broken_links: r
                .broken_links
                .iter()
                .map(|l| BrokenLinkDto {
                    note: l.note.clone(),
                    line: l.line,
                    link: l.link.clone(),
                })
                .collect(),
            missing_sidecars: r.missing_sidecars,
            stale_notebooks: r.stale_notebooks,
            unused_covers: r.unused_covers,
            unused_cover_bytes: r.unused_cover_bytes,
            unreadable_backups: r.unreadable_backups.clone(),
            database: r.database.clone(),
            fixable: r.fixable(),
        })
    })
    .await
}

/// Fixes what the health check can fix by itself. Returns how many things.
#[tauri::command]
#[specta::specta]
pub async fn repair_health(state: State<'_, AppState>) -> AppResult<u32> {
    let library = state.library()?;
    blocking(move || Ok(library.repair_health()?)).await
}

#[derive(Debug, Clone, Copy, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum LocateResult {
    Linked,
    /// Another copy or edition: ask before using it.
    DifferentFile,
}

/// Puts a missing book's file back ("Locate file…").
#[tauri::command]
#[specta::specta]
pub async fn locate_file(
    state: State<'_, AppState>,
    id: String,
    path: String,
    copy: bool,
    accept_other: bool,
) -> AppResult<LocateResult> {
    let library = state.library()?;
    let id = book_id(&id)?;
    let result = blocking(move || {
        let mode = if copy {
            ImportMode::Copy
        } else {
            ImportMode::Move
        };
        Ok(library.locate_file(&id, Path::new(&path), mode, accept_other)?)
    })
    .await?;
    if result == LocateOutcome::Linked {
        state.schedule_scan();
    }
    Ok(match result {
        LocateOutcome::Linked => LocateResult::Linked,
        LocateOutcome::DifferentFile => LocateResult::DifferentFile,
    })
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupFileDto {
    pub path: String,
    pub created_at: String,
    #[specta(type = u32)]
    pub size: u64,
    pub includes_book_files: bool,
    pub books: u32,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettingsDto {
    pub enabled: bool,
    pub folder: Option<String>,
    pub every_hours: u32,
    pub keep: u32,
    pub book_files: bool,
    pub last_backup: Option<String>,
    pub last_error: Option<String>,
    pub last_attempt: Option<String>,
    pub auto_export: Option<AutoExport>,
    pub last_auto_export: Option<String>,
    pub auto_export_error: Option<String>,
    /// This library's backups in the folder, newest first.
    pub backups: Vec<BackupFileDto>,
    /// The signed-in profile may change these settings (the owner).
    pub can_edit: bool,
}

fn settings_dto(state: &AppState) -> AppResult<BackupSettingsDto> {
    let library = state.library()?;
    let (id, settings) = state.backup_settings()?;
    let backups = settings
        .folder
        .as_deref()
        .filter(|f| !f.is_empty())
        .map(|f| libreri_library::list_backups(Path::new(f), &id))
        .unwrap_or_default()
        .into_iter()
        .map(|b| BackupFileDto {
            path: b.path.to_string_lossy().into_owned(),
            created_at: b.created_at,
            size: b.size,
            includes_book_files: b.includes_book_files,
            books: b.books,
        })
        .collect();
    Ok(BackupSettingsDto {
        enabled: settings.enabled,
        folder: settings.folder,
        every_hours: settings.every_hours,
        keep: settings.keep,
        book_files: settings.book_files,
        last_backup: settings.last_backup,
        last_error: settings.last_error,
        last_attempt: settings.last_attempt,
        auto_export: settings.auto_export,
        last_auto_export: settings.last_auto_export,
        auto_export_error: settings.auto_export_error,
        backups,
        can_edit: library.require_owner_profile().is_ok(),
    })
}

/// Backups and the file kept up to date, for Settings › Export & import.
#[tauri::command]
#[specta::specta]
pub async fn get_backup_settings(state: State<'_, AppState>) -> AppResult<BackupSettingsDto> {
    settings_dto(&state)
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupSettingsChange {
    pub enabled: bool,
    pub folder: Option<String>,
    pub every_hours: u32,
    pub keep: u32,
    pub book_files: bool,
    pub auto_export: Option<AutoExport>,
}

/// Changes the backup settings (owner only).
#[tauri::command]
#[specta::specta]
pub async fn set_backup_settings(
    state: State<'_, AppState>,
    change: BackupSettingsChange,
) -> AppResult<BackupSettingsDto> {
    let library = state.library()?;
    library.require_owner_profile()?;
    if change.enabled && change.folder.as_deref().is_none_or(|f| f.trim().is_empty()) {
        return Err(AppError::invalid("choose a folder for backups first"));
    }
    if let Some(folder) = change.folder.as_deref().filter(|f| !f.is_empty()) {
        let folder = Path::new(folder);
        if folder.starts_with(library.layout().root()) {
            return Err(AppError::invalid(
                "keep backups outside the library folder, ideally on another drive",
            ));
        }
    }
    if let Some(auto) = &change.auto_export {
        if !matches!(
            auto.format,
            ExportFormat::Bibtex
                | ExportFormat::Ris
                | ExportFormat::CslJson
                | ExportFormat::Csv
                | ExportFormat::Json
        ) {
            return Err(AppError::invalid(
                "only BibTeX, RIS, CSL-JSON, CSV and JSON files can be kept up to date",
            ));
        }
        if auto.path.trim().is_empty() {
            return Err(AppError::invalid("choose where to keep the file"));
        }
    }
    let id = library.info().id.to_string();
    let auto_changed = state.backups.get(&id).auto_export != change.auto_export;
    state.backups.update(&id, |s| {
        s.enabled = change.enabled;
        s.folder = change.folder.filter(|f| !f.trim().is_empty());
        s.every_hours = if change.every_hours >= 168 { 168 } else { 24 };
        s.keep = change.keep.clamp(1, 100);
        s.book_files = change.book_files;
        s.auto_export = change.auto_export;
        if auto_changed {
            s.last_auto_export = None;
            s.auto_export_error = None;
        }
    })?;
    // Write the kept file straight away, so the reader sees it work.
    if auto_changed {
        if let Some(auto) = state.backups.get(&id).auto_export {
            let lib = library.clone();
            let result =
                blocking(move || Ok(lib.write_catalogue(auto.format, Path::new(&auto.path))?))
                    .await;
            let stamp = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
            state.backups.update(&id, |s| match result {
                Ok(_) => s.last_auto_export = Some(stamp),
                Err(e) => s.auto_export_error = Some(e.message),
            })?;
        }
    }
    settings_dto(&state)
}

/// Starts a backup now. Returns the job id, or `None` if one is running.
#[tauri::command]
#[specta::specta]
pub fn back_up_now(state: State<'_, AppState>) -> AppResult<Option<String>> {
    state.library()?.require_owner_profile()?;
    Ok(state.start_backup(false)?.map(|j| j.to_string()))
}

/// Shows a file (an export or a backup) in the system file manager.
#[tauri::command]
#[specta::specta]
pub fn reveal_path(app: AppHandle, path: String) -> AppResult<()> {
    let path = PathBuf::from(path);
    if !path.exists() {
        return Err(AppError::new(
            AppErrorKind::NotFound,
            "that file is no longer there",
        ));
    }
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FormatCount {
    pub file_type: FileType,
    pub count: u32,
}

/// What a library from another app holds.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ForeignSummaryDto {
    pub source: ForeignSource,
    pub label: String,
    pub books: u32,
    pub with_files: u32,
    pub highlights: u32,
    pub notes: u32,
    pub rated: u32,
    pub formats: Vec<FormatCount>,
    /// Goodreads and StoryGraph: books found in this library.
    pub matched: u32,
    pub reading_log: bool,
    pub warnings: Vec<String>,
}

/// Reads a Calibre library, Zotero folder, BibTeX/RIS file or reading log
/// and says what importing it would bring in.
#[tauri::command]
#[specta::specta]
pub async fn inspect_foreign(
    state: State<'_, AppState>,
    path: String,
) -> AppResult<ForeignSummaryDto> {
    let library = state.library()?;
    blocking(move || {
        let s = library.inspect_foreign(Path::new(&path))?;
        let source = s
            .source
            .ok_or_else(|| AppError::invalid("Libreri does not recognise this"))?;
        Ok(ForeignSummaryDto {
            source,
            label: source.label().to_owned(),
            books: s.books,
            with_files: s.with_files,
            highlights: s.highlights,
            notes: s.notes,
            rated: s.rated,
            formats: s
                .formats
                .into_iter()
                .map(|(file_type, count)| FormatCount { file_type, count })
                .collect(),
            matched: s.matched,
            reading_log: source.is_reading_log(),
            warnings: s.warnings,
        })
    })
    .await
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ForeignImportDto {
    /// Preferred formats, best first; empty = Libreri's order.
    pub formats: Vec<FileType>,
    /// Folder under Books/ ("" = top level).
    pub folder: String,
    pub replace_personal: bool,
}

/// Imports from another app (files are copied). Returns the job id; the
/// report arrives as a `ForeignImported` event.
#[tauri::command]
#[specta::specta]
pub fn import_foreign(
    state: State<'_, AppState>,
    path: String,
    options: ForeignImportDto,
) -> AppResult<String> {
    let library = state.library()?;
    library.require_edit()?;
    let path = PathBuf::from(path);
    let source = libreri_export::foreign::detect(&path)
        .ok_or_else(|| AppError::invalid("Libreri does not recognise this"))?;
    let opts = ForeignImport {
        formats: options.formats,
        folder: options.folder,
        replace_personal: options.replace_personal,
    };
    Ok(state
        .start_foreign_import(path, opts, source.label())?
        .to_string())
}
