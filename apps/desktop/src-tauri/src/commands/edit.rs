//! Changing PDF files (Phase 6b): page edits, redaction, corrections,
//! forms, markup saved into the PDF, and the versions kept of each change.

use crate::dto::BookDto;
use crate::error::{AppError, AppErrorKind, AppResult};
use crate::events::{LibraryChanged, PhonePage};
use crate::state::AppState;
use base64::Engine;
use libreri_core::BookId;
use libreri_pdf_edit::{EditPlan, PdfAnnot};
use serde::Serialize;
use specta::Type;
use std::path::PathBuf;
use std::time::Duration;
use tauri::{AppHandle, Manager, State};
use tauri_specta::Event;

/// Largest filled-in form accepted from the interface.
const MAX_FORM: usize = 500 * 1024 * 1024;

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

/// Pages from other PDFs are named by path, or `book:<id>` for a book in
/// the library.
fn resolve_files(state: &AppState, plan: &mut EditPlan) -> AppResult<()> {
    let library = state.library()?;
    for f in &mut plan.files {
        if let Some(id) = f.strip_prefix("book:") {
            let book = library.book(&book_id(id)?)?;
            if book.file_type != libreri_core::FileType::Pdf {
                return Err(AppError::invalid(format!(
                    "“{}” is not a PDF",
                    book.metadata.title
                )));
            }
            let path = library
                .layout()
                .resolve_relative(&book.rel_path)
                .ok_or_else(|| AppError::invalid("that book's file is missing"))?;
            *f = path.to_string_lossy().into_owned();
        } else if !PathBuf::from(&*f).is_absolute() {
            return Err(AppError::invalid("pick the PDF to take pages from"));
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditResultDto {
    pub book: BookDto,
    pub pages: u32,
    /// Pages turned into pictures so nothing stayed under a redaction.
    pub flattened: Vec<u32>,
    pub warnings: Vec<String>,
}

/// Applies page edits to a PDF. The old file is kept as a version.
#[tauri::command]
#[specta::specta]
pub async fn edit_pages(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    mut plan: EditPlan,
) -> AppResult<EditResultDto> {
    let library = state.library()?;
    let id = book_id(&id)?;
    resolve_files(&state, &mut plan)?;
    let (book, report) = blocking(move || Ok(library.edit_pages(&id, &plan)?)).await?;
    state.request_indexing();
    let _ = LibraryChanged::default().emit(&app);
    Ok(EditResultDto {
        book: book.into(),
        pages: report.pages,
        flattened: report.flattened,
        warnings: report.warnings,
    })
}

/// Writes the edited pages as a new book next to this one (the book
/// itself is not changed): "Save as new book", extracting and splitting.
#[tauri::command]
#[specta::specta]
pub async fn save_pages_as_book(
    state: State<'_, AppState>,
    id: String,
    mut plan: EditPlan,
    name: String,
) -> AppResult<()> {
    let library = state.library()?;
    let id = book_id(&id)?;
    resolve_files(&state, &mut plan)?;
    let name = libreri_export::safe_file_name(name.trim().trim_end_matches(".pdf"));
    if name.is_empty() {
        return Err(AppError::invalid("give the new book a name"));
    }
    let folder = library.book(&id)?.rel_path;
    let dir = library.scratch_file("d")?;
    std::fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{name}.pdf"));
    let out = dest.clone();
    let lib = library.clone();
    blocking(move || Ok(lib.write_edited(&id, &plan, &out).map(|_| ())?)).await?;
    let folder = std::path::Path::new(&folder)
        .parent()
        .and_then(|p| p.strip_prefix("Books").ok())
        .map(|p| p.to_string_lossy().replace('\\', "/"))
        .unwrap_or_default();
    state.start_import(libreri_library::ImportRequest {
        sources: vec![dest],
        folder,
        mode: libreri_library::ImportMode::Move,
    })?;
    Ok(())
}

/// Saves a PDF with filled-in form fields (base64 from the reader) as the
/// book's new version.
#[tauri::command]
#[specta::specta]
pub async fn save_filled_form(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    data: String,
) -> AppResult<BookDto> {
    let library = state.library()?;
    let id = book_id(&id)?;
    if data.len() > MAX_FORM / 3 * 4 + 4 {
        return Err(AppError::invalid("that PDF is too large"));
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data.trim())
        .map_err(|_| AppError::invalid("the filled-in form could not be read"))?;
    let book = blocking(move || Ok(library.save_filled_form(&id, &bytes)?)).await?;
    state.request_indexing();
    let _ = LibraryChanged::default().emit(&app);
    Ok(book.into())
}

/// Writes markup into the PDF as standard annotations (a new version) and
/// removes those marks from Libreri's own markup.
#[tauri::command]
#[specta::specta]
pub async fn save_markup_into_pdf(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    annots: Vec<PdfAnnot>,
) -> AppResult<BookDto> {
    let library = state.library()?;
    let id = book_id(&id)?;
    let book = blocking(move || Ok(library.save_markup_into_pdf(&id, &annots)?)).await?;
    let _ = LibraryChanged::default().emit(&app);
    Ok(book.into())
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionDto {
    pub id: String,
    pub file_name: String,
    pub saved_at: String,
    pub reason: String,
    #[specta(type = f64)]
    pub size: u64,
}

impl From<libreri_library::VersionInfo> for VersionDto {
    fn from(v: libreri_library::VersionInfo) -> Self {
        Self {
            id: v.id.to_string(),
            file_name: v.file_name,
            saved_at: v.saved_at,
            reason: v.reason,
            size: v.size,
        }
    }
}

/// A book's earlier versions, newest first.
#[tauri::command]
#[specta::specta]
pub fn list_versions(state: State<'_, AppState>, id: String) -> AppResult<Vec<VersionDto>> {
    Ok(state
        .library()?
        .versions(&book_id(&id)?)?
        .into_iter()
        .map(Into::into)
        .collect())
}

/// Makes an earlier version current again (the current file is kept).
#[tauri::command]
#[specta::specta]
pub async fn restore_version(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    version: String,
) -> AppResult<BookDto> {
    let library = state.library()?;
    let (id, version) = (book_id(&id)?, book_id(&version)?);
    let book = blocking(move || Ok(library.restore_version(&id, &version)?)).await?;
    state.request_indexing();
    let _ = LibraryChanged::default().emit(&app);
    Ok(book.into())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_version(
    state: State<'_, AppState>,
    id: String,
    version: String,
) -> AppResult<()> {
    let library = state.library()?;
    let (id, version) = (book_id(&id)?, book_id(&version)?);
    blocking(move || Ok(library.delete_version(&id, &version)?)).await
}

/// Saves a copy of an earlier version where the reader chooses.
#[tauri::command]
#[specta::specta]
pub async fn save_version_copy(
    state: State<'_, AppState>,
    id: String,
    version: String,
    dest: String,
) -> AppResult<()> {
    let library = state.library()?;
    let (id, version) = (book_id(&id)?, book_id(&version)?);
    blocking(move || Ok(library.copy_version(&id, &version, std::path::Path::new(&dest))?)).await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VersionsUsage {
    pub count: u32,
    #[specta(type = f64)]
    pub bytes: u64,
}

/// How much room earlier versions take.
#[tauri::command]
#[specta::specta]
pub async fn versions_usage(state: State<'_, AppState>) -> AppResult<VersionsUsage> {
    let library = state.library()?;
    blocking(move || {
        let (count, bytes) = library.versions_usage();
        Ok(VersionsUsage { count, bytes })
    })
    .await
}

/// Deletes every earlier version of every book.
#[tauri::command]
#[specta::specta]
pub async fn delete_all_versions(state: State<'_, AppState>) -> AppResult<u32> {
    let library = state.library()?;
    blocking(move || Ok(library.delete_all_versions()?)).await
}

/// Starts the phone page for photos of paper pages; photos arrive as
/// `PhonePage` events.
#[tauri::command]
#[specta::specta]
pub async fn start_phone_pages(
    app: AppHandle,
) -> AppResult<crate::commands::scan::PhonePairingDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        let events = app.clone();
        state.library()?;
        state.stop_phone_scan();
        let scanner = libreri_scan::PhoneScanner::start_for(
            libreri_scan::PhoneMode::Pages,
            Duration::from_secs(20 * 60),
            move |event| {
                let payload = match event {
                    libreri_scan::PhoneEvent::Opened => PhonePage {
                        kind: "opened".into(),
                        picture: None,
                    },
                    libreri_scan::PhoneEvent::Page(bytes) => {
                        let mime = if bytes.starts_with(&[0xFF, 0xD8]) {
                            "image/jpeg"
                        } else {
                            "image/png"
                        };
                        PhonePage {
                            kind: "page".into(),
                            picture: Some(format!(
                                "data:{mime};base64,{}",
                                base64::engine::general_purpose::STANDARD.encode(bytes)
                            )),
                        }
                    }
                    libreri_scan::PhoneEvent::Scanned(_) => return,
                };
                let _ = payload.emit(&events);
            },
        )
        .map_err(AppError::invalid)?;
        let p = scanner.pairing().clone();
        state.set_phone_scanner(scanner);
        Ok(crate::commands::scan::PhonePairingDto {
            url: p.url,
            qr_svg: p.qr_svg,
            expires_in: p.expires_in as u32,
        })
    })
    .await
}

/// How many pages a PDF file has (before taking pages from it).
#[tauri::command]
#[specta::specta]
pub async fn pdf_page_count(path: String) -> AppResult<u32> {
    blocking(move || {
        libreri_pdf_edit::page_sizes(std::path::Path::new(&path))
            .map(|s| s.len() as u32)
            .map_err(|e| AppError::invalid(e.to_string()))
    })
    .await
}
