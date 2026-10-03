//! Comic and DjVu pages, and the helper programs (DjVuLibre, Tesseract,
//! unar) with their assisted install.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::events::HelperInstall;
use crate::state::AppState;
use libreri_core::BookId;
use libreri_helpers::{Helper, HelperStatus, InstallPlan, Installer};
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

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OutlineDto {
    pub title: String,
    pub page: Option<u32>,
    pub children: Vec<OutlineDto>,
}

fn outline(items: Vec<libreri_library::OutlineItem>) -> Vec<OutlineDto> {
    items
        .into_iter()
        .map(|i| OutlineDto {
            title: i.title,
            page: i.page,
            children: outline(i.children),
        })
        .collect()
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PageBookDto {
    /// "comic" | "djvu"
    pub kind: String,
    pub pages: u32,
    /// Width and height of each page (DjVu).
    pub sizes: Vec<(u32, u32)>,
    pub right_to_left: bool,
    pub outline: Vec<OutlineDto>,
    pub has_text: bool,
}

/// Opens a comic or DjVu book: page count, sizes and contents. Pages are
/// then loaded from `book://localhost/.pages/<id>/<page>?w=<width>`.
#[tauri::command]
#[specta::specta]
pub async fn open_pages(state: State<'_, AppState>, id: String) -> AppResult<PageBookDto> {
    let library = state.library()?;
    let cache = state.page_cache.clone();
    let id = book_id(&id)?;
    blocking(move || {
        let b = library.open_pages(&id, &cache)?;
        Ok(PageBookDto {
            kind: match b.kind {
                libreri_library::PageKind::Comic => "comic".into(),
                libreri_library::PageKind::Djvu => "djvu".into(),
            },
            pages: b.pages,
            sizes: b.sizes,
            right_to_left: b.right_to_left,
            outline: outline(b.outline),
            has_text: b.has_text,
        })
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
pub struct WordDto {
    pub text: String,
    /// x, y, width, height as fractions of the page from the top left.
    pub rect: [f64; 4],
}

/// The words of a DjVu page with their boxes, for selecting and finding.
#[tauri::command]
#[specta::specta]
pub async fn page_words(
    state: State<'_, AppState>,
    id: String,
    page: u32,
) -> AppResult<Vec<WordDto>> {
    let library = state.library()?;
    let id = book_id(&id)?;
    blocking(move || {
        Ok(library
            .page_words(&id, page)?
            .into_iter()
            .map(|w| WordDto {
                text: w.text,
                rect: w.rect,
            })
            .collect())
    })
    .await
}

/// The plain text of every DjVu page, for Find in book.
#[tauri::command]
#[specta::specta]
pub async fn page_texts(state: State<'_, AppState>, id: String) -> AppResult<Vec<String>> {
    let library = state.library()?;
    let cache = state.page_cache.clone();
    let id = book_id(&id)?;
    blocking(move || Ok(library.page_texts(&id, &cache)?)).await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HelperInfo {
    #[serde(flatten)]
    pub status: HelperStatus,
    pub name: String,
    pub purpose: String,
    pub licence: String,
    /// How Libreri would install it here, if it can.
    pub plan: Option<InstallPlan>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HelpersDto {
    pub helpers: Vec<HelperInfo>,
    pub installer: Option<Installer>,
    /// "macos" | "windows" | "linux" | "flatpak"
    pub platform: String,
}

fn helpers_dto() -> HelpersDto {
    let installer = libreri_helpers::installer();
    HelpersDto {
        helpers: Helper::ALL
            .iter()
            .map(|h| HelperInfo {
                status: libreri_helpers::status(*h),
                name: h.name().into(),
                purpose: h.purpose().into(),
                licence: h.licence().into(),
                plan: installer.and_then(|i| libreri_helpers::install_plan(*h, i)),
            })
            .collect(),
        installer,
        platform: if libreri_helpers::in_flatpak() {
            "flatpak".into()
        } else if cfg!(target_os = "macos") {
            "macos".into()
        } else if cfg!(windows) {
            "windows".into()
        } else {
            "linux".into()
        },
    }
}

/// Which helper programs are installed, and how to install the others.
#[tauri::command]
#[specta::specta]
pub async fn helpers_status() -> AppResult<HelpersDto> {
    blocking(|| {
        libreri_helpers::refresh();
        Ok(helpers_dto())
    })
    .await
}

/// Installs a helper with the computer's package manager. Progress arrives
/// as `HelperInstall` events; the call returns when it is done.
#[tauri::command]
#[specta::specta]
pub async fn install_helper(app: AppHandle, helper: Helper) -> AppResult<HelpersDto> {
    let installer = libreri_helpers::installer().ok_or_else(|| {
        AppError::invalid(if cfg!(target_os = "macos") {
            "Homebrew is needed to install this. Install Homebrew from brew.sh, then try again."
        } else if cfg!(windows) {
            "winget (App Installer from the Microsoft Store) is needed to install this."
        } else if libreri_helpers::in_flatpak() {
            "The Flatpak version of Libreri cannot install helper programs."
        } else {
            "No package manager Libreri knows was found; install it with your system's software centre."
        })
    })?;
    let plan = libreri_helpers::install_plan(helper, installer).ok_or_else(|| {
        AppError::invalid(format!(
            "{} cannot install {}",
            installer.name(),
            helper.name()
        ))
    })?;
    let handle = app.clone();
    blocking(move || {
        let result = libreri_helpers::run_install(&plan, |line| {
            let _ = HelperInstall {
                helper,
                line: Some(line.to_owned()),
                done: false,
                error: None,
            }
            .emit(&handle);
        });
        let _ = HelperInstall {
            helper,
            line: None,
            done: true,
            error: result.as_ref().err().cloned(),
        }
        .emit(&handle);
        result.map_err(AppError::invalid)?;
        Ok(helpers_dto())
    })
    .await
}

/// Bytes used by the page cache on this computer.
#[tauri::command]
#[specta::specta]
pub async fn page_cache_size(state: State<'_, AppState>) -> AppResult<f64> {
    let cache = state.page_cache.clone();
    blocking(move || Ok(libreri_library::page_cache_size(&cache) as f64)).await
}

/// Empties the page cache (pages are made again when read).
#[tauri::command]
#[specta::specta]
pub async fn clear_page_cache(state: State<'_, AppState>) -> AppResult<()> {
    let cache = state.page_cache.clone();
    blocking(move || {
        libreri_library::prune_page_cache(&cache, 0);
        Ok(())
    })
    .await
}
