//! The Notes hub.

use crate::error::AppResult;
use crate::state::AppState;
use libreri_core::Annotation;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteDto {
    pub annotation: Annotation,
    pub book_title: String,
    pub file_type: libreri_core::FileType,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NotebookEntryDto {
    pub book_id: Option<String>,
    pub title: String,
    pub rel_path: String,
    /// Seconds since 1970.
    pub modified: f64,
    pub excerpt: String,
    pub words: u32,
}

/// All highlights, comments and bookmarks of the signed-in profile.
#[tauri::command]
#[specta::specta]
pub fn list_all_notes(state: State<'_, AppState>) -> AppResult<Vec<NoteDto>> {
    Ok(state
        .library()?
        .all_notes()?
        .into_iter()
        .map(|n| NoteDto {
            annotation: n.annotation,
            book_title: n.book_title,
            file_type: n.file_type,
        })
        .collect())
}

/// All Markdown notes in the signed-in profile's notes folder.
#[tauri::command]
#[specta::specta]
pub fn list_notebooks(state: State<'_, AppState>) -> AppResult<Vec<NotebookEntryDto>> {
    Ok(state
        .library()?
        .notebooks()?
        .into_iter()
        .map(|n| NotebookEntryDto {
            book_id: n.book_id.map(|b| b.to_string()),
            title: n.title,
            rel_path: n.rel_path,
            modified: n.modified as f64,
            excerpt: n.excerpt,
            words: n.words,
        })
        .collect())
}

#[tauri::command]
#[specta::specta]
pub fn read_note(state: State<'_, AppState>, rel_path: String) -> AppResult<String> {
    Ok(state.library()?.read_note(&rel_path)?)
}

#[tauri::command]
#[specta::specta]
pub fn write_note(state: State<'_, AppState>, rel_path: String, content: String) -> AppResult<()> {
    Ok(state.library()?.write_note(&rel_path, &content)?)
}

/// Starts a note that is not about one book. Returns its path.
#[tauri::command]
#[specta::specta]
pub fn create_note(state: State<'_, AppState>, title: String) -> AppResult<String> {
    Ok(state.library()?.create_note(&title)?)
}

/// Shows the signed-in profile's notes folder in the file manager.
#[tauri::command]
#[specta::specta]
pub fn reveal_notes_folder(app: AppHandle, state: State<'_, AppState>) -> AppResult<String> {
    let dir = state.library()?.notes_folder()?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| crate::error::AppError::new(crate::error::AppErrorKind::Io, e.to_string()))?;
    Ok(dir.to_string_lossy().into_owned())
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageDto {
    pub books: f64,
    pub notes: f64,
    pub data: f64,
}

/// How much space the library's folders use, in bytes.
#[tauri::command]
#[specta::specta]
pub async fn library_storage(state: State<'_, AppState>) -> AppResult<StorageDto> {
    // Walking a large library takes a moment: keep it off the main thread.
    let library = state.library()?;
    let (books, notes, data) =
        tauri::async_runtime::spawn_blocking(move || library.storage_usage())
            .await
            .map_err(|e| {
                crate::error::AppError::new(crate::error::AppErrorKind::Io, e.to_string())
            })?;
    Ok(StorageDto {
        books: books as f64,
        notes: notes as f64,
        data: data as f64,
    })
}
