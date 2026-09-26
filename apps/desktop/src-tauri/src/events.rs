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
