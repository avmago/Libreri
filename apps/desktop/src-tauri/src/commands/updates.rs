//! Updates: Libreri looks for a new version on its GitHub
//! releases (`latest.json`, signed with the project's updater key), and
//! installs it when asked, then restarts. Nothing is sent but the request.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use serde::Serialize;
use specta::Type;
use tauri::{AppHandle, State};
use tauri_plugin_updater::UpdaterExt;
use tauri_specta::Event;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateDto {
    pub version: String,
    pub current: String,
    /// What is new (the release notes), when given.
    pub notes: Option<String>,
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// Look for a new version when Libreri starts.
    pub check_on_start: bool,
    pub current: String,
    /// False in builds made without the updater key (development builds).
    pub available: bool,
}

/// Download progress of an update (bytes so far, of total when known).
#[derive(Debug, Clone, Serialize, Type, tauri_specta::Event)]
#[serde(rename_all = "camelCase")]
pub struct UpdateProgress {
    pub done: f64,
    pub total: Option<f64>,
}

fn configured(app: &AppHandle) -> bool {
    // A Flatpak is updated by Flathub, not by Libreri itself.
    if std::env::var_os("FLATPAK_ID").is_some() {
        return false;
    }
    app.config()
        .plugins
        .0
        .get("updater")
        .and_then(|u| u.get("pubkey"))
        .and_then(|k| k.as_str())
        .is_some_and(|k| !k.trim().is_empty())
}

#[tauri::command]
#[specta::specta]
pub fn update_status(app: AppHandle, state: State<'_, AppState>) -> UpdateStatus {
    let off = state
        .settings
        .lock()
        .map(|s| s.updates_off)
        .unwrap_or(false);
    UpdateStatus {
        check_on_start: !off,
        current: app.package_info().version.to_string(),
        available: configured(&app),
    }
}

#[tauri::command]
#[specta::specta]
pub fn set_update_check(state: State<'_, AppState>, on: bool) -> AppResult<()> {
    state.update_settings(|s| s.updates_off = !on)?;
    Ok(())
}

/// Looks for a newer version. None when this is the newest.
#[tauri::command]
#[specta::specta]
pub async fn update_check(app: AppHandle) -> AppResult<Option<UpdateDto>> {
    if !configured(&app) {
        return Err(AppError::invalid(
            "this build of Libreri cannot update itself",
        ));
    }
    let updater = app
        .updater()
        .map_err(|e| AppError::invalid(e.to_string()))?;
    let found = updater
        .check()
        .await
        .map_err(|e| AppError::invalid(format!("could not look for updates: {e}")))?;
    Ok(found.map(|u| UpdateDto {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone(),
        date: u.date.map(|d| d.to_string()),
    }))
}

/// Downloads and installs the new version (its signature is checked), then
/// restarts Libreri. Progress arrives as `UpdateProgress`.
#[tauri::command]
#[specta::specta]
pub async fn update_install(app: AppHandle) -> AppResult<()> {
    let updater = app
        .updater()
        .map_err(|e| AppError::invalid(e.to_string()))?;
    let Some(update) = updater
        .check()
        .await
        .map_err(|e| AppError::invalid(format!("could not look for updates: {e}")))?
    else {
        return Err(AppError::invalid("Libreri is already the newest version"));
    };
    let handle = app.clone();
    let mut done: f64 = 0.0;
    update
        .download_and_install(
            move |chunk, total| {
                done += chunk as f64;
                let _ = UpdateProgress {
                    done,
                    total: total.map(|t| t as f64),
                }
                .emit(&handle);
            },
            || {},
        )
        .await
        .map_err(|e| AppError::invalid(format!("the update could not be installed: {e}")))?;
    // Notes, positions and settings are saved as they change; the window
    // flushes what is pending before calling this.
    app.restart();
}
