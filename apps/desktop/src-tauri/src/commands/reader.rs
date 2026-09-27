//! Reading: positions, highlights and bookmarks, notebooks, open tabs.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use libreri_core::{Annotation, BookId};
use serde::Serialize;
use specta::Type;
use tauri::State;

fn book_id(id: &str) -> AppResult<BookId> {
    BookId::from_hex(id).map_err(|e| AppError::invalid(e.to_string()))
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NotebookDto {
    pub rel_path: String,
    pub content: String,
}

impl From<libreri_library::Notebook> for NotebookDto {
    fn from(n: libreri_library::Notebook) -> Self {
        Self {
            rel_path: n.rel_path,
            content: n.content,
        }
    }
}

/// Where the current profile stopped reading (JSON), if anywhere.
#[tauri::command]
#[specta::specta]
pub fn get_position(state: State<'_, AppState>, id: String) -> AppResult<Option<String>> {
    Ok(state.library()?.position(&book_id(&id)?)?)
}

#[tauri::command]
#[specta::specta]
pub fn save_position(
    state: State<'_, AppState>,
    id: String,
    locator: String,
    progress: f32,
) -> AppResult<()> {
    Ok(state
        .library()?
        .save_position(&book_id(&id)?, &locator, progress)?)
}

#[tauri::command]
#[specta::specta]
pub fn list_annotations(state: State<'_, AppState>, id: String) -> AppResult<Vec<Annotation>> {
    Ok(state.library()?.annotations(&book_id(&id)?)?)
}

/// Adds or updates a highlight or bookmark; returns it with timestamps set.
#[tauri::command]
#[specta::specta]
pub fn save_annotation(
    state: State<'_, AppState>,
    annotation: Annotation,
) -> AppResult<Annotation> {
    Ok(state.library()?.save_annotation(annotation)?)
}

#[tauri::command]
#[specta::specta]
pub fn delete_annotation(state: State<'_, AppState>, id: String) -> AppResult<()> {
    Ok(state.library()?.delete_annotation(&id)?)
}

/// The Markdown notebook for a book, created on first use.
#[tauri::command]
#[specta::specta]
pub fn get_notebook(state: State<'_, AppState>, id: String) -> AppResult<NotebookDto> {
    Ok(state.library()?.notebook(&book_id(&id)?)?.into())
}

#[tauri::command]
#[specta::specta]
pub fn save_notebook(
    state: State<'_, AppState>,
    id: String,
    content: String,
) -> AppResult<NotebookDto> {
    Ok(state
        .library()?
        .save_notebook(&book_id(&id)?, &content)?
        .into())
}

/// The reader tabs that were open last time (JSON written by the interface).
#[tauri::command]
#[specta::specta]
pub fn get_session(state: State<'_, AppState>) -> AppResult<Option<String>> {
    Ok(state.library()?.session()?)
}

#[tauri::command]
#[specta::specta]
pub fn save_session(state: State<'_, AppState>, session: String) -> AppResult<()> {
    Ok(state.library()?.save_session(&session)?)
}
