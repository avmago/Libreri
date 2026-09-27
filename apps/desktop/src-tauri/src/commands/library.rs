use crate::dto::{FolderKind, LibrarySummary};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use libreri_library::{Library, OpenOptions};
use std::path::{Path, PathBuf};
use tauri::State;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

/// Tells the Welcome screen what the chosen folder contains.
#[tauri::command]
#[specta::specta]
pub fn inspect_folder(path: String) -> FolderKind {
    let path = Path::new(&path);
    if !path.exists() {
        FolderKind::Missing
    } else if Library::is_library(path) {
        FolderKind::Library
    } else if std::fs::read_dir(path)
        .map(|mut d| d.next().is_none())
        .unwrap_or(false)
    {
        FolderKind::Empty
    } else {
        FolderKind::OtherFiles
    }
}

/// Creates a new library in `path` and opens it.
#[tauri::command]
#[specta::specta]
pub fn create_library(
    state: State<'_, AppState>,
    path: String,
    name: Option<String>,
) -> AppResult<LibrarySummary> {
    state.close_library();
    let library = Library::create(Path::new(&path), name.as_deref(), APP_VERSION)?;
    adopt(&state, library)
}

/// Opens an existing library. `force` takes over a lock left by another
/// computer, after the user has confirmed.
#[tauri::command]
#[specta::specta]
pub fn open_library(
    state: State<'_, AppState>,
    path: String,
    force: bool,
) -> AppResult<LibrarySummary> {
    state.close_library();
    let library = Library::open(Path::new(&path), OpenOptions { force })?;
    adopt(&state, library)
}

#[tauri::command]
#[specta::specta]
pub fn close_library(state: State<'_, AppState>) {
    state.close_library();
}

#[tauri::command]
#[specta::specta]
pub fn current_library(state: State<'_, AppState>) -> Option<LibrarySummary> {
    state.library_if_open().map(|l| LibrarySummary::of(&l))
}

/// Checks the library folder for changes now (the watcher does this
/// automatically; this is the manual "Refresh").
#[tauri::command]
#[specta::specta]
pub fn rescan_library(state: State<'_, AppState>) -> AppResult<Option<String>> {
    state.library()?;
    Ok(state.schedule_scan().map(|id| id.to_string()))
}

/// Rebuilds the database from the book files and JSON sidecars.
#[tauri::command]
#[specta::specta]
pub fn rebuild_library_index(state: State<'_, AppState>) -> AppResult<String> {
    Ok(state.start_rebuild()?.to_string())
}

/// Stops a running import or scan at the next file.
#[tauri::command]
#[specta::specta]
pub fn cancel_job(state: State<'_, AppState>, id: String) -> bool {
    id.parse().is_ok_and(|id| state.jobs.cancel(id))
}

/// Makes `library` the open one and remembers it on the Welcome screen.
pub(crate) fn adopt(state: &AppState, library: Library) -> Result<LibrarySummary, AppError> {
    let summary = LibrarySummary::of(&library);
    let root: PathBuf = library.layout().root().to_path_buf();
    let name = library.info().name.clone();
    state.adopt(library);
    state.update_settings(|s| s.remember_library(name, root))?;
    Ok(summary)
}
