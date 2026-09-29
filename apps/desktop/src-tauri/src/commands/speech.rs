//! Speech recognition (Phase 7b): whisper models, dictation, voice notes
//! and finding an audiobook's places in its text by listening.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::events::SpeechModelDownload;
use crate::state::AppState;
use base64::Engine;
use libreri_core::BookId;
use libreri_jobs::{JobContext, JobError};
use libreri_library::Library;
use libreri_speech::{audio, sync, ModelInfo, Transcriber};
use serde::Serialize;
use specta::Type;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

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

/// 16-bit little-endian mono PCM at 16 kHz, sent as base64.
fn pcm(b64: &str) -> AppResult<Vec<f32>> {
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .map_err(|_| AppError::invalid("the recording could not be read"))?;
    Ok(audio::from_i16_le(&bytes))
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpeechSettingsDto {
    pub models: Vec<ModelInfo>,
    /// The model in use: the one chosen, or the first downloaded.
    pub model: Option<String>,
    pub language: Option<String>,
    pub transcribe_notes: bool,
}

fn settings_dto(state: &AppState) -> SpeechSettingsDto {
    let s = state
        .settings
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .speech
        .clone();
    let models = libreri_speech::models(&state.whisper_dir);
    let model = s
        .model
        .filter(|m| models.iter().any(|x| &x.id == m && x.downloaded))
        .or_else(|| models.iter().find(|m| m.downloaded).map(|m| m.id.clone()));
    SpeechSettingsDto {
        models,
        model,
        language: s.language,
        transcribe_notes: s.transcribe_notes,
    }
}

#[tauri::command]
#[specta::specta]
pub fn speech_settings(state: State<'_, AppState>) -> SpeechSettingsDto {
    settings_dto(&state)
}

/// Chooses the model (only a downloaded one), the language spoken and
/// whether voice notes are written down.
#[tauri::command]
#[specta::specta]
pub fn set_speech_settings(
    state: State<'_, AppState>,
    model: Option<String>,
    language: Option<String>,
    transcribe_notes: bool,
) -> AppResult<SpeechSettingsDto> {
    if let Some(m) = &model {
        if libreri_speech::model_path(&state.whisper_dir, m).is_none() {
            return Err(AppError::invalid("download that model first"));
        }
    }
    let language = language
        .map(|l| l.trim().to_ascii_lowercase())
        .filter(|l| !l.is_empty() && l.len() <= 8 && l.chars().all(|c| c.is_ascii_alphabetic()));
    state.update_settings(|s| {
        s.speech.model = model;
        s.speech.language = language;
        s.speech.transcribe_notes = transcribe_notes;
    })?;
    Ok(settings_dto(&state))
}

/// Downloads a model; progress arrives as `SpeechModelDownload`. The
/// first model downloaded becomes the one used.
#[tauri::command]
#[specta::specta]
pub async fn download_speech_model(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> AppResult<SpeechSettingsDto> {
    let dir = state.whisper_dir.clone();
    let cancel: Arc<AtomicBool> = Arc::default();
    {
        let mut running = state
            .model_downloads
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if running.contains_key(&id) {
            return Err(AppError::invalid("that model is already downloading"));
        }
        running.insert(id.clone(), Arc::clone(&cancel));
    }
    let model = id.clone();
    let result = blocking(move || {
        let mut last = std::time::Instant::now();
        let result = libreri_speech::models::download(&dir, &model, &cancel, |done, total| {
            if last.elapsed().as_millis() > 250 {
                last = std::time::Instant::now();
                let _ = SpeechModelDownload {
                    id: model.clone(),
                    done: done as f64,
                    total: total.map(|t| t as f64),
                    finished: false,
                    error: None,
                }
                .emit(&app);
            }
        });
        let _ = SpeechModelDownload {
            id: model.clone(),
            done: 0.0,
            total: None,
            finished: true,
            error: result.as_ref().err().cloned(),
        }
        .emit(&app);
        result.map(|_| ()).map_err(AppError::invalid)
    })
    .await;
    state
        .model_downloads
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .remove(&id);
    result?;
    let chosen = state
        .settings
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .speech
        .model
        .clone();
    if chosen.is_none() {
        state.update_settings(|s| s.speech.model = Some(id))?;
    }
    Ok(settings_dto(&state))
}

#[tauri::command]
#[specta::specta]
pub fn cancel_speech_model_download(state: State<'_, AppState>, id: String) {
    if let Some(c) = state
        .model_downloads
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(&id)
    {
        c.store(true, Ordering::SeqCst);
    }
}

/// Removes a downloaded model. If it was the one used, another downloaded
/// model (if any) takes its place.
#[tauri::command]
#[specta::specta]
pub async fn remove_speech_model(app: AppHandle, id: String) -> AppResult<SpeechSettingsDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        state.drop_transcriber();
        libreri_speech::models::remove(&state.whisper_dir, &id).map_err(AppError::invalid)?;
        let chosen = state
            .settings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .speech
            .model
            .clone();
        if chosen.as_deref() == Some(id.as_str()) {
            let next = libreri_speech::models(&state.whisper_dir)
                .into_iter()
                .find(|m| m.downloaded)
                .map(|m| m.id);
            state.update_settings(|s| s.speech.model = next)?;
        }
        Ok(settings_dto(&state))
    })
    .await
}

fn language(state: &AppState, hint: Option<String>) -> Option<String> {
    state
        .settings
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .speech
        .language
        .clone()
        .or(hint)
}

