//! Searching inside books, the search index, and OCR ("Make searchable")
//! with its language files.

use crate::dto::BookDto;
use crate::error::{AppError, AppErrorKind, AppResult};
use crate::events::OcrLanguageDownload;
use crate::state::AppState;
use libreri_core::BookId;
use libreri_helpers::tessdata::{self, OcrLanguage};
use libreri_library::{Hit, OcrOptions, TextState};
use libreri_search::IndexCounts;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use tauri_specta::Event;

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

/// One book whose words match.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TextMatchDto {
    pub book: BookDto,
    /// Pages or passages that match.
    pub total: u32,
    /// The best few, in reading order.
    pub hits: Vec<Hit>,
}

/// Books whose text matches `query` (words, "a phrase", -left-out).
#[tauri::command]
#[specta::specta]
pub async fn search_text(
    state: State<'_, AppState>,
    query: String,
    limit: u32,
) -> AppResult<Vec<TextMatchDto>> {
    let library = state.library()?;
    let index = state.search_index()?;
    blocking(move || {
        Ok(library
            .search_text(&index, &query, limit.clamp(1, 500) as usize)?
            .into_iter()
            .map(|r| TextMatchDto {
                book: BookDto::from(r.book),
                total: r.total,
                hits: r.hits,
            })
            .collect())
    })
    .await
}

