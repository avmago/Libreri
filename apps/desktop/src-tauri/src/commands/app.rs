use crate::dto::AppInfo;
use crate::error::{AppError, AppErrorKind, AppResult};
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;

/// Version and platform, for the About screen and diagnostics.
#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: std::env::consts::OS.to_owned(),
    }
}

/// Opens a web link in the system browser. Only `http`, `https` and
/// `mailto` links are allowed.
#[tauri::command]
#[specta::specta]
pub fn open_external_url(app: AppHandle, url: String) -> AppResult<()> {
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://")
        || lower.starts_with("http://")
        || lower.starts_with("mailto:"))
    {
        return Err(AppError::invalid("only web and email links can be opened"));
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))
}
