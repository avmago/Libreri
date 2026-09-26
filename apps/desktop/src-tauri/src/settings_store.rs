//! Loads and saves `settings.json` in the OS app-config folder.

use libreri_core::AppSettings;
use std::fs;
use std::path::{Path, PathBuf};

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(config_dir: &Path) -> Self {
        Self {
            path: config_dir.join("settings.json"),
        }
    }

    /// Reads settings, falling back to defaults if the file is missing or
    /// damaged (a damaged file is kept as `settings.json.bak`).
    pub fn load(&self) -> AppSettings {
        match fs::read_to_string(&self.path) {
            Ok(text) => serde_json::from_str(&text).unwrap_or_else(|_| {
                let _ = fs::rename(&self.path, self.path.with_extension("json.bak"));
                AppSettings::default()
            }),
            Err(_) => AppSettings::default(),
        }
    }

    /// Writes settings atomically.
    pub fn save(&self, settings: &AppSettings) -> std::io::Result<()> {
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
        fs::rename(tmp, &self.path)
    }
}
