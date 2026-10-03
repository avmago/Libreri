//! Comparing documents: versions of a book, two books, or a book
//! and a file. Pages are shown through `book://…/.compare/<id>/<a|b>/<page>`.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use libreri_core::BookId;
use libreri_library::CompareSource;
use libreri_pdf_edit::{Change, PagePair};
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

/// One side of a comparison.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CompareSourceDto {
    /// A book as it is now.
    Book { id: String },
    /// An earlier version of a book.
    Version { book: String, version: String },
    /// A PDF or DjVu file on this computer.
    File { path: String },
}

impl CompareSourceDto {
    fn source(&self) -> AppResult<CompareSource> {
        Ok(match self {
            Self::Book { id } => CompareSource::Book(book_id(id)?),
            Self::Version { book, version } => CompareSource::Version {
                book: book_id(book)?,
                version: book_id(version)?,
            },
            Self::File { path } => {
                let p = std::path::PathBuf::from(path);
                if !p.is_absolute() || !p.is_file() {
                    return Err(AppError::invalid("choose a PDF or DjVu file"));
                }
                CompareSource::File(p)
            }
        })
    }
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CompareStarted {
    pub id: String,
    pub job_id: String,
    pub a_label: String,
    pub b_label: String,
    pub a_pages: u32,
    pub b_pages: u32,
}

/// Opens both sides and starts comparing them; the result arrives as a
/// `CompareFinished` event (progress as job events).
#[tauri::command]
#[specta::specta]
pub async fn start_compare(
    app: tauri::AppHandle,
    a: CompareSourceDto,
    b: CompareSourceDto,
) -> AppResult<CompareStarted> {
    let (a, b) = (a.source()?, b.source()?);
    blocking(move || {
        use tauri::Manager;
        let state = app.state::<AppState>();
        let (id, job) = state.start_compare(a, b)?;
        let s = state
            .compare_session(&id)
            .ok_or_else(|| AppError::invalid("the comparison is gone"))?;
        Ok(CompareStarted {
            id,
            job_id: job.to_string(),
            a_label: s.a.label.clone(),
            b_label: s.b.label.clone(),
            a_pages: s.a.pages(),
            b_pages: s.b.pages(),
        })
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ComparisonDto {
    pub a_label: String,
    pub b_label: String,
    pub a_pages: u32,
    pub b_pages: u32,
    /// False while it is still running.
    pub done: bool,
    pub error: Option<String>,
    pub pairs: Vec<PagePair>,
    pub changes: Vec<Change>,
}

/// A comparison's result (or that it is still running).
#[tauri::command]
#[specta::specta]
pub fn get_comparison(state: State<'_, AppState>, id: String) -> AppResult<ComparisonDto> {
    let s = state
        .compare_session(&id)
        .ok_or_else(|| AppError::invalid("the comparison is gone; start it again"))?;
    let result = s.result.lock().unwrap_or_else(|p| p.into_inner()).clone();
    let mut dto = ComparisonDto {
        a_label: s.a.label.clone(),
        b_label: s.b.label.clone(),
        a_pages: s.a.pages(),
        b_pages: s.b.pages(),
        done: result.is_some(),
        error: None,
        pairs: Vec::new(),
        changes: Vec::new(),
    };
    match result {
        Some(Ok(r)) => {
            dto.pairs = r.pairs;
            dto.changes = r.changes;
        }
        Some(Err(e)) => dto.error = Some(e),
        None => {}
    }
    Ok(dto)
}

/// Saves the comparison as a PDF report at `dest`.
#[tauri::command]
#[specta::specta]
pub async fn export_compare_report(
    state: State<'_, AppState>,
    id: String,
    dest: String,
) -> AppResult<()> {
    let library = state.library()?;
    let s = state
        .compare_session(&id)
        .ok_or_else(|| AppError::invalid("the comparison is gone; start it again"))?;
    blocking(move || {
        let result = s.result.lock().unwrap_or_else(|p| p.into_inner()).clone();
        let Some(Ok(r)) = result else {
            return Err(AppError::invalid("the comparison has not finished"));
        };
        let made = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        Ok(library.compare_report(&s.a, &s.b, &r, &made, std::path::Path::new(&dest))?)
    })
    .await
}

/// Forgets a comparison (its view closed).
#[tauri::command]
#[specta::specta]
pub fn close_compare(state: State<'_, AppState>, id: String) {
    state.close_compare(&id);
}