/// Every match in one book, in reading order.
#[tauri::command]
#[specta::specta]
pub async fn search_in_book(
    state: State<'_, AppState>,
    id: String,
    query: String,
    limit: u32,
) -> AppResult<Vec<Hit>> {
    let library = state.library()?;
    let index = state.search_index()?;
    let id = book_id(&id)?;
    blocking(move || {
        Ok(library.search_in_book(&index, &id, &query, limit.clamp(1, 2000) as usize)?)
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TextStatusDto {
    /// `null` while the book waits to be indexed.
    pub state: Option<TextState>,
    pub pages: u32,
    pub empty_pages: u32,
    pub ocr_pages: u32,
    pub ocr_languages: Vec<String>,
    pub can_ocr: bool,
    pub message: Option<String>,
    /// Languages to read it in: the book's own, else the usual ones.
    pub suggested_languages: Vec<String>,
}

pub(crate) fn default_languages(state: &AppState) -> Vec<String> {
    let list = state
        .settings
        .lock()
        .map(|s| s.ocr_languages.clone())
        .unwrap_or_default();
    if list.is_empty() {
        vec!["eng".into()]
    } else {
        list
    }
}

/// Whether a book's words can be searched, and its OCR text.
#[tauri::command]
#[specta::specta]
pub async fn text_status(state: State<'_, AppState>, id: String) -> AppResult<TextStatusDto> {
    let library = state.library()?;
    let index = state.search_index()?;
    let defaults = default_languages(&state);
    let id = book_id(&id)?;
    blocking(move || {
        let s = library.text_status(&index, &id)?;
        let book = library.book(&id)?;
        let own = book
            .metadata
            .language
            .as_deref()
            .and_then(tessdata::from_book_language)
            .map(|c| vec![c.to_owned()]);
        let suggested = if !s.ocr_languages.is_empty() {
            s.ocr_languages.clone()
        } else {
            own.unwrap_or(defaults)
        };
        Ok(TextStatusDto {
            state: s.state,
            pages: s.pages,
            empty_pages: s.empty_pages,
            ocr_pages: s.ocr_pages,
            ocr_languages: s.ocr_languages,
            can_ocr: s.can_ocr,
            message: s.message,
            suggested_languages: suggested,
        })
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatusDto {
    pub counts: IndexCounts,
    /// Bytes on this computer.
    pub size: f64,
    pub running: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn search_index_status(state: State<'_, AppState>) -> AppResult<IndexStatusDto> {
    let index = state.search_index()?;
    let running = state.indexing();
    blocking(move || {
        Ok(IndexStatusDto {
            counts: index.counts().map_err(AppError::invalid)?,
            size: index.size() as f64,
            running,
        })
    })
    .await
}

/// Empties the index and fills it again in the background.
#[tauri::command]
#[specta::specta]
pub async fn rebuild_search_index(state: State<'_, AppState>) -> AppResult<()> {
    let index = state.search_index()?;
    blocking(move || index.clear().map_err(AppError::invalid)).await?;
    state.request_indexing();
    Ok(())
}

/// Starts indexing now (it also runs after every change to the library).
#[tauri::command]
#[specta::specta]
pub fn update_search_index(state: State<'_, AppState>) -> AppResult<()> {
    state.search_index()?;
    state.request_indexing();
    Ok(())
}

/// Reads scanned PDF and DjVu books with OCR in the background. Returns
/// the job id; the result arrives as `OcrFinished`.
#[tauri::command]
#[specta::specta]
pub fn make_searchable(
    state: State<'_, AppState>,
    ids: Vec<String>,
    languages: Vec<String>,
    redo: bool,
) -> AppResult<String> {
    let library = state.library()?;
    library.require_edit()?;
    let ids = ids
        .iter()
        .map(|i| book_id(i))
        .collect::<AppResult<Vec<_>>>()?;
    if ids.is_empty() {
        return Err(AppError::invalid("choose a book first"));
    }
    if libreri_helpers::find_program("tesseract").is_none() {
        return Err(AppError::invalid(libreri_formats::ocr::NOT_INSTALLED));
    }
    let languages = if languages.is_empty() {
        default_languages(&state)
    } else {
        languages
    };
    let tessdata_dir = tessdata::prepare(&state.tessdata, &languages).map_err(AppError::invalid)?;
    let workers = std::thread::available_parallelism()
        .map(|n| n.get() / 2)
        .unwrap_or(2)
        .clamp(1, 4);
    let options = OcrOptions {
        languages,
        tessdata: tessdata_dir,
        redo,
        workers,
    };
    Ok(state.start_ocr(ids, options)?.to_string())
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UnsearchableDto {
    pub id: String,
    pub state: TextState,
}

/// Books whose pages are scans without text (all or some).
#[tauri::command]
#[specta::specta]
pub async fn books_without_text(state: State<'_, AppState>) -> AppResult<Vec<UnsearchableDto>> {
    let library = state.library()?;
    let index = state.search_index()?;
    blocking(move || {
        Ok(library
            .books_without_text(&index)?
            .into_iter()
            .map(|(id, state)| UnsearchableDto {
                id: id.to_string(),
                state,
            })
            .collect())
    })
    .await
}

/// Removes a book's OCR text.
#[tauri::command]
#[specta::specta]
pub async fn forget_ocr(state: State<'_, AppState>, id: String) -> AppResult<()> {
    let library = state.library()?;
    let index = state.search_index().ok();
    let id = book_id(&id)?;
    blocking(move || Ok(library.forget_ocr(&id, index.as_deref())?)).await
}

/// Pages of a PDF that have OCR text, for the reader's hidden text layer.
#[tauri::command]
#[specta::specta]
pub async fn ocr_pages(state: State<'_, AppState>, id: String) -> AppResult<Vec<u32>> {
    let library = state.library()?;
    let id = book_id(&id)?;
    blocking(move || Ok(library.ocr_pages(&id)?)).await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OcrLanguagesDto {
    pub languages: Vec<OcrLanguage>,
    /// Used when a book does not say its language.
    pub defaults: Vec<String>,
    pub tesseract: bool,
}

#[tauri::command]
#[specta::specta]
pub async fn ocr_languages(state: State<'_, AppState>) -> AppResult<OcrLanguagesDto> {
    let dir = state.tessdata.clone();
    let defaults = default_languages(&state);
    blocking(move || {
        Ok(OcrLanguagesDto {
            languages: tessdata::languages(&dir),
            defaults,
            tesseract: libreri_helpers::find_program("tesseract").is_some(),
        })
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub fn set_ocr_languages(state: State<'_, AppState>, languages: Vec<String>) -> AppResult<()> {
    state.update_settings(|s| s.ocr_languages = languages)?;
    Ok(())
}

/// Downloads an OCR language; progress arrives as `OcrLanguageDownload`.
#[tauri::command]
#[specta::specta]
pub async fn download_ocr_language(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
) -> AppResult<()> {
    let dir = state.tessdata.clone();
    blocking(move || {
        let mut last = std::time::Instant::now();
        let result = tessdata::download(&dir, &code, |done, total| {
            if last.elapsed().as_millis() > 200 {
                last = std::time::Instant::now();
                let _ = OcrLanguageDownload {
                    code: code.clone(),
                    done: done as f64,
                    total: total.map(|t| t as f64),
                    finished: false,
                    error: None,
                }
                .emit(&app);
            }
        });
        let _ = OcrLanguageDownload {
            code: code.clone(),
            done: 0.0,
            total: None,
            finished: true,
            error: result.as_ref().err().cloned(),
        }
        .emit(&app);
        result.map_err(AppError::invalid)
    })
    .await
}

#[tauri::command]
#[specta::specta]
pub fn remove_ocr_language(state: State<'_, AppState>, code: String) -> AppResult<()> {
    tessdata::remove(&state.tessdata, &code).map_err(AppError::invalid)
}
