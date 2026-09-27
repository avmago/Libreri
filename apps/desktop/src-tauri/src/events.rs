//! Typed events sent from Rust to the UI.

use serde::Serialize;
use specta::Type;
use tauri_specta::Event;

/// Progress of a background job, forwarded from `libreri-jobs`.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct JobEventPayload {
    pub id: String,
    /// "started" | "progress" | "finished" | "failed" | "cancelled"
    pub kind: String,
    pub label: Option<String>,
    pub done: Option<u32>,
    pub total: Option<u32>,
    pub message: Option<String>,
}

impl From<libreri_jobs::JobEvent> for JobEventPayload {
    fn from(e: libreri_jobs::JobEvent) -> Self {
        use libreri_jobs::JobEvent as J;
        let clamp = |n: u64| n.min(u32::MAX as u64) as u32;
        let base = |id: libreri_jobs::JobId, kind: &str| Self {
            id: id.to_string(),
            kind: kind.to_owned(),
            label: None,
            done: None,
            total: None,
            message: None,
        };
        match e {
            J::Started { id, label } => Self {
                label: Some(label),
                ..base(id, "started")
            },
            J::Progress {
                id,
                done,
                total,
                message,
            } => Self {
                done: Some(clamp(done)),
                total: Some(clamp(total)),
                message,
                ..base(id, "progress")
            },
            J::Finished { id } => base(id, "finished"),
            J::Failed { id, error } => Self {
                message: Some(error),
                ..base(id, "failed")
            },
            J::Cancelled { id } => base(id, "cancelled"),
        }
    }
}

/// The books or folders changed (import, scan, rebuild). The interface
/// refetches its lists.
#[derive(Debug, Clone, Default, Serialize, Type, Event)]
pub struct LibraryChanged {}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateDto {
    pub file: String,
    pub existing_title: String,
    pub existing_id: String,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FailedFileDto {
    pub file: String,
    pub reason: String,
}

/// Summary of a finished import, for the report shown to the user.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ImportFinished {
    pub job_id: String,
    pub added: u32,
    pub added_ids: Vec<String>,
    pub duplicates: Vec<DuplicateDto>,
    pub relinked: u32,
    pub unsupported: u32,
    pub failed: Vec<FailedFileDto>,
    pub warnings: Vec<String>,
}

impl ImportFinished {
    pub fn new(job: libreri_jobs::JobId, r: &libreri_library::ImportReport) -> Self {
        Self {
            job_id: job.to_string(),
            added: r.added_ids.len() as u32,
            added_ids: r.added_ids.iter().map(ToString::to_string).collect(),
            duplicates: r
                .duplicates
                .iter()
                .map(|d| DuplicateDto {
                    file: d.file.clone(),
                    existing_title: d.existing_title.clone(),
                    existing_id: d.existing_id.to_string(),
                })
                .collect(),
            relinked: r.relinked,
            unsupported: r.unsupported,
            failed: r
                .failed
                .iter()
                .map(|(file, reason)| FailedFileDto {
                    file: file.clone(),
                    reason: reason.clone(),
                })
                .collect(),
            warnings: r.warnings.clone(),
        }
    }
}

/// Someone signed in or out (in any window). Every window checks who is
/// signed in now, so locking locks them all.
#[derive(Debug, Clone, Default, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SessionChanged {
    /// The profile now signed in, if any.
    pub profile_id: Option<String>,
}

/// "Fill in missing details" finished (by hand, or after an import).
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct DetailsFilled {
    pub job_id: String,
    pub filled: Vec<String>,
    /// Books with no sure match, to look up one by one.
    pub unsure: Vec<String>,
    pub unchanged: u32,
    pub failed: Vec<FailedFileDto>,
}

impl DetailsFilled {
    pub fn new(job: libreri_jobs::JobId, r: &libreri_library::FillReport) -> Self {
        Self {
            job_id: job.to_string(),
            filled: r.filled.iter().map(ToString::to_string).collect(),
            unsure: r.unsure.iter().map(ToString::to_string).collect(),
            unchanged: r.unchanged,
            failed: r
                .failed
                .iter()
                .map(|(file, reason)| FailedFileDto {
                    file: file.clone(),
                    reason: reason.clone(),
                })
                .collect(),
        }
    }
}

/// The phone page was opened, or it read a barcode.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct PhoneScan {
    /// "opened" | "scanned"
    pub kind: String,
    pub scanned: Option<crate::commands::scan::ScannedDto>,
}

