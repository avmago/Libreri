//! `online-sources.json` in the OS app-config folder: which sources to
//! look book details up in, and the reader's API keys. Kept per computer,
//! outside the library, so keys are never exported or synced with the
//! library folder (docs/data-portability.md).

use libreri_metadata::Settings;
use std::fs;
use std::path::{Path, PathBuf};

pub struct OnlineStore {
    path: PathBuf,
}

impl OnlineStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join("online-sources.json"),
        }
    }

    pub fn load(&self) -> Settings {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default()
    }

    /// Writes atomically; on Unix only the user can read the file.
    pub fn save(&self, settings: &Settings) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        {
            use std::io::Write;
            let mut f = fs::File::create(&tmp)?;
            f.write_all(&serde_json::to_vec_pretty(settings)?)?;
            // On disk before it replaces the old file (a crash keeps one whole).
            f.sync_all()?;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&tmp, fs::Permissions::from_mode(0o600))?;
        }
        fs::rename(tmp, &self.path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_and_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let store = OnlineStore::new(dir.path());
        assert_eq!(store.load(), Settings::default());
        let s = Settings {
            isbndb_key: Some("abc".into()),
            fill_on_import: true,
            ..Default::default()
        };
        store.save(&s).unwrap();
        assert_eq!(store.load(), s);
        fs::write(dir.path().join("online-sources.json"), "{ nope").unwrap();
        assert_eq!(store.load(), Settings::default());
    }
}
