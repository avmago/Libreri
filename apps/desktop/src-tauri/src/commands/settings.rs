use crate::dto::{SettingsDto, Theme};
use crate::error::AppResult;
use crate::state::AppState;
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
#[specta::specta]
pub fn get_settings(state: State<'_, AppState>) -> SettingsDto {
    SettingsDto::from(&*state.settings.lock().expect("settings lock poisoned"))
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