/// Writes down a stretch of speech (dictation). `lang` is a hint (the
/// book's language) used when Settings do not name one.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_pcm(
    app: AppHandle,
    state: State<'_, AppState>,
    pcm_base64: String,
    lang: Option<String>,
) -> AppResult<String> {
    let samples = pcm(&pcm_base64)?;
    let lang = language(&state, lang);
    blocking(move || {
        let t = app.state::<AppState>().transcriber()?;
        let segs = t
            .transcribe(&samples, lang.as_deref(), None)
            .map_err(AppError::invalid)?;
        Ok(libreri_speech::join(&segs))
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VoiceNoteDto {
    /// The recording, in the library (`Notes/<profile>/Voice notes/…`).
    pub path: String,
    /// Seconds.
    pub duration: f64,
}

/// Saves a recording as FLAC in the profile's voice notes.
#[tauri::command]
#[specta::specta]
pub async fn save_voice_note(
    state: State<'_, AppState>,
    pcm_base64: String,
) -> AppResult<VoiceNoteDto> {
    let library = state.library()?;
    let samples = pcm(&pcm_base64)?;
    blocking(move || {
        if samples.len() < audio::RATE as usize / 2 {
            return Err(AppError::invalid("the recording is too short"));
        }
        let duration = samples.len() as f64 / f64::from(audio::RATE);
        let flac = audio::to_flac(&samples, audio::RATE).map_err(AppError::invalid)?;
        let path = library.save_voice(&flac, "flac")?;
        Ok(VoiceNoteDto { path, duration })
    })
    .await
}

/// Writes down what is said in a saved recording.
#[tauri::command]
#[specta::specta]
pub async fn transcribe_voice_note(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    lang: Option<String>,
) -> AppResult<String> {
    let library = state.library()?;
    let file = library.voice_file(&path)?;
    let lang = language(&state, lang);
    blocking(move || {
        let t = app.state::<AppState>().transcriber()?;
        let (samples, _) = audio::decode(&file, 0.0, None).map_err(AppError::invalid)?;
        let segs = t
            .transcribe(&samples, lang.as_deref(), None)
            .map_err(AppError::invalid)?;
        Ok(libreri_speech::join(&segs))
    })
    .await
}

/// Starts finding sync points by listening; ends with `AutoSyncFinished`.
/// Returns the job id.
#[tauri::command]
#[specta::specta]
pub fn auto_sync_audiobook(state: State<'_, AppState>, id: String) -> AppResult<String> {
    Ok(state.start_auto_sync(book_id(&id)?)?.to_string())
}

/// Seconds of audio listened to at each place.
const STRETCH: f64 = 24.0;

/// The work of `auto_sync_audiobook`: listen to stretches spread over the
/// audiobook, find each in the text, keep the ones that agree.
pub(crate) fn auto_sync(
    library: &Library,
    transcriber: &Transcriber,
    language: Option<&str>,
    audio_id: &BookId,
    text_id: &BookId,
    ctx: &JobContext,
    report: &mut crate::events::AutoSyncFinished,
) -> Result<(), JobError> {
    let failed = |e: libreri_library::Error| JobError::Failed(e.to_string());
    let path = library.audio_file(audio_id).map_err(failed)?;
    let duration = library
        .audio_info(audio_id)
        .ok()
        .and_then(|i| i.duration)
        .or_else(|| audio::duration(&path))
        .ok_or_else(|| JobError::Failed("the audiobook's length is unknown".into()))?;
    let lang = language
        .map(str::to_owned)
        .or_else(|| library.book(text_id).ok().and_then(|b| b.metadata.language));
    ctx.progress(0, 1, Some("Reading the book's text".into()));
    let placed = library.sync_words(text_id).map_err(failed)?;
    let mut map = Vec::with_capacity(placed.len());
    let mut normed = Vec::with_capacity(placed.len());
    for (k, w) in placed.iter().enumerate() {
        let n = sync::norm(&w.word);
        if !n.is_empty() {
            normed.push(n);
            map.push(k);
        }
    }
    if normed.len() < 50 {
        return Err(JobError::Failed(
            "the linked book has too little text to match (scans need OCR first)".into(),
        ));
    }
    let index = sync::TextIndex::new(normed);
    // About one place every five minutes, at least six, at most forty.
    let count = ((duration / 300.0).round() as usize).clamp(6, 40);
    let starts = sync::plan(duration, count, STRETCH);
    let total = starts.len() as u64;
    let mut heard = Vec::new();
    for (k, start) in starts.iter().enumerate() {
        ctx.check_cancelled()?;
        ctx.progress(
            k as u64,
            total,
            Some(format!("Listening at {}", clock(*start))),
        );
        let Ok((samples, at)) = audio::decode(&path, *start, Some(STRETCH)) else {
            continue;
        };
        let segments = transcriber
            .transcribe(&samples, lang.as_deref(), None)
            .map_err(JobError::Failed)?;
        report.tried += 1;
        heard.push(sync::Heard {
            start: at,
            segments,
        });
    }
    ctx.check_cancelled()?;
    let anchors = sync::anchors(&index, &heard);
    let points: Vec<_> = anchors
        .iter()
        .filter_map(|a| map.get(a.word).map(|&k| placed[k].sync_point(a.t)))
        .collect();
    report.found = points.len() as u32;
    if !points.is_empty() {
        library.add_auto_points(audio_id, points).map_err(failed)?;
    }
    ctx.progress(total, total, None);
    Ok(())
}

fn clock(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}
