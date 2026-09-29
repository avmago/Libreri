use crate::dto::{SettingsDto, Theme};
use crate::error::AppResult;
use crate::state::AppState;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub fn get_settings(state: State<'_, AppState>) -> SettingsDto {
    SettingsDto::from(
        &*state
            .settings
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner),
    )
}

#[tauri::command]
#[specta::specta]
pub fn set_theme(state: State<'_, AppState>, theme: Theme) -> AppResult<SettingsDto> {
    let settings = state.update_settings(|s| s.theme = theme.into())?;
    Ok(SettingsDto::from(&settings))
}

#[tauri::command]
#[specta::specta]
pub fn forget_recent_library(state: State<'_, AppState>, path: String) -> AppResult<SettingsDto> {
    let path = PathBuf::from(path);
    let settings = state.update_settings(|s| s.recent_libraries.retain(|r| r.path != path))?;
    Ok(SettingsDto::from(&settings))
}

/// Accent colours offered in Settings › Appearance. `None` is the default
/// (black, near-white in dark mode).
const ACCENTS: &[&str] = &[
    "#2563eb", "#4f46e5", "#7c3aed", "#db2777", "#dc2626", "#ea580c", "#15803d", "#0f766e",
];

#[tauri::command]
#[specta::specta]
pub fn set_accent(state: State<'_, AppState>, accent: Option<String>) -> AppResult<SettingsDto> {
    if let Some(a) = &accent {
        if !ACCENTS.contains(&a.as_str()) {
            return Err(crate::error::AppError::invalid(
                "that accent colour is not offered",
            ));
        }
    }
    let settings = state.update_settings(|s| s.accent = accent)?;
    Ok(SettingsDto::from(&settings))
}
