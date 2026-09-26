//! Books: listing, details, editing, moving, trash, covers, opening.

use crate::dto::{BookDto, FacetsDto};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use base64::Engine;
use libreri_core::{BookId, BookMetadata, BookQuery, BookUserState};
use tauri::{AppHandle, State};
use tauri_plugin_opener::OpenerExt;

fn book_id(id: &str) -> AppResult<BookId> {
    BookId::from_hex(id).map_err(|e| AppError::invalid(e.to_string()))
}

fn book_ids(ids: &[String]) -> AppResult<Vec<BookId>> {
    ids.iter().map(|id| book_id(id)).collect()
}

#[tauri::command]
#[specta::specta]
pub fn list_books(state: State<'_, AppState>, query: BookQuery) -> AppResult<Vec<BookDto>> {
    let library = state.library()?;
    Ok(library.books(&query)?.into_iter().map(Into::into).collect())
}

#[tauri::command]
#[specta::specta]
pub fn get_book(state: State<'_, AppState>, id: String) -> AppResult<BookDto> {
    Ok(state.library()?.book(&book_id(&id)?)?.into())
}

#[tauri::command]
#[specta::specta]
pub fn library_facets(state: State<'_, AppState>) -> AppResult<FacetsDto> {
    Ok(FacetsDto::from(&state.library()?.facets()?))
}

/// Saves edited details. Refuses invalid ISBNs, an empty title, etc. with a
/// message that says what to fix.
#[tauri::command]
#[specta::specta]
pub fn update_book(
    state: State<'_, AppState>,
    id: String,
    metadata: BookMetadata,
) -> AppResult<BookDto> {
    Ok(state
        .library()?
        .update_metadata(&book_id(&id)?, metadata)?
        .into())
}

/// Saves the current profile's reading status, rating and favourite flag.
#[tauri::command]
#[specta::specta]
pub fn set_book_state(
    state: State<'_, AppState>,
    id: String,
    user: BookUserState,
) -> AppResult<BookDto> {
    Ok(state
        .library()?
        .set_user_state(&book_id(&id)?, &user)?
        .into())
}

/// Moves books into a folder (relative to `Books/`). Returns how many moved.
#[tauri::command]
#[specta::specta]
pub fn move_books(state: State<'_, AppState>, ids: Vec<String>, folder: String) -> AppResult<u32> {
    let moved = state.library()?.move_books(&book_ids(&ids)?, &folder)?;
    Ok(moved.len() as u32)
}

/// Moves books to the system trash. Returns how many were removed.
#[tauri::command]
#[specta::specta]
pub fn trash_books(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<u32> {
    Ok(state.library()?.trash_books(&book_ids(&ids)?)? as u32)
}

/// Stores a cover sent by the interface (base64 of a JPEG or PNG), such as
/// the first page of a PDF rendered with PDF.js.
#[tauri::command]
#[specta::specta]
pub fn save_cover(state: State<'_, AppState>, id: String, image_base64: String) -> AppResult<()> {
    let data = image_base64
        .split_once(',')
        .map(|(_, d)| d)
        .unwrap_or(&image_base64);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| AppError::invalid("the image data is damaged"))?;
    state.library()?.set_cover(&book_id(&id)?, &bytes)?;
    Ok(())
}

fn book_path(state: &AppState, id: &str) -> AppResult<std::path::PathBuf> {
    let library = state.library()?;
    let book = library.book(&book_id(id)?)?;
    library
        .layout()
        .resolve_relative(&book.rel_path)
        .ok_or_else(|| AppError::invalid("the book's path is not valid"))
}

/// Opens the book in the system's default app (until Libreri's own reader
/// arrives in Phase 2).
#[tauri::command]
#[specta::specta]
pub fn open_book_externally(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<()> {
    let path = book_path(&state, &id)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::new(crate::error::AppErrorKind::Io, e.to_string()))
}

/// Shows the book's file in Finder / Explorer / the file manager.
#[tauri::command]
#[specta::specta]
pub fn reveal_book(app: AppHandle, state: State<'_, AppState>, id: String) -> AppResult<()> {
    let path = book_path(&state, &id)?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| AppError::new(crate::error::AppErrorKind::Io, e.to_string()))
}
