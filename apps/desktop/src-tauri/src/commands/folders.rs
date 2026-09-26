//! Folders under `Books/` and importing into them.

use crate::dto::{FolderDto, ImportModeDto};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use libreri_library::ImportRequest;
use std::path::PathBuf;
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

#[tauri::command]
#[specta::specta]
pub fn list_folders(state: State<'_, AppState>) -> AppResult<Vec<FolderDto>> {
    Ok(state.library()?.folders()?.iter().map(Into::into).collect())
}

/// Creates `name` inside `parent`; returns the new folder's path.
#[tauri::command]
#[specta::specta]
pub fn create_folder(
    state: State<'_, AppState>,
    parent: String,
    name: String,
) -> AppResult<String> {
    Ok(state.library()?.create_folder(&parent, &name)?)
}

#[tauri::command]
#[specta::specta]
pub fn rename_folder(state: State<'_, AppState>, path: String, name: String) -> AppResult<String> {
    Ok(state.library()?.rename_folder(&path, &name)?)
}

#[tauri::command]
#[specta::specta]
pub fn move_folder(state: State<'_, AppState>, path: String, parent: String) -> AppResult<String> {
    Ok(state.library()?.move_folder(&path, &parent)?)
}

/// Moves a folder and its books to the system trash; returns the number of
/// books removed.
#[tauri::command]
#[specta::specta]
pub fn trash_folder(state: State<'_, AppState>, path: String) -> AppResult<u32> {
    Ok(state.library()?.trash_folder(&path)? as u32)
}

#[tauri::command]
#[specta::specta]
pub fn reveal_folder(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<()> {
    let library = state.library()?;
    let rel = if path.is_empty() {
        "Books".to_owned()
    } else {
        format!("Books/{path}")
    };
    let abs = library
        .layout()
        .resolve_relative(&rel)
        .ok_or_else(|| AppError::invalid("that folder is outside the library"))?;
    app.opener()
        .open_path(abs.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::new(crate::error::AppErrorKind::Io, e.to_string()))
}

/// Starts importing files and folders into `folder`. Returns the job id;
/// progress arrives as job events and the summary as `ImportFinished`.
#[tauri::command]
#[specta::specta]
pub fn import_paths(
    state: State<'_, AppState>,
    paths: Vec<String>,
    folder: String,
    mode: ImportModeDto,
) -> AppResult<String> {
    if paths.is_empty() {
        return Err(AppError::invalid("nothing to import"));
    }
    let request = ImportRequest {
        sources: paths.into_iter().map(PathBuf::from).collect(),
        folder,
        mode: mode.into(),
    };
    Ok(state.start_import(request)?.to_string())
}