/// An export finished.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ExportFinished {
    pub job_id: String,
    pub path: String,
    pub books: u32,
    pub notes: u32,
    pub files: u32,
    #[specta(type = u32)]
    pub bytes: u64,
    pub warnings: Vec<String>,
}

impl ExportFinished {
    pub fn new(job: libreri_jobs::JobId, r: &libreri_library::ExportReport) -> Self {
        Self {
            job_id: job.to_string(),
            path: r.path.to_string_lossy().into_owned(),
            books: r.books,
            notes: r.notes,
            files: r.files,
            bytes: r.bytes,
            warnings: r.warnings.clone(),
        }
    }
}

/// A Libreri archive was imported: the summary shown to the user.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveImported {
    pub job_id: String,
    pub linked: u32,
    pub added: u32,
    pub other_file: u32,
    pub missing: u32,
    pub files_restored: u32,
    pub details_updated: u32,
    pub notes_added: u32,
    pub notes_updated: u32,
    pub notes_kept: u32,
    pub note_files_added: u32,
    pub note_conflicts: Vec<String>,
    pub profiles_created: Vec<String>,
    pub missing_books: Vec<String>,
    pub warnings: Vec<String>,
}

impl ArchiveImported {
    pub fn new(job: libreri_jobs::JobId, r: &libreri_library::ArchiveImportReport) -> Self {
        Self {
            job_id: job.to_string(),
            linked: r.linked,
            added: r.added,
            other_file: r.other_file,
            missing: r.missing,
            files_restored: r.files_restored,
            details_updated: r.details_updated,
            notes_added: r.notes_added,
            notes_updated: r.notes_updated,
            notes_kept: r.notes_kept,
            note_files_added: r.note_files_added,
            note_conflicts: r.note_conflicts.clone(),
            profiles_created: r.profiles_created.clone(),
            missing_books: r.missing_books.iter().map(ToString::to_string).collect(),
            warnings: r.warnings.clone(),
        }
    }
}

/// A backup finished (by hand or on schedule).
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct BackupFinished {
    pub job_id: String,
    /// Set when the backup was written.
    pub path: Option<String>,
    pub error: Option<String>,
    /// Started by the schedule rather than "Back up now".
    pub scheduled: bool,
}

/// An import from another app finished.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct ForeignImported {
    pub job_id: String,
    pub source: String,
    pub added: u32,
    pub already_here: u32,
    pub details_added: u32,
    pub without_file: Vec<String>,
    pub without_file_count: u32,
    pub highlights_added: u32,
    pub notes_added: u32,
    pub personal_updated: u32,
    pub unmatched: Vec<String>,
    pub unmatched_count: u32,
    pub failed: Vec<FailedFileDto>,
    pub warnings: Vec<String>,
}

impl ForeignImported {
    pub fn new(job: libreri_jobs::JobId, source: &str, r: &libreri_library::ForeignReport) -> Self {
        Self {
            job_id: job.to_string(),
            source: source.to_owned(),
            added: r.added,
            already_here: r.already_here,
            details_added: r.details_added,
            without_file: r.without_file.clone(),
            without_file_count: r.without_file_count,
            highlights_added: r.highlights_added,
            notes_added: r.notes_added,
            personal_updated: r.personal_updated,
            unmatched: r.unmatched.clone(),
            unmatched_count: r.unmatched_count,
            failed: r
                .failed
                .iter()
                .map(|(file, reason)| FailedFileDto {
                    file: file.clone(),
                    reason: reason.clone(),
                })
                .collect(),
            warnings: r.warnings.iter().take(20).cloned().collect(),
        }
    }
}

/// Progress of installing a helper program.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct HelperInstall {
    pub helper: libreri_helpers::Helper,
    /// A line of the package manager's output.
    pub line: Option<String>,
    pub done: bool,
    pub error: Option<String>,
}

/// The search index is being brought up to date in the background.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct SearchIndexProgress {
    pub done: u32,
    pub total: u32,
    pub running: bool,
}

/// "Make searchable" finished (or stopped).
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct OcrFinished {
    pub job_id: String,
    pub book_ids: Vec<String>,
    /// Books done.
    pub books: u32,
    pub pages_read: u32,
    pub pages_failed: u32,
    pub errors: Vec<String>,
}

/// Progress of downloading an OCR language.
#[derive(Debug, Clone, Serialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct OcrLanguageDownload {
    pub code: String,
    pub done: f64,
    pub total: Option<f64>,
    pub finished: bool,
    pub error: Option<String>,
}
