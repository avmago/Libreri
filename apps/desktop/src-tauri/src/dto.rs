//! Data sent to and from the UI.
//!
//! These mirror core types but live in the shell, so `libreri-core` stays
//! free of any IPC or TypeScript concerns. tauri-specta turns them into
//! TypeScript types automatically.

use libreri_core::settings::{AppSettings, RecentLibrary};
use libreri_core::ThemePreference;
use libreri_library::Library;
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    System,
    Light,
    Dark,
    HighContrast,
}

impl From<ThemePreference> for Theme {
    fn from(t: ThemePreference) -> Self {
        match t {
            ThemePreference::System => Self::System,
            ThemePreference::Light => Self::Light,
            ThemePreference::Dark => Self::Dark,
            ThemePreference::HighContrast => Self::HighContrast,
        }
    }
}

impl From<Theme> for ThemePreference {
    fn from(t: Theme) -> Self {
        match t {
            Theme::System => Self::System,
            Theme::Light => Self::Light,
            Theme::Dark => Self::Dark,
            Theme::HighContrast => Self::HighContrast,
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecentLibraryDto {
    pub name: String,
    pub path: String,
    /// False when the folder no longer exists or is no longer a library.
    pub available: bool,
}

impl From<&RecentLibrary> for RecentLibraryDto {
    fn from(r: &RecentLibrary) -> Self {
        Self {
            name: r.name.clone(),
            path: r.path.to_string_lossy().into_owned(),
            available: Library::is_library(&r.path),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SettingsDto {
    pub theme: Theme,
    pub accent: Option<String>,
    pub recent_libraries: Vec<RecentLibraryDto>,
}

impl From<&AppSettings> for SettingsDto {
    fn from(s: &AppSettings) -> Self {
        Self {
            theme: s.theme.into(),
            accent: s.accent.clone(),
            recent_libraries: s.recent_libraries.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibrarySummary {
    pub id: String,
    pub name: String,
    pub path: String,
    /// `u32` is plenty for a book count and maps to a plain TypeScript number.
    pub book_count: u32,
}

impl LibrarySummary {
    pub fn of(library: &Library) -> Self {
        Self {
            id: library.info().id.to_string(),
            name: library.info().name.clone(),
            path: library.layout().root().to_string_lossy().into_owned(),
            book_count: library.db().book_count().unwrap_or(0).min(u32::MAX as u64) as u32,
        }
    }
}

/// What a folder the user picked contains, so the Welcome screen can offer
/// the right action.
#[derive(Debug, Clone, Copy, Serialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum FolderKind {
    Missing,
    Empty,
    Library,
    OtherFiles,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub version: String,
    pub platform: String,
}
