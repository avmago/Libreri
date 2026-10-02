//! Natural voices for reading aloud (ADR 0030): Kokoro and Piper voices
//! downloaded in Settings › Reader, switched on or off, deleted, and
//! spoken with on this computer.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use base64::Engine as _;
use libreri_voices::{KokoroInfo, NaturalVoice, PiperLanguage, PiperList};
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

fn lock<T>(m: &std::sync::Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A downloaded voice, and whether it is switched on.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NaturalVoiceDto {
    #[serde(flatten)]
    pub voice: NaturalVoice,
    pub on: bool,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NaturalVoicesDto {
    /// Every downloaded voice.
    pub voices: Vec<NaturalVoiceDto>,
    pub kokoro: KokoroInfo,
    /// Downloads switched off ("kokoro", "piper:<voice>").
    pub off: Vec<String>,
    /// eSpeak NG is installed (it turns words into sounds for the voices).
    pub espeak: bool,
    /// What is downloading.
    pub downloading: Option<String>,
}

fn dto(state: &AppState) -> NaturalVoicesDto {
    let off = lock(&state.settings).voices_off.clone();
    let voices = state
        .voices
        .voices()
        .into_iter()
        .map(|v| NaturalVoiceDto {
            on: !off.contains(&v.pack),
            voice: v,
        })
        .collect();
    NaturalVoicesDto {
        voices,
        kokoro: state.voices.kokoro_info(),
        off,
        espeak: libreri_helpers::find_program("espeak-ng").is_some(),
        downloading: lock(&state.voice_download)
            .as_ref()
            .map(|(id, _)| id.clone()),
    }
}

#[tauri::command]
#[specta::specta]
pub fn natural_voices(state: State<'_, AppState>) -> NaturalVoicesDto {
    dto(&state)
}

/// The languages of the Piper collection (fetched once a week).
#[tauri::command]
#[specta::specta]
pub async fn piper_languages(app: AppHandle) -> AppResult<Vec<PiperLanguage>> {
    blocking(move || {
        app.state::<AppState>()
            .voices
            .piper_languages()
            .map_err(AppError::invalid)
    })
    .await
}

/// The Piper voices of a language ("de_DE") free to use.
#[tauri::command]
#[specta::specta]
pub async fn piper_voices(app: AppHandle, code: String) -> AppResult<PiperList> {
    blocking(move || {
        app.state::<AppState>()
            .voices
            .piper_voices(&code)
            .map_err(AppError::invalid)
    })
    .await
}

/// Downloads "kokoro" or "piper:<voice>"; progress arrives as
/// `VoiceDownload`.
#[tauri::command]
#[specta::specta]
pub async fn download_voice(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<NaturalVoicesDto> {
    use crate::events::VoiceDownload;
    use tauri_specta::Event;
    let cancel: Arc<AtomicBool> = Arc::default();
    {
        let mut running = lock(&state.voice_download);
        if running.is_some() {
            return Err(AppError::invalid("a voice is already downloading"));
        }
        *running = Some((id.clone(), Arc::clone(&cancel)));
    }
    let voices = Arc::clone(&state.voices);
    let result = blocking(move || {
        let mut last = std::time::Instant::now();
        let handle = app.clone();
        let event_id = id.clone();
        let result = voices.download(&id, &cancel, move |done, total| {
            if last.elapsed().as_millis() > 250 {
                last = std::time::Instant::now();
                let _ = VoiceDownload {
                    id: event_id.clone(),
                    done: done as f64,
                    total: total as f64,
                    finished: false,
                    error: None,
                }
                .emit(&handle);
            }
        });
        let _ = VoiceDownload {
            id,
            done: 0.0,
            total: 0.0,
            finished: true,
            error: result.as_ref().err().cloned(),
        }
        .emit(&app);
        result.map_err(AppError::invalid)
    })
    .await;
    *lock(&state.voice_download) = None;
    result?;
    Ok(dto(&state))
}

#[tauri::command]
#[specta::specta]
pub fn cancel_voice_download(state: State<'_, AppState>) {
    if let Some((_, c)) = lock(&state.voice_download).as_ref() {
        c.store(true, Ordering::SeqCst);
    }
}

/// Deletes a download ("kokoro", "piper:<voice>").
#[tauri::command]
#[specta::specta]
pub async fn remove_voice(app: AppHandle, id: String) -> AppResult<NaturalVoicesDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        if lock(&state.voice_download)
            .as_ref()
            .is_some_and(|(d, _)| *d == id)
        {
            return Err(AppError::invalid("stop the download first"));
        }
        state.voices.remove(&id).map_err(AppError::invalid)?;
        if lock(&state.settings).voices_off.contains(&id) {
            state.update_settings(|s| s.voices_off.retain(|v| *v != id))?;
        }
        Ok(dto(&state))
    })
    .await
}

/// Switches a download's voices on or off.
#[tauri::command]
#[specta::specta]
pub fn set_voice_on(
    state: State<'_, AppState>,
    id: String,
    on: bool,
) -> AppResult<NaturalVoicesDto> {
    state.update_settings(|s| {
        s.voices_off.retain(|v| *v != id);
        if !on {
            s.voices_off.push(id.clone());
        }
    })?;
    if !on {
        state.voices.unload();
    }
    Ok(dto(&state))
}

/// Speaks `text` with a natural voice; a WAV file as base64.
#[tauri::command]
#[specta::specta]
pub async fn speak_natural(
    app: AppHandle,
    voice: String,
    text: String,
    speed: f64,
) -> AppResult<String> {
    blocking(move || {
        let state = app.state::<AppState>();
        let (samples, rate) = state
            .voices
            .speak(&voice, &text, speed as f32)
            .map_err(AppError::invalid)?;
        Ok(base64::engine::general_purpose::STANDARD.encode(libreri_voices::wav(&samples, rate)))
    })
    .await
}

/// Frees the memory of the voice last used (reading aloud stopped).
#[tauri::command]
#[specta::specta]
pub fn unload_voices(state: State<'_, AppState>) {
    state.voices.unload();
}
