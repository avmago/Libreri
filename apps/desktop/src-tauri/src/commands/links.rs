//! Links to web pages, videos and files (Phase 8c, ADR 0026): details
//! fetched once when a link is added, offline copies of web pages, the
//! player for embedded videos, and video and audio files on this computer.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use base64::Engine;
use libreri_links::{Fetched, KnownVideo, LinkDetails};
use serde::Serialize;
use specta::Type;
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
pub struct LinkPreviewDto {
    pub details: LinkDetails,
    /// The picture as a data URL, to show before saving.
    pub picture: Option<String>,
    /// A web page whose readable part can be kept offline.
    pub can_copy: bool,
}

/// Fetches a link's details (once, when it is being added). Nothing is
/// saved until [`link_save`].
#[tauri::command]
#[specta::specta]
pub async fn link_fetch(app: AppHandle, url: String) -> AppResult<LinkPreviewDto> {
    blocking(move || {
        let fetched = libreri_links::fetch(&url).map_err(AppError::invalid)?;
        let picture = fetched.picture.as_ref().map(|(b, kind)| {
            format!(
                "data:{kind};base64,{}",
                base64::engine::general_purpose::STANDARD.encode(b)
            )
        });
        let dto = LinkPreviewDto {
            details: fetched.details.clone(),
            picture,
            can_copy: fetched.page.is_some(),
        };
        let state = app.state::<AppState>();
        let mut cache = state
            .fetched_links
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if cache.len() > 8 {
            cache.clear();
        }
        cache.insert(fetched.details.url.clone(), Arc::new(fetched));
        Ok(dto)
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkSavedDto {
    /// The picture, in the profile's notes folder.
    pub picture: Option<String>,
    /// The offline copy of the page, in the profile's notes folder.
    pub copy: Option<String>,
}

/// Keeps a fetched link's picture and, for a web page when asked, an
/// offline copy of it. `url` is the address [`link_fetch`] returned.
#[tauri::command]
#[specta::specta]
pub async fn link_save(
    app: AppHandle,
    url: String,
    title: String,
    keep_copy: bool,
) -> AppResult<LinkSavedDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        let library = state.library()?;
        let cached = state
            .fetched_links
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(&url)
            .cloned();
        let fetched: Arc<Fetched> = match cached {
            Some(f) => f,
            None => Arc::new(libreri_links::fetch(&url).map_err(AppError::invalid)?),
        };
        let picture = match &fetched.picture {
            Some((bytes, kind)) => library.save_link_picture(&title, bytes, kind).ok(),
            None => None,
        };
        let copy = match (&fetched.page, keep_copy) {
            (Some(page), true) => {
                let saved = chrono::Local::now().format("%-d %B %Y").to_string();
                let html = libreri_links::copy::make(
                    page,
                    &libreri_links::copy::Source {
                        url: &fetched.details.url,
                        title: Some(&title)
                            .filter(|t| !t.trim().is_empty())
                            .map(|t| t.as_str()),
                        site: &fetched.details.site,
                        saved: &saved,
                    },
                    libreri_links::copy::web_pictures(),
                )
                .map_err(AppError::invalid)?;
                Some(library.save_web_copy(&title, &html)?)
            }
            _ => None,
        };
        state
            .fetched_links
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&url);
        Ok(LinkSavedDto { picture, copy })
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkFileDto {
    /// The path in the library when the file is inside it (so the link
    /// travels with the library), otherwise the full path.
    pub file: String,
    pub name: String,
    /// "video" or "audio".
    pub media: String,
    pub in_library: bool,
}

const VIDEO: &[&str] = &["mp4", "m4v", "webm", "mov", "mkv", "ogv"];
const AUDIO: &[&str] = &["mp3", "m4a", "m4b", "aac", "ogg", "opus", "flac", "wav"];

fn media_kind(path: &std::path::Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    if VIDEO.contains(&ext.as_str()) {
        Some("video")
    } else if AUDIO.contains(&ext.as_str()) {
        Some("audio")
    } else {
        None
    }
}

/// A video or audio file chosen to link to.
#[tauri::command]
#[specta::specta]
pub async fn link_file(state: State<'_, AppState>, path: String) -> AppResult<LinkFileDto> {
    let p = std::path::PathBuf::from(&path);
    let media = media_kind(&p).ok_or_else(|| {
        AppError::invalid("choose a video or audio file (MP4, WebM, MOV, MP3, M4A, FLAC…)")
    })?;
    if !p.is_file() {
        return Err(AppError::invalid("that file cannot be found"));
    }
    let rel = state.library_if_open().and_then(|l| l.relative_path(&p));
    Ok(LinkFileDto {
        in_library: rel.is_some(),
        file: rel.unwrap_or(path),
        name: p
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        media: media.into(),
    })
}

/// Where a linked video or audio file plays from: a `book://` path
/// (`.media/<token>/<name>`), usable while the app runs.
#[tauri::command]
#[specta::specta]
pub async fn link_media_url(state: State<'_, AppState>, file: String) -> AppResult<String> {
    let path = if std::path::Path::new(&file).is_absolute() {
        std::path::PathBuf::from(&file)
    } else {
        let library = state.library()?;
        // Only files the signed-in profile may open.
        if !library.may_open(&file) {
            return Err(AppError::invalid("that file is not in the library"));
        }
        library
            .layout()
            .resolve_relative(&file)
            .ok_or_else(|| AppError::invalid("that file is not in the library"))?
    };
    if media_kind(&path).is_none() {
        return Err(AppError::invalid(
            "only video and audio files can be played",
        ));
    }
    if !path.is_file() {
        return Err(AppError::invalid(
            "the file cannot be found; it may have been moved or be on another computer",
        ));
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    let name: String = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    state
        .media_files
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(token.clone(), path);
    Ok(format!(".media/{token}/{name}"))
}

fn player(state: &AppState) -> AppResult<Arc<libreri_links::PlayerServer>> {
    let mut slot = state
        .player
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(p) = slot.as_ref() {
        return Ok(Arc::clone(p));
    }
    let p = Arc::new(
        libreri_links::PlayerServer::start()
            .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?,
    );
    *slot = Some(Arc::clone(&p));
    Ok(p)
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayerDto {
    /// The local page holding the site's player.
    pub page: String,
    /// The video's own page, for "Open in browser".
    pub watch: String,
}

/// The player for an embedded video, at a start time (seconds).
#[tauri::command]
#[specta::specta]
pub async fn link_player(
    state: State<'_, AppState>,
    url: String,
    video: Option<KnownVideo>,
    embed: Option<String>,
    start: Option<f64>,
) -> AppResult<PlayerDto> {
    let (src, watch) = match &video {
        Some(v) => (
            libreri_links::embed_url(v, start),
            libreri_links::watch_url(v, start),
        ),
        None => (
            embed.ok_or_else(|| AppError::invalid("this link has no player"))?,
            url,
        ),
    };
    if !src.starts_with("https://") {
        return Err(AppError::invalid("this link has no player"));
    }
    Ok(PlayerDto {
        page: player(&state)?.url_for(&src),
        watch,
    })
}

/// Opens a player in a window of its own.
#[tauri::command]
#[specta::specta]
pub async fn link_pop_out(app: AppHandle, page: String, title: String) -> AppResult<()> {
    // Async: building a window from a command on the main thread deadlocks
    // on Windows.
    let url: tauri::Url = page
        .parse()
        .map_err(|_| AppError::invalid("not a player address"))?;
    if url.host_str() != Some("127.0.0.1") {
        return Err(AppError::invalid("not a player address"));
    }
    let label = format!("player-{}", uuid::Uuid::new_v4().simple());
    tauri::WebviewWindowBuilder::new(&app, label, tauri::WebviewUrl::External(url))
        .title(if title.trim().is_empty() {
            "Video"
        } else {
            title.trim()
        })
        .inner_size(854.0, 480.0)
        .min_inner_size(320.0, 180.0)
        .build()
        .map(|_| ())
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))
}

/// Opens a linked file (an offline copy, or a video or recording) in the
/// app the system uses for it.
#[tauri::command]
#[specta::specta]
pub async fn link_open_file(
    app: AppHandle,
    state: State<'_, AppState>,
    file: String,
) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let path = if std::path::Path::new(&file).is_absolute() {
        let p = std::path::PathBuf::from(&file);
        if media_kind(&p).is_none() {
            return Err(AppError::invalid(
                "only linked videos and recordings open this way",
            ));
        }
        p
    } else {
        if !opens_linked(&file) {
            return Err(AppError::invalid(
                "only linked web pages, videos and recordings open this way",
            ));
        }
        let library = state.library()?;
        let p = library
            .layout()
            .resolve_relative(&file)
            .ok_or_else(|| AppError::invalid("that file is not in the library"))?;
        // Offline copies: only the signed-in profile's own; other files
        // only where the signed-in profile may look.
        if file.starts_with("Notes/") {
            library.own_note_file(&file)?
        } else if library.may_open(&file) {
            p
        } else {
            return Err(AppError::invalid("that file is not in the library"));
        }
    };
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))
}

/// Whether a file in the library is one a link may open with another app:
/// an offline copy of a web page, a PDF, or a video or audio file. Never a
/// program or script.
fn opens_linked(rel: &str) -> bool {
    let p = std::path::Path::new(rel);
    let ext = p
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    media_kind(p).is_some() || matches!(ext.as_str(), "html" | "htm" | "pdf")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_open_only_pages_documents_and_media() {
        assert!(opens_linked("Notes/p/Links/Web pages/Sea.html"));
        assert!(opens_linked("Books/Talks/Lecture.MP4"));
        assert!(opens_linked("Books/Paper.pdf"));
        assert!(!opens_linked("Books/tool.exe"));
        assert!(!opens_linked("Books/run.command"));
        assert!(!opens_linked("Books/no-extension"));
    }
}
