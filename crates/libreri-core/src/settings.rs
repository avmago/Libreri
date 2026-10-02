//! App-wide settings kept in the operating system's app-config folder
//! (not inside a library), such as the theme and recently opened libraries.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// How the app chooses light or dark.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ThemePreference {
    #[default]
    System,
    Light,
    Dark,
    HighContrast,
}

/// A library the user opened before, shown on the Welcome screen.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentLibrary {
    pub name: String,
    pub path: PathBuf,
}

/// Settings that belong to this computer rather than to a library.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub theme: ThemePreference,
    /// Accent colour as a hex string; `None` means the default (black).
    pub accent: Option<String>,
    pub recent_libraries: Vec<RecentLibrary>,
    /// Languages OCR reads when a book does not say its own (Tesseract
    /// codes); empty means English.
    pub ocr_languages: Vec<String>,
    /// Speech recognition (whisper models are per computer).
    pub speech: SpeechSettings,
    /// How handwriting on canvases is read: "system" (macOS or Windows
    /// recognition) or "tesseract". `None` picks the system's where there
    /// is one.
    pub ink_engine: Option<String>,
    /// Read maths from pictures with the downloaded model (off: LaTeX is
    /// only copied where a book has it exactly).
    pub maths_from_pictures: bool,
    /// What reads scanned pages: `None` for Tesseract, or a downloaded
    /// model ("paddleocr-vl", ADR 0029).
    pub ocr_engine: Option<String>,
    /// Downloaded natural voices switched off ("kokoro", "piper:<voice>",
    /// ADR 0030); every other downloaded one is on.
    pub voices_off: Vec<String>,
    /// Do not look for a new version of Libreri when it starts.
    pub updates_off: bool,
}

/// Speech recognition: which downloaded model to use and how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpeechSettings {
    /// The model used (only a downloaded one can be chosen).
    pub model: Option<String>,
    /// The language spoken ("en"); `None` lets the model tell.
    pub language: Option<String>,
    /// Write down what is said in voice notes when they are recorded.
    pub transcribe_notes: bool,
}

impl Default for SpeechSettings {
    fn default() -> Self {
        Self {
            model: None,
            language: None,
            transcribe_notes: true,
        }
    }
}

/// How many libraries the Welcome screen remembers.
pub const MAX_RECENT_LIBRARIES: usize = 8;

impl AppSettings {
    /// Moves (or adds) a library to the top of the recent list.
    pub fn remember_library(&mut self, name: impl Into<String>, path: PathBuf) {
        self.recent_libraries.retain(|r| r.path != path);
        self.recent_libraries.insert(
            0,
            RecentLibrary {
                name: name.into(),
                path,
            },
        );
        self.recent_libraries.truncate(MAX_RECENT_LIBRARIES);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remember_library_deduplicates_and_orders() {
        let mut s = AppSettings::default();
        s.remember_library("A", "/a".into());
        s.remember_library("B", "/b".into());
        s.remember_library("A again", "/a".into());
        let names: Vec<_> = s.recent_libraries.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["A again", "B"]);
    }

    #[test]
    fn remember_library_caps_the_list() {
        let mut s = AppSettings::default();
        for i in 0..20 {
            s.remember_library(format!("L{i}"), format!("/l{i}").into());
        }
        assert_eq!(s.recent_libraries.len(), MAX_RECENT_LIBRARIES);
        assert_eq!(s.recent_libraries[0].name, "L19");
    }

    #[test]
    fn settings_tolerate_missing_fields() {
        let s: AppSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(s, AppSettings::default());
    }
}
