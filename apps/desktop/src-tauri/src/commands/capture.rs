//! Capturing paper notes: photos from the camera, the phone or
//! picture files are straightened and cleaned, then saved as one
//! searchable PDF in the profile's notes folder.

use crate::error::{AppError, AppErrorKind, AppResult};
use crate::state::AppState;
use base64::Engine;
use libreri_scan::paper::{self, Clean, Corner};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State};

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> AppResult<T> + Send + 'static,
) -> AppResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))?
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CapturePhoto {
    /// The photo, kept for this capture only.
    pub id: String,
    pub width: u32,
    pub height: u32,
    /// Where the page was found (top-left, top-right, bottom-right,
    /// bottom-left), as fractions.
    pub corners: Vec<Corner>,
    /// A small copy to show, as a data: URL.
    pub preview: String,
}

fn jpeg_url(bytes: &[u8]) -> String {
    format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn thumb(img: &image::DynamicImage, side: u32) -> AppResult<String> {
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 80);
    img.thumbnail(side, side)
        .to_rgb8()
        .write_with_encoder(enc)
        .map_err(|e| AppError::invalid(e.to_string()))?;
    Ok(jpeg_url(&out))
}

/// Opens a photo; iPhone HEIC pictures are converted with macOS's `sips`.
fn open_photo(bytes: Vec<u8>, name: &str) -> AppResult<image::DynamicImage> {
    let heic = name.to_ascii_lowercase().ends_with(".heic")
        || name.to_ascii_lowercase().ends_with(".heif")
        || bytes
            .get(4..12)
            .is_some_and(|b| b.starts_with(b"ftypheic") || b.starts_with(b"ftypmif1"));
    if heic {
        if !cfg!(target_os = "macos") {
            return Err(AppError::invalid(
                "HEIC pictures can only be read on a Mac; save it as JPEG first",
            ));
        }
        let dir = std::env::temp_dir().join("libreri-capture");
        std::fs::create_dir_all(&dir)?;
        let src = dir.join(format!("{}.heic", uuid::Uuid::new_v4()));
        let dst = src.with_extension("jpg");
        std::fs::write(&src, &bytes)?;
        let ok = std::process::Command::new("/usr/bin/sips")
            .args(["-s", "format", "jpeg"])
            .arg(&src)
            .arg("--out")
            .arg(&dst)
            .output()
            .is_ok_and(|o| o.status.success());
        let _ = std::fs::remove_file(&src);
        let out = std::fs::read(&dst)
            .map_err(|_| AppError::invalid("the HEIC picture could not be read"));
        let _ = std::fs::remove_file(&dst);
        if !ok {
            return Err(AppError::invalid("the HEIC picture could not be read"));
        }
        return image::load_from_memory(&out?).map_err(|e| AppError::invalid(e.to_string()));
    }
    image::load_from_memory(&bytes)
        .map_err(|_| AppError::invalid("that picture could not be read (JPEG, PNG, WebP or HEIC)"))
}

fn add(state: &AppState, img: image::DynamicImage) -> AppResult<CapturePhoto> {
    let library = state.library()?;
    // Photos are kept as JPEG while the capture is being made.
    let path = library.scratch_file("jpg")?;
    let mut out = Vec::new();
    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
    let img = image::DynamicImage::ImageRgb8(img.to_rgb8());
    img.write_with_encoder(enc)
        .map_err(|e| AppError::invalid(e.to_string()))?;
    std::fs::write(&path, &out)?;
    let id = uuid::Uuid::new_v4().to_string();
    state
        .capture_photos
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(id.clone(), path);
    Ok(CapturePhoto {
        id,
        width: img.width(),
        height: img.height(),
        corners: paper::detect(&img).to_vec(),
        preview: thumb(&img, 1400)?,
    })
}

/// Adds a photo (a data: URL, from the camera or the phone).
#[tauri::command]
#[specta::specta]
pub async fn capture_add(app: AppHandle, photo: String) -> AppResult<CapturePhoto> {
    blocking(move || {
        let data = photo.split_once(',').map_or(photo.as_str(), |(_, d)| d);
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|_| AppError::invalid("the photo could not be read"))?;
        let img = open_photo(bytes, "")?;
        add(&app.state::<AppState>(), img)
    })
    .await
}

/// Adds a picture file from the computer.
#[tauri::command]
#[specta::specta]
pub async fn capture_add_file(app: AppHandle, path: String) -> AppResult<CapturePhoto> {
    blocking(move || {
        let p = PathBuf::from(&path);
        let bytes = std::fs::read(&p)?;
        let img = open_photo(bytes, &path)?;
        add(&app.state::<AppState>(), img)
    })
    .await
}

fn photo_path(state: &AppState, id: &str) -> AppResult<PathBuf> {
    state
        .capture_photos
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .get(id)
        .cloned()
        .ok_or_else(|| AppError::new(AppErrorKind::NotFound, "that photo is gone; add it again"))
}

