//! Organising: bulk edit, tags and categories.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use libreri_core::{BookId, BulkEdit};
use serde::Serialize;
use specta::Type;
use tauri::State;

fn book_ids(ids: &[String]) -> AppResult<Vec<BookId>> {
    ids.iter()
        .map(|id| BookId::from_hex(id).map_err(|e| AppError::invalid(e.to_string())))
        .collect()
}

/// Applies one edit to many books. Returns how many changed.
#[tauri::command]
#[specta::specta]
pub fn bulk_edit_books(
    state: State<'_, AppState>,
    ids: Vec<String>,
    edit: BulkEdit,
) -> AppResult<u32> {
    Ok(state.library()?.bulk_edit(&book_ids(&ids)?, &edit)? as u32)
}

#[tauri::command]
#[specta::specta]
pub fn rename_tag(state: State<'_, AppState>, from: String, to: String) -> AppResult<u32> {
    Ok(state.library()?.rename_tag(&from, &to)? as u32)
}

#[tauri::command]
#[specta::specta]
pub fn merge_tags(
    state: State<'_, AppState>,
    sources: Vec<String>,
    into: String,
) -> AppResult<u32> {
    Ok(state.library()?.merge_tags(&sources, &into)? as u32)
}

#[tauri::command]
#[specta::specta]
pub fn delete_tag(state: State<'_, AppState>, tag: String) -> AppResult<u32> {
    Ok(state.library()?.delete_tag(&tag)? as u32)
}

#[tauri::command]
#[specta::specta]
pub fn rename_category(state: State<'_, AppState>, from: String, to: String) -> AppResult<u32> {
    Ok(state.library()?.rename_category(&from, &to)? as u32)
}

#[tauri::command]
#[specta::specta]
pub fn delete_category(state: State<'_, AppState>, path: String) -> AppResult<u32> {
    Ok(state.library()?.delete_category(&path)? as u32)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagCountDto {
    pub name: String,
    pub count: u32,
}

/// Groups of tags that look like the same thing.
#[tauri::command]
#[specta::specta]
pub fn similar_tags(state: State<'_, AppState>) -> AppResult<Vec<Vec<TagCountDto>>> {
    Ok(state
        .library()?
        .similar_tags()?
        .into_iter()
        .map(|g| {
            g.into_iter()
                .map(|(name, count)| TagCountDto { name, count })
                .collect()
        })
        .collect())
}
