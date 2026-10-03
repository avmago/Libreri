//! `backups.json` in the OS app-config folder: for each library (by id),
//! whether and where to back it up, and a file to keep up to date. Kept per
//! computer because the backup folder is a path on this computer
//! (docs/adr/0014-export-import-backups.md).

use libreri_export::ExportFormat;
use serde::{Deserialize, Serialize};
use specta::Type;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

fn every_day() -> u32 {
    24
}

fn ten() -> u32 {
    10
}

/// A file (BibTeX, CSL-JSON…) Libreri rewrites whenever the library changes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AutoExport {
    pub format: ExportFormat,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct BackupSettings {
    /// Off until the owner turns it on.
    pub enabled: bool,
    pub folder: Option<String>,
    /// 24 (daily) or 168 (weekly).
    #[serde(default = "every_day")]
    pub every_hours: u32,
    /// How many backups to keep; older ones are removed.
    #[serde(default = "ten")]
    pub keep: u32,
    pub book_files: bool,
    pub last_backup: Option<String>,
    pub last_error: Option<String>,
    /// When the last backup (successful or not) started.
    pub last_attempt: Option<String>,
    pub auto_export: Option<AutoExport>,
    pub last_auto_export: Option<String>,
    pub auto_export_error: Option<String>,
}

impl Default for BackupSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            folder: None,
            every_hours: every_day(),
            keep: ten(),
            book_files: false,
            last_backup: None,
            last_error: None,
            last_attempt: None,
            auto_export: None,
            last_auto_export: None,
            auto_export_error: None,
        }
    }
}

impl BackupSettings {
    /// True when the last attempt failed less than an hour ago (so the
    /// schedule waits instead of retrying every few minutes).
    pub fn failed_recently(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        self.last_error.is_some()
            && self
                .last_attempt
                .as_deref()
                .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
                .is_some_and(|t| now.signed_duration_since(t) < chrono::Duration::hours(1))
    }

    /// True when a scheduled backup should run now.
    pub fn due(&self, now: chrono::DateTime<chrono::Utc>) -> bool {
        if !self.enabled || self.folder.as_deref().is_none_or(str::is_empty) {
            return false;
        }
        let Some(last) = self
            .last_backup
            .as_deref()
            .and_then(|t| chrono::DateTime::parse_from_rfc3339(t).ok())
        else {
            return true;
        };
        now.signed_duration_since(last)
            >= chrono::Duration::hours(i64::from(self.every_hours.max(1)))
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct File {
    #[serde(default)]
    libraries: BTreeMap<String, BackupSettings>,
}

pub struct BackupStore {
    path: PathBuf,
}

impl BackupStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join("backups.json"),
        }
    }

    fn load_all(&self) -> File {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    pub fn get(&self, library: &str) -> BackupSettings {
        self.load_all()
            .libraries
            .get(library)
            .cloned()
            .unwrap_or_default()
    }

    /// Changes one library's settings and writes the file atomically.
    pub fn update(
        &self,
        library: &str,
        change: impl FnOnce(&mut BackupSettings),
    ) -> std::io::Result<BackupSettings> {
        let mut all = self.load_all();
        let entry = all.libraries.entry(library.to_owned()).or_default();
        change(entry);
        let out = entry.clone();
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(&all)?)?;
        fs::rename(tmp, &self.path)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn off_by_default_and_due_once_a_day() {
        let dir = tempfile::tempdir().unwrap();
        let store = BackupStore::new(dir.path());
        let s = store.get("lib");
        assert!(!s.enabled);
        assert_eq!((s.every_hours, s.keep), (24, 10));
        let now = chrono::Utc::now();
        assert!(!s.due(now));

        let s = store
            .update("lib", |s| {
                s.enabled = true;
                s.folder = Some("/backups".into());
            })
            .unwrap();
        assert!(s.due(now), "never backed up");
        let s = store
            .update("lib", |s| {
                s.last_backup = Some((now - chrono::Duration::hours(3)).to_rfc3339())
            })
            .unwrap();
        assert!(!s.due(now));
        assert!(s.due(now + chrono::Duration::hours(22)));
        assert_eq!(store.get("other"), BackupSettings::default());

        let s = store
            .update("lib", |s| {
                s.last_error = Some("drive not found".into());
                s.last_attempt = Some((now - chrono::Duration::minutes(10)).to_rfc3339());
            })
            .unwrap();
        assert!(s.failed_recently(now));
        assert!(!s.failed_recently(now + chrono::Duration::hours(2)));
    }
}