fn corners(list: &[Corner]) -> AppResult<[Corner; 4]> {
    <[Corner; 4]>::try_from(list).map_err(|_| AppError::invalid("a page has four corners"))
}

/// How one page should come out.
#[derive(Debug, Clone, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CapturePageSpec {
    pub id: String,
    pub corners: Vec<Corner>,
    pub clean: Clean,
    /// Quarter turns, clockwise.
    pub turns: i32,
}

/// The page as it will be saved, small (a data: URL).
#[tauri::command]
#[specta::specta]
pub async fn capture_preview(app: AppHandle, page: CapturePageSpec) -> AppResult<String> {
    blocking(move || {
        let path = photo_path(&app.state::<AppState>(), &page.id)?;
        let img = image::open(&path).map_err(|e| AppError::invalid(e.to_string()))?;
        // Straighten from a smaller copy: quick, and plenty for a preview.
        let small = img.thumbnail(1200, 1200);
        let jpeg = paper::process(
            &small,
            corners(&page.corners)?,
            page.clean,
            page.turns,
            78,
            Some(700),
        )
        .map_err(AppError::invalid)?;
        Ok(jpeg_url(&jpeg))
    })
    .await
}

#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CaptureSaved {
    /// The PDF in the library: `Notes/<profile>/Captures/….pdf`.
    pub path: String,
    pub pages: u32,
    /// The text read from the pages (empty when not read).
    pub text: String,
}

fn ocr_page(
    jpeg: &[u8],
    page: u32,
    languages: &[String],
    tessdata: Option<&Path>,
) -> Option<(libreri_pdf_edit::OcrWords, String)> {
    let dir = std::env::temp_dir().join("libreri-capture");
    std::fs::create_dir_all(&dir).ok()?;
    let file = dir.join(format!("{}.jpg", uuid::Uuid::new_v4()));
    std::fs::write(&file, jpeg).ok()?;
    let result = libreri_formats::ocr::recognize(&file, page, languages, tessdata, None);
    let _ = std::fs::remove_file(&file);
    let p = result.ok()?;
    let words = p
        .words
        .iter()
        .map(|w| (w.text.clone(), w.rect.map(f64::from)))
        .collect();
    Some((libreri_pdf_edit::OcrWords { page, words }, p.text))
}

/// Saves the pages as one PDF. With `read_text`, Tesseract (when installed)
/// reads them first, so the PDF can be searched.
#[tauri::command]
#[specta::specta]
pub async fn capture_save(
    app: AppHandle,
    state: State<'_, AppState>,
    pages: Vec<CapturePageSpec>,
    title: String,
    read_text: bool,
) -> AppResult<CaptureSaved> {
    let library = state.library()?;
    let languages = crate::commands::search::default_languages(&state);
    let tessdata = if read_text && libreri_helpers::find_program("tesseract").is_some() {
        Some(
            libreri_helpers::tessdata::prepare(&state.tessdata, &languages)
                .ok()
                .flatten(),
        )
    } else {
        None
    };
    blocking(move || {
        let state = app.state::<AppState>();
        let mut jpegs = Vec::new();
        for p in &pages {
            let path = photo_path(&state, &p.id)?;
            let img = image::open(&path).map_err(|e| AppError::invalid(e.to_string()))?;
            jpegs.push(
                paper::process(&img, corners(&p.corners)?, p.clean, p.turns, 82, None)
                    .map_err(AppError::invalid)?,
            );
        }
        let mut ocr = Vec::new();
        let mut text = String::new();
        if let Some(tessdata) = &tessdata {
            for (i, jpeg) in jpegs.iter().enumerate() {
                if let Some((words, t)) =
                    ocr_page(jpeg, i as u32 + 1, &languages, tessdata.as_deref())
                {
                    ocr.push(words);
                    if !t.trim().is_empty() {
                        if !text.is_empty() {
                            text.push_str("\n\n");
                        }
                        text.push_str(t.trim());
                    }
                }
            }
        }
        let count = jpegs.len() as u32;
        let path = library.save_capture(&title, jpegs, &ocr)?;
        // The photos are no longer needed.
        let mut photos = state
            .capture_photos
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for p in &pages {
            if let Some(f) = photos.remove(&p.id) {
                let _ = std::fs::remove_file(f);
            }
        }
        Ok(CaptureSaved {
            path,
            pages: count,
            text,
        })
    })
    .await
}

/// Forgets photos that were not saved.
#[tauri::command]
#[specta::specta]
pub fn capture_discard(state: State<'_, AppState>, ids: Vec<String>) {
    let mut photos = state
        .capture_photos
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    for id in ids {
        if let Some(f) = photos.remove(&id) {
            let _ = std::fs::remove_file(f);
        }
    }
}

/// Opens a captured PDF in the system's PDF app.
#[tauri::command]
#[specta::specta]
pub fn open_note_file(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let file = state.library()?.capture_file(&path)?;
    app.opener()
        .open_path(file.to_string_lossy(), None::<&str>)
        .map_err(|e| AppError::new(AppErrorKind::Io, e.to_string()))
}
