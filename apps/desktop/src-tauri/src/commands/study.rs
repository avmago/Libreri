//! Study: the reading calendar's document (per profile), its export as an
//! .ics file, and the running timer in the menu bar, taskbar or tray.

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use serde::Deserialize;
use specta::Type;
use tauri::menu::{Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::window::{ProgressBarState, ProgressBarStatus};
use tauri::{AppHandle, Manager, State};

const TRAY: &str = "libreri-timer";

/// The signed-in profile's reading calendar (JSON), or None at first.
#[tauri::command]
#[specta::specta]
pub fn study_read(state: State<'_, AppState>) -> AppResult<Option<String>> {
    Ok(state.library()?.read_study()?)
}

#[tauri::command]
#[specta::specta]
pub fn study_write(state: State<'_, AppState>, json: String) -> AppResult<()> {
    Ok(state.library()?.write_study(&json)?)
}

/// Writes the calendar as an .ics file where the person chose to save it.
#[tauri::command]
#[specta::specta]
pub fn save_calendar_file(path: String, ics: String) -> AppResult<()> {
    let p = std::path::PathBuf::from(&path);
    if !p.extension().is_some_and(|e| e.eq_ignore_ascii_case("ics")) {
        return Err(AppError::invalid("a calendar file ends in .ics"));
    }
    if !ics.starts_with("BEGIN:VCALENDAR") {
        return Err(AppError::invalid("not a calendar"));
    }
    std::fs::write(&p, ics).map_err(|e| AppError::invalid(e.to_string()))
}

/// The running timer, as the menu bar / taskbar / tray shows it.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TimerTray {
    /// "15:32", shown next to the icon (macOS menu bar, Linux where it can).
    pub text: String,
    /// "Focus · round 3 of 4 · The Lighthouse".
    pub detail: String,
    /// How far through (0–1), for the taskbar button; None for a stopwatch.
    pub progress: Option<f64>,
    pub paused: bool,
    /// Offer "Skip" (focus sessions).
    pub can_skip: bool,
}

fn set_progress(app: &AppHandle, progress: Option<f64>, paused: bool) {
    let Some(w) = app.get_webview_window("main") else {
        return;
    };
    let state = match progress {
        Some(p) => ProgressBarState {
            status: Some(if paused {
                ProgressBarStatus::Paused
            } else {
                ProgressBarStatus::Normal
            }),
            progress: Some((p.clamp(0.0, 1.0) * 100.0).round() as u64),
        },
        None => ProgressBarState {
            status: Some(ProgressBarStatus::None),
            progress: None,
        },
    };
    let _ = w.set_progress_bar(state);
}

fn menu(app: &AppHandle, t: &TimerTray) -> tauri::Result<Menu<tauri::Wry>> {
    let head = MenuItem::with_id(app, "timer-head", &t.detail, false, None::<&str>)?;
    let pause = MenuItem::with_id(
        app,
        "timer-pause",
        if t.paused { "Resume" } else { "Pause" },
        true,
        None::<&str>,
    )?;
    let skip = MenuItem::with_id(
        app,
        "timer-skip",
        "Skip to the next part",
        t.can_skip,
        None::<&str>,
    )?;
    let stop = MenuItem::with_id(app, "timer-stop", "Stop", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let open = MenuItem::with_id(app, "timer-open", "Open Libreri", true, None::<&str>)?;
    Menu::with_items(app, &[&head, &pause, &skip, &stop, &sep, &open])
}

/// Shows the running timer in the menu bar (macOS), the taskbar and tray
/// (Windows) or the tray (Linux); None takes it away. Menu choices arrive
/// as `TimerTrayAction`.
#[tauri::command]
#[specta::specta]
pub fn timer_tray(app: AppHandle, timer: Option<TimerTray>) -> AppResult<()> {
    let Some(t) = timer else {
        let _ = app.remove_tray_by_id(TRAY);
        set_progress(&app, None, false);
        return Ok(());
    };
    set_progress(&app, t.progress, t.paused);
    let m = menu(&app, &t).map_err(|e| AppError::invalid(e.to_string()))?;
    let tooltip = format!("{} · {}", t.text, t.detail);
    if let Some(tray) = app.tray_by_id(TRAY) {
        let _ = tray.set_title(Some(&t.text));
        let _ = tray.set_tooltip(Some(&tooltip));
        let _ = tray.set_menu(Some(m));
        return Ok(());
    }
    let mut b = TrayIconBuilder::with_id(TRAY)
        .title(&t.text)
        .tooltip(&tooltip)
        .menu(&m)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, e| {
            use crate::events::TimerTrayAction;
            use tauri_specta::Event;
            let id = e.id().as_ref();
            if id == "timer-open" {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.show();
                    let _ = w.set_focus();
                }
                return;
            }
            let action = id.trim_start_matches("timer-").to_owned();
            let _ = TimerTrayAction { action }.emit(app);
        });
    if let Some(icon) = app.default_window_icon() {
        b = b.icon(icon.clone());
    }
    b.build(&app)
        .map_err(|e| AppError::invalid(e.to_string()))?;
    Ok(())
}
