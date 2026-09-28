//! Reading maths from pictures (Phase 8b, ADR 0025): off until turned on
//! in Settings, which downloads the model.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use base64::Engine;
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
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MathsSettingsDto {
    /// Turned on in Settings.
    pub on: bool,
    pub downloaded: bool,
    pub size_mb: u32,
    pub downloading: bool,
}

fn settings(state: &AppState) -> MathsSettingsDto {
    let status = libreri_maths::status(&state.maths_dir);
    MathsSettingsDto {
        on: state
            .settings
            .lock()
            .expect("settings lock")
            .maths_from_pictures,
        downloaded: status.downloaded,
        size_mb: status.size_mb,
        downloading: state.maths_download.lock().expect("maths lock").is_some(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn maths_settings(state: State<'_, AppState>) -> MathsSettingsDto {
    settings(&state)
}

/// Turns reading maths from pictures on or off (the model stays until
/// removed).
#[tauri::command]
#[specta::specta]
pub fn set_maths_from_pictures(
    state: State<'_, AppState>,
    on: bool,
) -> AppResult<MathsSettingsDto> {
    state.update_settings(|s| s.maths_from_pictures = on)?;
    if !on {
        *state.maths_reader.lock().expect("maths lock") = None;
    }
    Ok(settings(&state))
}

/// Downloads the model; progress arrives as `MathsDownload`.
#[tauri::command]
#[specta::specta]
pub async fn download_maths_model(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<MathsSettingsDto> {
    use crate::events::MathsDownload;
    use tauri_specta::Event;
    let cancel: Arc<AtomicBool> = Arc::default();
    {
        let mut running = state.maths_download.lock().expect("maths lock");
        if running.is_some() {
            return Err(AppError::invalid("the maths model is already downloading"));
        }
        *running = Some(Arc::clone(&cancel));
    }
    let dir = state.maths_dir.clone();
    let result = blocking(move || {
        let mut last = std::time::Instant::now();
        let handle = app.clone();
        let result = libreri_maths::download(&dir, &cancel, move |done, total| {
            if last.elapsed().as_millis() > 250 {
                last = std::time::Instant::now();
                let _ = MathsDownload {
                    done: done as f64,
                    total: total as f64,
                    finished: false,
                    error: None,
                }
                .emit(&handle);
            }
        });
        let _ = MathsDownload {
            done: 0.0,
            total: 0.0,
            finished: true,
            error: result.as_ref().err().cloned(),
        }
        .emit(&app);
        result.map_err(AppError::invalid)
    })
    .await;
    *state.maths_download.lock().expect("maths lock") = None;
    result?;
    Ok(settings(&state))
}

#[tauri::command]
#[specta::specta]
pub fn cancel_maths_download(state: State<'_, AppState>) {
    if let Some(c) = state.maths_download.lock().expect("maths lock").as_ref() {
        c.store(true, Ordering::SeqCst);
    }
}

/// Removes the model and turns the feature off.
#[tauri::command]
#[specta::specta]
pub fn remove_maths_model(state: State<'_, AppState>) -> AppResult<MathsSettingsDto> {
    *state.maths_reader.lock().expect("maths lock") = None;
    libreri_maths::remove(&state.maths_dir).map_err(AppError::invalid)?;
    state.update_settings(|s| s.maths_from_pictures = false)?;
    Ok(settings(&state))
}

/// Reads a picture of maths (PNG or JPEG, base64 or a data URL) as LaTeX.
#[tauri::command]
#[specta::specta]
pub async fn maths_from_picture(
    app: AppHandle,
    state: State<'_, AppState>,
    picture: String,
) -> AppResult<String> {
    let s = settings(&state);
    if !s.on || !s.downloaded {
        return Err(AppError::invalid(
            "reading maths from pictures is turned off (Settings › Writing)",
        ));
    }
    let data = picture
        .split_once(";base64,")
        .map_or(picture.as_str(), |(_, d)| d);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| AppError::invalid("the picture could not be read"))?;
    blocking(move || {
        let state = app.state::<AppState>();
        let reader = {
            let mut slot = state.maths_reader.lock().expect("maths lock");
            match slot.as_ref() {
                Some(r) => Arc::clone(r),
                None => {
                    let r = Arc::new(
                        libreri_maths::Reader::load(&state.maths_dir).map_err(AppError::invalid)?,
                    );
                    *slot = Some(Arc::clone(&r));
                    r
                }
            }
        };
        reader.read(&bytes).map_err(AppError::invalid)
    })
    .await
}
