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
