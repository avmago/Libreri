//! Data sent to and from the UI.
//!
//! These mirror core types but live in the shell, so `libreri-core` stays
//! free of any IPC or TypeScript concerns. tauri-specta turns them into
//! TypeScript types automatically.

use libreri_core::settings::{AppSettings, RecentLibrary};
use libreri_core::ThemePreference;
use libreri_core::{Book, ContentType, FileType};
use libreri_library::{Facets, FolderNode, Library};
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
            book_count: library
                .with_db(|db| db.book_count())
                .unwrap_or(0)
                .min(u32::MAX as u64) as u32,
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

/// A book as the interface sees it: the stored record plus the paths of
/// its images (served by `book://`) and its folder.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BookDto {
    #[serde(flatten)]
    pub book: Book,
    /// Folder relative to `Books/` ("" = top level).
    pub folder: String,
    pub thumbnail: Option<String>,
    pub cover: Option<String>,
}

impl From<Book> for BookDto {
    fn from(book: Book) -> Self {
        let (thumbnail, cover) = if book.has_cover {
            (
                Some(libreri_library::thumbnail_rel(&book.id)),
                Some(libreri_library::cover_rel(&book.id)),
            )
        } else {
            (None, None)
        };
        Self {
            folder: book.folder().to_owned(),
            thumbnail,
            cover,
            book,
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CountDto<T> {
    pub value: T,
    pub count: u32,
}

/// Counts for the sidebar and the filter menus.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FacetsDto {
    pub total: u32,
    pub want_to_read: u32,
    pub reading: u32,
    pub finished: u32,
    pub favorites: u32,
    pub audio: u32,
    pub missing: u32,
    pub file_types: Vec<CountDto<FileType>>,
    pub content_types: Vec<CountDto<ContentType>>,
    pub tags: Vec<CountDto<String>>,
    pub categories: Vec<CountDto<String>>,
}

fn counts<T: Clone>(list: &[(T, u32)]) -> Vec<CountDto<T>> {
    list.iter()
        .map(|(value, count)| CountDto {
            value: value.clone(),
            count: *count,
        })
        .collect()
}

impl From<&Facets> for FacetsDto {
    fn from(f: &Facets) -> Self {
        Self {
            total: f.total,
            want_to_read: f.want_to_read,
            reading: f.reading,
            finished: f.finished,
            favorites: f.favorites,
            audio: f.audio,
            missing: f.missing,
            file_types: counts(&f.file_types),
            content_types: counts(&f.content_types),
            tags: counts(&f.tags),
            categories: counts(&f.categories),
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FolderDto {
    pub name: String,
    /// Relative to `Books/`, `/`-separated.
    pub path: String,
    pub book_count: u32,
    pub total_count: u32,
    pub children: Vec<FolderDto>,
}

impl From<&FolderNode> for FolderDto {
    fn from(n: &FolderNode) -> Self {
        Self {
            name: n.name.clone(),
            path: n.path.clone(),
            book_count: n.book_count,
            total_count: n.total_count,
            children: n.children.iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Type, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ImportModeDto {
    Move,
    Copy,
}

impl From<ImportModeDto> for libreri_library::ImportMode {
    fn from(m: ImportModeDto) -> Self {
        match m {
            ImportModeDto::Move => Self::Move,
            ImportModeDto::Copy => Self::Copy,
        }
    }
}
