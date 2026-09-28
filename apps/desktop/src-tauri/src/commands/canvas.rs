//! Handwriting canvases (Phase 8a): Excalidraw files in the profile's
//! notes folder, and reading handwriting as text.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use base64::Engine;
use libreri_core::BookId;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, Manager, State};

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}

fn book_id(id: &str) -> AppResult<BookId> {
    BookId::from_hex(id).map_err(|e| AppError::invalid(e.to_string()))
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CanvasDto {
    pub title: String,
    pub rel_path: String,
    pub book_id: Option<String>,
    pub paper: String,
    pub modified: f64,
    pub elements: u32,
}

/// The profile's canvases, newest first (only a book's, when given).
#[tauri::command]
#[specta::specta]
pub async fn canvases(
    state: State<'_, AppState>,
    book: Option<String>,
) -> AppResult<Vec<CanvasDto>> {
    let library = state.library()?;
    let book = book.as_deref().map(book_id).transpose()?;
    blocking(move || {
        Ok(library
            .canvases(book.as_ref())?
            .into_iter()
            .map(|c| CanvasDto {
                title: c.title,
                rel_path: c.rel_path,
                book_id: c.book_id.map(|b| b.to_string()),
                paper: c.paper,
                modified: c.modified as f64,
                elements: c.elements,
            })
            .collect())
    })
    .await
}

/// Starts a canvas. Returns its path.
#[tauri::command]
#[specta::specta]
pub fn create_canvas(
    state: State<'_, AppState>,
    title: String,
    book: Option<String>,
    paper: String,
) -> AppResult<String> {
    let book = book.as_deref().map(book_id).transpose()?;
    Ok(state
        .library()?
        .create_canvas(&title, book.as_ref(), &paper)?)
}

#[tauri::command]
#[specta::specta]
pub async fn read_canvas(state: State<'_, AppState>, path: String) -> AppResult<String> {
    let library = state.library()?;
    blocking(move || Ok(library.read_canvas(&path)?)).await
}

#[tauri::command]
#[specta::specta]
pub async fn write_canvas(
    state: State<'_, AppState>,
    path: String,
    content: String,
) -> AppResult<()> {
    let library = state.library()?;
    blocking(move || Ok(library.write_canvas(&path, &content)?)).await
}

#[tauri::command]
#[specta::specta]
pub fn set_canvas_paper(state: State<'_, AppState>, path: String, paper: String) -> AppResult<()> {
    Ok(state.library()?.set_canvas_paper(&path, &paper)?)
}

/// Renames a canvas; returns its new path.
#[tauri::command]
#[specta::specta]
pub fn rename_canvas(state: State<'_, AppState>, path: String, title: String) -> AppResult<String> {
    Ok(state.library()?.rename_canvas(&path, &title)?)
}

#[tauri::command]
#[specta::specta]
pub fn delete_canvas(state: State<'_, AppState>, path: String) -> AppResult<()> {
    Ok(state.library()?.delete_canvas(&path)?)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct InkSettingsDto {
    /// "system" or "tesseract".
    pub engine: String,
    /// The system's recogniser, if this system has one.
    pub system: Option<String>,
    pub tesseract: bool,
}

fn ink_engine(state: &AppState) -> String {
    let chosen = state
        .settings
        .lock()
        .expect("settings lock")
        .ink_engine
        .clone();
    match chosen.as_deref() {
        Some("system") if libreri_helpers::ink::system_name().is_some() => "system".into(),
        Some("tesseract") => "tesseract".into(),
        _ if libreri_helpers::ink::system_name().is_some() => "system".into(),
        _ => "tesseract".into(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn ink_settings(state: State<'_, AppState>) -> InkSettingsDto {
    InkSettingsDto {
        engine: ink_engine(&state),
        system: libreri_helpers::ink::system_name().map(str::to_owned),
        tesseract: libreri_helpers::find_program("tesseract").is_some(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn set_ink_engine(state: State<'_, AppState>, engine: String) -> AppResult<InkSettingsDto> {
    if engine != "system" && engine != "tesseract" {
        return Err(AppError::invalid("unknown handwriting reader"));
    }
    state.update_settings(|s| s.ink_engine = Some(engine))?;
    Ok(ink_settings(state))
}

/// Reads handwriting: a PNG of the ink (base64), with the engine chosen in
/// Settings. `lang` is the book's language, when known.
#[tauri::command]
#[specta::specta]
pub async fn ink_to_text(
    app: AppHandle,
    state: State<'_, AppState>,
    png_base64: String,
    lang: Option<String>,
) -> AppResult<String> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(png_base64.trim_start_matches("data:image/png;base64,"))
        .map_err(|_| AppError::invalid("the drawing could not be read"))?;
    let engine = ink_engine(&state);
    blocking(move || {
        let dir = std::env::temp_dir().join("libreri-ink");
        std::fs::create_dir_all(&dir)?;
        let file = dir.join(format!("{}.png", uuid::Uuid::new_v4()));
        std::fs::write(&file, &bytes)?;
        let result = if engine == "system" {
            libreri_helpers::ink::recognize_system(&file, lang.as_deref())
        } else {
            let state = app.state::<AppState>();
            // The languages chosen for OCR in Settings › Helper programs.
            let languages = crate::commands::search::default_languages(&state);
            let tessdata = libreri_helpers::tessdata::prepare(&state.tessdata, &languages)
                .map_err(|e| e.to_string());
            tessdata.and_then(|t| {
                libreri_formats::ocr::recognize_block(&file, &languages, t.as_deref())
            })
        };
        let _ = std::fs::remove_file(&file);
        result.map_err(AppError::invalid)
    })
    .await
}

/// Extra fonts for canvases, and which are downloaded.
#[tauri::command]
#[specta::specta]
pub fn canvas_fonts(state: State<'_, AppState>) -> Vec<libreri_helpers::fonts::ExtraFont> {
    libreri_helpers::fonts::fonts(&state.extras_dir)
}

/// Downloads an extra canvas font; progress arrives as `CanvasFontDownload`.
#[tauri::command]
#[specta::specta]
pub async fn download_canvas_font(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<libreri_helpers::fonts::ExtraFont>> {
    use crate::events::CanvasFontDownload;
    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use tauri_specta::Event;
    let dir = state.extras_dir.clone();
    let cancel: Arc<AtomicBool> = Arc::default();
    {
        let mut running = state.font_downloads.lock().expect("downloads lock");
        if running.contains_key(&id) {
            return Err(AppError::invalid("that font is already downloading"));
        }
        running.insert(id.clone(), Arc::clone(&cancel));
    }
    let font = id.clone();
    let result = blocking(move || {
        let mut last = std::time::Instant::now();
        let handle = app.clone();
        let f = font.clone();
        let result = libreri_helpers::fonts::download(&dir, &font, &cancel, move |done, total| {
            if last.elapsed().as_millis() > 250 {
                last = std::time::Instant::now();
                let _ = CanvasFontDownload {
                    id: f.clone(),
                    done: done as f64,
                    total: total.map(|t| t as f64),
                    finished: false,
                    error: None,
                }
                .emit(&handle);
            }
        });
        let _ = CanvasFontDownload {
            id: font.clone(),
            done: 0.0,
            total: None,
            finished: true,
            error: result.as_ref().err().cloned(),
        }
        .emit(&app);
        result.map_err(AppError::invalid)
    })
    .await;
    state
        .font_downloads
        .lock()
        .expect("downloads lock")
        .remove(&id);
    result?;
    Ok(libreri_helpers::fonts::fonts(&state.extras_dir))
}

#[tauri::command]
#[specta::specta]
pub fn cancel_canvas_font_download(state: State<'_, AppState>, id: String) {
    if let Some(c) = state
        .font_downloads
        .lock()
        .expect("downloads lock")
        .get(&id)
    {
        c.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[tauri::command]
#[specta::specta]
pub fn remove_canvas_font(
    state: State<'_, AppState>,
    id: String,
) -> AppResult<Vec<libreri_helpers::fonts::ExtraFont>> {
    libreri_helpers::fonts::remove(&state.extras_dir, &id).map_err(AppError::invalid)?;
    Ok(libreri_helpers::fonts::fonts(&state.extras_dir))
}
