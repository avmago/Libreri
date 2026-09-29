//! Barcode scanning: pictures from the camera or a file, and the phone page.

use super::blocking;
use crate::error::{AppError, AppErrorKind, AppResult};
use crate::events::PhoneScan;
use crate::state::AppState;
use base64::Engine;
use serde::Serialize;
use specta::Type;
use std::time::Duration;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

/// How long the phone page works.
const PHONE_LIFETIME: Duration = Duration::from_secs(10 * 60);

/// Largest picture accepted from the interface.
const MAX_PICTURE: usize = 25 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ScannedDto {
    pub code: String,
    pub isbn13: Option<String>,
}

impl From<libreri_scan::Scanned> for ScannedDto {
    fn from(s: libreri_scan::Scanned) -> Self {
        Self {
            code: s.code,
            isbn13: s.isbn13,
        }
    }
}

async fn decode(bytes: Vec<u8>) -> AppResult<Option<ScannedDto>> {
    if bytes.len() > MAX_PICTURE {
        return Err(AppError::invalid("that picture is too large"));
    }
    tauri::async_runtime::spawn_blocking(move || {
        libreri_scan::decode(&bytes)
            .map(|s| s.map(Into::into))
            .map_err(AppError::invalid)
    })
    .await
    .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}

/// Reads a barcode from a picture (a camera frame), sent as a data: URL or
/// base64. Returns nothing if there is no readable barcode.
#[tauri::command]
#[specta::specta]
pub async fn scan_picture(data: String) -> AppResult<Option<ScannedDto>> {
    let b64 = data.split_once(',').map_or(data.as_str(), |(_, b)| b);
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(b64.trim())
        .map_err(|_| AppError::invalid("that picture could not be read"))?;
    decode(bytes).await
}

/// Reads a barcode from a picture file the reader chose.
#[tauri::command]
#[specta::specta]
pub async fn scan_picture_file(path: String) -> AppResult<Option<ScannedDto>> {
    let meta = std::fs::metadata(&path)?;
    if meta.len() as usize > MAX_PICTURE {
        return Err(AppError::invalid("that picture is too large"));
    }
    decode(std::fs::read(&path)?).await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PhonePairingDto {
    pub url: String,
    pub qr_svg: String,
    pub expires_in: u32,
}

/// Starts the phone page (replacing any earlier one). Scans arrive as
/// `PhoneScan` events.
#[tauri::command]
#[specta::specta]
pub async fn start_phone_scan(app: AppHandle) -> AppResult<PhonePairingDto> {
    blocking(move || {
        let state = app.state::<AppState>();
        let events = app.clone();
        state.library()?;
        state.stop_phone_scan();
        let scanner = libreri_scan::PhoneScanner::start(PHONE_LIFETIME, move |event| {
            let payload = match event {
                libreri_scan::PhoneEvent::Opened => PhoneScan {
                    kind: "opened".into(),
                    scanned: None,
                },
                libreri_scan::PhoneEvent::Scanned(s) => PhoneScan {
                    kind: "scanned".into(),
                    scanned: Some(s.into()),
                },
                libreri_scan::PhoneEvent::Page(_) => return,
            };
            let _ = payload.emit(&events);
        })
        .map_err(AppError::invalid)?;
        let p = scanner.pairing().clone();
        state.set_phone_scanner(scanner);
        Ok(PhonePairingDto {
            url: p.url,
            qr_svg: p.qr_svg,
            expires_in: p.expires_in as u32,
        })
    })
    .await
}

/// Stops the phone page.
#[tauri::command]
#[specta::specta]
pub async fn stop_phone_scan(app: AppHandle) {
    let _ = blocking(move || {
        app.state::<AppState>().stop_phone_scan();
        Ok(())
    })
    .await;
}
