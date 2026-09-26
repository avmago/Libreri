use crate::dto::AppInfo;

/// Version and platform, for the About screen and diagnostics.
#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        version: env!("CARGO_PKG_VERSION").to_owned(),
        platform: std::env::consts::OS.to_owned(),
    }
}
