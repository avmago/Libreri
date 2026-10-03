//! Listening: audiobook chapters, links between audiobooks and
//! their text with sync points, and speaking with eSpeak NG where the
//! system has no voices.

use crate::dto::BookDto;
use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use libreri_core::BookId;
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::State;

fn book_id(id: &str) -> AppResult<BookId> {
    BookId::from_hex(id).map_err(|e| AppError::invalid(e.to_string()))
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDto {
    pub title: String,
    pub start: f64,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfoDto {
    pub duration: Option<f64>,
    pub chapters: Vec<ChapterDto>,
}

/// Length and chapters of an audiobook.
#[tauri::command]
#[specta::specta]
pub async fn audio_info(state: State<'_, AppState>, id: String) -> AppResult<AudioInfoDto> {
    let library = state.library()?;
    let id = book_id(&id)?;
    blocking(move || {
        let info = library.audio_info(&id)?;
        Ok(AudioInfoDto {
            duration: info.duration,
            chapters: info
                .chapters
                .into_iter()
                .map(|c| ChapterDto {
                    title: c.title,
                    start: c.start,
                })
                .collect(),
        })
    })
    .await
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncPointDto {
    pub t: f64,
    pub locator: String,
    pub progress: f64,
    pub label: Option<String>,
    #[serde(default)]
    pub auto: bool,
}

impl From<libreri_library::SyncPoint> for SyncPointDto {
    fn from(p: libreri_library::SyncPoint) -> Self {
        Self {
            t: p.t,
            locator: p.locator,
            progress: p.progress,
            label: p.label,
            auto: p.auto,
        }
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioLinkDto {
    /// The book the audiobook reads.
    pub text: Option<BookDto>,
    pub points: Vec<SyncPointDto>,
}

fn link_dto(library: &libreri_library::Library, link: libreri_library::AudioLink) -> AudioLinkDto {
    AudioLinkDto {
        text: link
            .text
            .and_then(|id| library.book(&id).ok())
            .map(Into::into),
        points: link.points.into_iter().map(Into::into).collect(),
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_audio_link(state: State<'_, AppState>, id: String) -> AppResult<AudioLinkDto> {
    let library = state.library()?;
    let link = library.audio_link(&book_id(&id)?)?;
    Ok(link_dto(&library, link))
}

/// Links an audiobook to its book (or unlinks it with `None`).
#[tauri::command]
#[specta::specta]
pub fn set_audio_link(
    state: State<'_, AppState>,
    id: String,
    text: Option<String>,
) -> AppResult<AudioLinkDto> {
    let library = state.library()?;
    let text = text.as_deref().map(book_id).transpose()?;
    let link = library.set_audio_link(&book_id(&id)?, text.as_ref())?;
    Ok(link_dto(&library, link))
}

#[tauri::command]
#[specta::specta]
pub fn set_sync_points(
    state: State<'_, AppState>,
    id: String,
    points: Vec<SyncPointDto>,
) -> AppResult<AudioLinkDto> {
    let library = state.library()?;
    let points = points
        .into_iter()
        .map(|p| libreri_library::SyncPoint {
            t: p.t,
            locator: p.locator,
            progress: p.progress,
            label: p.label,
            auto: p.auto,
        })
        .collect();
    let link = library.set_sync_points(&book_id(&id)?, points)?;
    Ok(link_dto(&library, link))
}

/// Audiobooks linked to a book.
#[tauri::command]
#[specta::specta]
pub fn audiobooks_for(state: State<'_, AppState>, id: String) -> AppResult<Vec<BookDto>> {
    let library = state.library()?;
    Ok(library
        .audiobooks_for(&book_id(&id)?)?
        .into_iter()
        .filter_map(|id| library.book(&id).ok())
        .map(Into::into)
        .collect())
}

/// eSpeak NG voices (none when it is not installed).
#[tauri::command]
#[specta::specta]
pub async fn system_voices() -> AppResult<Vec<libreri_helpers::speech::Voice>> {
    blocking(|| Ok(libreri_helpers::speech::voices())).await
}

/// Speaks `text` with eSpeak NG; returns when done (true) or stopped
/// (false).
#[tauri::command]
#[specta::specta]
pub async fn system_speak(
    state: State<'_, AppState>,
    text: String,
    voice: Option<String>,
    rate: f64,
) -> AppResult<bool> {
    let speaker = state.speaker.clone();
    blocking(move || {
        speaker
            .speak(&text, voice.as_deref(), rate)
            .map_err(AppError::invalid)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub fn system_stop_speaking(state: State<'_, AppState>) {
    state.speaker.stop();
}
