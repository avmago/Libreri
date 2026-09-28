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

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PageScaleDto {
    pub page: u32,
    pub unit: String,
    pub per_point: f64,
}

/// Pages of a PDF that say how to measure them (CAD and map exports).
#[tauri::command]
#[specta::specta]
pub async fn measure_scales(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<PageScaleDto>> {
    let library = state.library()?;
    let id = book_id(&id)?;
    tauri::async_runtime::spawn_blocking(move || {
        Ok(library
            .measure_scales(&id)?
            .into_iter()
            .map(|s| PageScaleDto {
                page: s.page,
                unit: s.unit,
                per_point: s.per_point,
            })
            .collect())
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))?
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PictureDto {
    /// A data URL, ready to place on a page.
    pub src: String,
    pub width: u32,
    pub height: u32,
}

/// Reads a picture file for the markup picture and signature tools, made
/// smaller when it is large.
#[tauri::command]
#[specta::specta]
pub async fn read_picture(path: String) -> AppResult<PictureDto> {
    tauri::async_runtime::spawn_blocking(move || {
        let meta = std::fs::metadata(&path).map_err(|e| AppError::invalid(e.to_string()))?;
        if meta.len() > 60 * 1024 * 1024 {
            return Err(AppError::invalid("the picture is too large"));
        }
        let bytes = std::fs::read(&path).map_err(|e| AppError::invalid(e.to_string()))?;
        let (out, mime, width, height) =
            libreri_thumbs::picture(&bytes, 1600).map_err(|e| AppError::invalid(e.to_string()))?;
        use base64::Engine;
        let b64 = base64::engine::general_purpose::STANDARD.encode(out);
        Ok(PictureDto {
            src: format!("data:{mime};base64,{b64}"),
            width,
            height,
        })
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))?
}

/// Saves a copy of the book with its markup drawn in, as a PDF at `dest`.
/// With `add_to_library`, the copy is also imported next to the book.
#[tauri::command]
#[specta::specta]
pub async fn export_marked_up(
    state: State<'_, AppState>,
    id: String,
    dest: String,
    pages: Vec<libreri_pdf_edit::DrawPage>,
    add_to_library: bool,
) -> AppResult<()> {
    let library = state.library()?;
    let cache = state.page_cache.clone();
    let book = book_id(&id)?;
    let folder = library.book(&book)?.rel_path;
    let dest_path = std::path::PathBuf::from(&dest);
    let out = dest_path.clone();
    tauri::async_runtime::spawn_blocking(move || {
        library.export_marked_up(&book, &pages, &out, &cache)
    })
    .await
    .map_err(|e| AppError::invalid(e.to_string()))??;
    if add_to_library {
        // Next to the book, in its folder under Books/.
        let folder = std::path::Path::new(&folder)
            .parent()
            .and_then(|p| p.strip_prefix("Books").ok())
            .map(|p| p.to_string_lossy().replace('\\', "/"))
            .unwrap_or_default();
        state.start_import(libreri_library::ImportRequest {
            sources: vec![dest_path],
            folder,
            mode: libreri_library::ImportMode::Copy,
        })?;
    }
    Ok(())
}
