//! OCR models (ADR 0029): downloaded on request, chosen in Settings ›
//! Helper programs, removed with one click. Tesseract stays the default.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use libreri_formats::ocr::PageReader;
use libreri_ocr::OcrModelInfo;
use serde::Serialize;
use specta::Type;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(crate::error::AppErrorKind::Io, e.to_string()))?
}

fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OcrEnginesDto {
    /// "tesseract" or a downloaded model's id.
    pub selected: String,
    pub tesseract: bool,
    pub models: Vec<OcrModelInfo>,
    /// The model being downloaded.
    pub downloading: Option<String>,
}

/// The engine that reads scanned pages: a downloaded model, or None for
/// Tesseract (also when the chosen model was removed).
pub fn chosen_model(state: &AppState) -> Option<String> {
    let id = lock(&state.settings).ocr_engine.clone()?;
    libreri_ocr::is_downloaded(&state.ocr_models_dir, &id).then_some(id)
}

fn dto(state: &AppState) -> OcrEnginesDto {
    OcrEnginesDto {
        selected: chosen_model(state).unwrap_or_else(|| "tesseract".into()),
        tesseract: libreri_helpers::find_program("tesseract").is_some(),
        models: libreri_ocr::models(&state.ocr_models_dir),
        downloading: lock(&state.ocr_download).as_ref().map(|(id, _)| id.clone()),
    }
}

/// The chosen model, loaded (once; kept while it stays chosen).
pub fn reader(state: &AppState) -> AppResult<Option<Arc<dyn PageReader>>> {
    let Some(id) = chosen_model(state) else {
        return Ok(None);
    };
    let mut slot = lock(&state.ocr_reader);
    if let Some((loaded, r)) = slot.as_ref() {
        if *loaded == id {
            return Ok(Some(Arc::clone(r)));
        }
    }
    *slot = None;
    let r = libreri_ocr::load(&state.ocr_models_dir, &id).map_err(AppError::invalid)?;
    *slot = Some((id, Arc::clone(&r)));
    Ok(Some(r))
}

#[tauri::command]
#[specta::specta]
pub fn ocr_engines(state: State<'_, AppState>) -> OcrEnginesDto {
    dto(&state)
}

/// Chooses what reads scanned pages: "tesseract" or a downloaded model.
#[tauri::command]
#[specta::specta]
pub fn set_ocr_engine(state: State<'_, AppState>, id: String) -> AppResult<OcrEnginesDto> {
    let engine = if id == "tesseract" {
        None
    } else if libreri_ocr::is_downloaded(&state.ocr_models_dir, &id) {
        Some(id)
    } else {
        return Err(AppError::invalid("download the model first"));
    };
    state.update_settings(|s| s.ocr_engine = engine)?;
    // A model that is no longer chosen gives its memory back.
    let keep = chosen_model(&state);
    let mut slot = lock(&state.ocr_reader);
    if slot
        .as_ref()
        .is_some_and(|(id, _)| Some(id) != keep.as_ref())
    {
        *slot = None;
    }
    drop(slot);
    Ok(dto(&state))
}

/// Downloads a model; progress arrives as `OcrModelDownload`.
#[tauri::command]
#[specta::specta]
pub async fn download_ocr_model(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<OcrEnginesDto> {
    use crate::events::OcrModelDownload;
    use tauri_specta::Event;
    let cancel: Arc<AtomicBool> = Arc::default();
    {
        let mut running = lock(&state.ocr_download);
        if running.is_some() {
            return Err(AppError::invalid("a model is already downloading"));
        }
        *running = Some((id.clone(), Arc::clone(&cancel)));
    }
    let dir = state.ocr_models_dir.clone();
    let model = id.clone();
    let result = blocking(move || {
        let mut last = std::time::Instant::now();
        let handle = app.clone();
        let event_id = model.clone();
        let result = libreri_ocr::download(&dir, &model, &cancel, move |done, total| {
            if last.elapsed().as_millis() > 250 {
                last = std::time::Instant::now();
                let _ = OcrModelDownload {
                    id: event_id.clone(),
                    done: done as f64,
                    total: total as f64,
                    finished: false,
                    error: None,
                }
                .emit(&handle);
            }
        });
        let _ = OcrModelDownload {
            id: model,
            done: 0.0,
            total: 0.0,
            finished: true,
            error: result.as_ref().err().cloned(),
        }
        .emit(&app);
        result.map_err(AppError::invalid)
    })
    .await;
    *lock(&state.ocr_download) = None;
    result?;
    Ok(dto(&state))
}

#[tauri::command]
#[specta::specta]
pub fn cancel_ocr_model_download(state: State<'_, AppState>) {
    if let Some((_, c)) = lock(&state.ocr_download).as_ref() {
        c.store(true, Ordering::SeqCst);
    }
}

/// Removes a model (and goes back to Tesseract if it was chosen). Text
/// already read with it stays.
#[tauri::command]
#[specta::specta]
pub async fn remove_ocr_model(app: AppHandle, id: String) -> AppResult<OcrEnginesDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        if lock(&state.ocr_download)
            .as_ref()
            .is_some_and(|(d, _)| *d == id)
        {
            return Err(AppError::invalid("stop the download first"));
        }
        {
            let mut slot = lock(&state.ocr_reader);
            if slot.as_ref().is_some_and(|(loaded, _)| *loaded == id) {
                *slot = None;
            }
        }
        libreri_ocr::remove(&state.ocr_models_dir, &id).map_err(AppError::invalid)?;
        if lock(&state.settings).ocr_engine.as_deref() == Some(id.as_str()) {
            state.update_settings(|s| s.ocr_engine = None)?;
        }
        Ok(dto(&state))
    })
    .await
}
