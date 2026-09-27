//! Book details found online: looking up, saving picked details, cover
//! previews, filling in many books, and the sources settings.

use crate::dto::BookDto;
use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use base64::Engine;
use libreri_core::{BookId, BookMetadata};
use libreri_metadata::{Lookup, Query, Source};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::Arc;
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

/// What a book would be looked up by (its identifiers, or ones found in
/// its pages), to fill the search form.
#[tauri::command]
#[specta::specta]
pub async fn details_query(state: State<'_, AppState>, id: String) -> AppResult<Query> {
    let library = state.library()?;
    let id = book_id(&id)?;
    blocking(move || Ok(library.details_query(&id)?)).await
}

/// Asks the online sources about a book. `query` is the reader's own
/// search; without it the book's details are used.
#[tauri::command]
#[specta::specta]
pub async fn find_details(
    state: State<'_, AppState>,
    id: String,
    query: Option<Query>,
) -> AppResult<Lookup> {
    let library = state.library()?;
    let id = book_id(&id)?;
    let online = state.online_settings();
    let http = Arc::clone(&state.http);
    blocking(move || Ok(library.find_details(&id, query, &online, &*http)?)).await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppliedDetails {
    pub book: BookDto,
    /// Why the picked cover could not be used, if it could not.
    pub cover_error: Option<String>,
}

/// Saves the details picked in the merge screen, and the picked cover.
#[tauri::command]
#[specta::specta]
pub async fn apply_details(
    state: State<'_, AppState>,
    id: String,
    metadata: BookMetadata,
    cover_url: Option<String>,
) -> AppResult<AppliedDetails> {
    let library = state.library()?;
    let id = book_id(&id)?;
    let http = Arc::clone(&state.http);
    blocking(move || {
        let (book, cover_error) =
            library.apply_details(&id, metadata, cover_url.as_deref(), &*http)?;
        Ok(AppliedDetails {
            book: book.into(),
            cover_error,
        })
    })
    .await
}

/// A cover from a source as a data: URL, so the interface can show it
/// without loading images from the internet itself.
#[tauri::command]
#[specta::specta]
pub async fn cover_preview(state: State<'_, AppState>, url: String) -> AppResult<String> {
    state.library()?;
    let http = Arc::clone(&state.http);
    blocking(move || {
        let cover = libreri_metadata::fetch_cover(&*http, &url).map_err(AppError::invalid)?;
        let mime = match cover.extension {
            "png" => "image/png",
            "webp" => "image/webp",
            "gif" => "image/gif",
            _ => "image/jpeg",
        };
        Ok(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(&cover.bytes)
        ))
    })
    .await
}

/// Fills in missing details of many books in the background. Returns the
/// job id; the result arrives as a `DetailsFilled` event.
#[tauri::command]
#[specta::specta]
pub fn fill_missing_details(state: State<'_, AppState>, ids: Vec<String>) -> AppResult<String> {
    let ids = ids
        .iter()
        .map(|i| book_id(i))
        .collect::<AppResult<Vec<_>>>()?;
    state.library()?.require_edit()?;
    Ok(state.start_fill_details(ids)?.to_string())
}

/// One source in Settings › Online details.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceInfo {
    pub source: Source,
    pub name: String,
    pub enabled: bool,
    pub needs_key: bool,
    /// The end of the saved key ("…a1b2"); the key itself never leaves Rust.
    pub key_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OnlineSettingsDto {
    pub sources: Vec<SourceInfo>,
    pub fill_on_import: bool,
}

fn dto(s: &libreri_metadata::Settings) -> OnlineSettingsDto {
    let hint = |k: &Option<String>| {
        k.as_deref().filter(|k| !k.trim().is_empty()).map(|k| {
            let tail: String = k
                .chars()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("…{tail}")
        })
    };
    OnlineSettingsDto {
        sources: Source::ALL
            .into_iter()
            .map(|src| SourceInfo {
                source: src,
                name: src.name().to_owned(),
                enabled: s.enabled.contains(&src),
                needs_key: src.needs_key(),
                key_hint: match src {
                    Source::ComicVine => hint(&s.comicvine_key),
                    Source::Isbndb => hint(&s.isbndb_key),
                    _ => None,
                },
            })
            .collect(),
        fill_on_import: s.fill_on_import,
    }
}

#[tauri::command]
#[specta::specta]
pub fn get_online_settings(state: State<'_, AppState>) -> OnlineSettingsDto {
    dto(&state.online_settings())
}

#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OnlineChange {
    pub enabled: Option<Vec<Source>>,
    pub fill_on_import: Option<bool>,
}

#[tauri::command]
#[specta::specta]
pub fn set_online_settings(
    state: State<'_, AppState>,
    change: OnlineChange,
) -> AppResult<OnlineSettingsDto> {
    state.library()?.require_edit()?;
    let s = state.update_online(|s| {
        if let Some(mut e) = change.enabled {
            e.sort();
            e.dedup();
            s.enabled = e;
        }
        if let Some(f) = change.fill_on_import {
            s.fill_on_import = f;
        }
    })?;
    Ok(dto(&s))
}

/// Saves (or, with `None`, removes) the reader's key for a source. Saving
/// a key also turns the source on.
#[tauri::command]
#[specta::specta]
pub fn set_source_key(
    state: State<'_, AppState>,
    source: Source,
    key: Option<String>,
) -> AppResult<OnlineSettingsDto> {
    state.library()?.require_edit()?;
    if !source.needs_key() {
        return Err(AppError::invalid(format!("{} needs no key", source.name())));
    }
    let key = key.map(|k| k.trim().to_owned()).filter(|k| !k.is_empty());
    if key
        .as_ref()
        .is_some_and(|k| k.len() > 200 || k.chars().any(char::is_whitespace))
    {
        return Err(AppError::invalid("that does not look like an API key"));
    }
    let s = state.update_online(|s| {
        match source {
            Source::ComicVine => s.comicvine_key = key.clone(),
            _ => s.isbndb_key = key.clone(),
        }
        if key.is_some() && !s.enabled.contains(&source) {
            s.enabled.push(source);
        }
        if key.is_none() {
            s.enabled.retain(|x| *x != source);
        }
    })?;
    Ok(dto(&s))
}
