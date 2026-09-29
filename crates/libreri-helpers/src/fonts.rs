//! Extra fonts for canvases (Phase 8a): Excalidraw's handwriting font for
//! Chinese, Japanese and Korean (Xiaolai, 13 MB) is not shipped with
//! Libreri; it can be downloaded from Settings into the app's data folder.
//! It comes from the same Excalidraw release Libreri is built with, as
//! published on the npm registry.

use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// The Excalidraw release in `apps/desktop/package.json`.
pub const EXCALIDRAW_VERSION: &str = "0.18.1";

/// A font that can be added.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtraFont {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_mb: u32,
    pub downloaded: bool,
}

struct Known {
    id: &'static str,
    /// Folder in the package's `dist/prod/fonts/`.
    folder: &'static str,
    name: &'static str,
    description: &'static str,
    size_mb: u32,
}

const KNOWN: &[Known] = &[Known {
    id: "xiaolai",
    folder: "Xiaolai",
    name: "Chinese, Japanese and Korean handwriting",
    description: "Xiaolai, the hand-drawn font canvases use for these scripts; without it they are shown in a plain font. The download is about 30 MB (the Excalidraw package it comes in); 13 MB is kept.",
    size_mb: 13,
}];

fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

/// Where a font's files go: `<dir>/fonts/<folder>/`, served to canvases
/// as `fonts/<folder>/…`, the path Excalidraw asks for.
fn folder(dir: &Path, k: &Known) -> PathBuf {
    dir.join("fonts").join(k.folder)
}

pub fn fonts(dir: &Path) -> Vec<ExtraFont> {
    KNOWN
        .iter()
        .map(|k| ExtraFont {
            id: k.id.into(),
            name: k.name.into(),
            description: k.description.into(),
            size_mb: k.size_mb,
            downloaded: folder(dir, k).join(".complete").is_file(),
        })
        .collect()
}

/// Counts bytes read, for progress.
struct Counting<'a, R> {
    inner: R,
    read: u64,
    cancel: &'a AtomicBool,
    progress: Box<dyn FnMut(u64) + 'a>,
}

impl<R: Read> Read for Counting<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.cancel.load(Ordering::SeqCst) {
            return Err(std::io::Error::other("cancelled"));
        }
        let n = self.inner.read(buf)?;
        self.read += n as u64;
        (self.progress)(self.read);
        Ok(n)
    }
}

/// Downloads a font. `progress` gets bytes so far and the download's size.
pub fn download(
    dir: &Path,
    id: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<(), String> {
    let k = known(id).ok_or("unknown font")?;
    let url = std::env::var("LIBRERI_FONTS_URL").unwrap_or_else(|_| {
        format!(
            "https://registry.npmjs.org/@excalidraw/excalidraw/-/excalidraw-{EXCALIDRAW_VERSION}.tgz"
        )
    });
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(crate::download::BODY_LIMIT))
        .max_redirects(8)
        .build()
        .into();
    let res = agent.get(&url).call().map_err(|e| match e {
        ureq::Error::StatusCode(c) => format!("the download failed (HTTP {c})"),
        ureq::Error::HostNotFound => "the download failed; check the internet connection".into(),
        other => format!("the download failed: {other}"),
    })?;
    let total: Option<u64> = res
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    let body = res
        .into_body()
        .into_with_config()
        .limit(200 * 1024 * 1024)
        .reader();
    let reader = crate::download::StallReader::new(body, crate::download::STALL, Some(cancel));
    let counting = Counting {
        inner: reader,
        read: 0,
        cancel,
        progress: Box::new(move |n| progress(n, total)),
    };
    let target = folder(dir, k);
    let tmp = dir.join("fonts").join(format!(".{}.download", k.folder));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let fail = |msg: String| {
        let _ = std::fs::remove_dir_all(&tmp);
        msg
    };
    let prefix = format!("package/dist/prod/fonts/{}/", k.folder);
    let mut archive = tar::Archive::new(flate2::read::GzDecoder::new(counting));
    let mut count = 0;
    let entries = archive.entries().map_err(|e| fail(e.to_string()))?;
    for entry in entries {
        let mut entry = entry.map_err(|e| {
            fail(if e.to_string().contains("cancelled") {
                "cancelled".into()
            } else {
                format!("the download stopped: {e}")
            })
        })?;
        let path = entry.path().map_err(|e| fail(e.to_string()))?.into_owned();
        let Some(name) = path
            .to_str()
            .and_then(|p| p.strip_prefix(&prefix))
            .filter(|n| n.ends_with(".woff2") && !n.contains('/') && !n.contains(".."))
            .map(str::to_owned)
        else {
            continue;
        };
        let mut bytes = Vec::new();
        entry
            .read_to_end(&mut bytes)
            .map_err(|e| fail(e.to_string()))?;
        std::fs::write(tmp.join(&name), bytes).map_err(|e| fail(e.to_string()))?;
        count += 1;
    }
    if count < 10 {
        return Err(fail("the download did not have the font".into()));
    }
    std::fs::write(tmp.join(".complete"), EXCALIDRAW_VERSION).map_err(|e| fail(e.to_string()))?;
    let _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&tmp, &target).map_err(|e| fail(e.to_string()))?;
    Ok(())
}

/// Removes a downloaded font.
pub fn remove(dir: &Path, id: &str) -> Result<(), String> {
    let k = known(id).ok_or("unknown font")?;
    match std::fs::remove_dir_all(folder(dir, k)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// The file for a request like `fonts/Xiaolai/…woff2`, if downloaded.
pub fn file(dir: &Path, rel: &str) -> Option<PathBuf> {
    let mut parts = rel.split('/');
    let (Some("fonts"), Some(folder_name), Some(name), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let k = KNOWN.iter().find(|k| k.folder == folder_name)?;
    if !name.ends_with(".woff2") || name.contains("..") || name.starts_with('.') {
        return None;
    }
    let p = folder(dir, k).join(name);
    p.is_file().then_some(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serves_only_downloaded_font_files() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!fonts(dir.path())[0].downloaded);
        let f = dir.path().join("fonts/Xiaolai");
        std::fs::create_dir_all(&f).unwrap();
        std::fs::write(f.join("a.woff2"), b"x").unwrap();
        std::fs::write(f.join(".complete"), b"0").unwrap();
        assert!(fonts(dir.path())[0].downloaded);
        assert!(file(dir.path(), "fonts/Xiaolai/a.woff2").is_some());
        assert!(file(dir.path(), "fonts/Xiaolai/../x.woff2").is_none());
        assert!(file(dir.path(), "fonts/Virgil/a.woff2").is_none());
        remove(dir.path(), "xiaolai").unwrap();
        assert!(!fonts(dir.path())[0].downloaded);
    }

    /// With the network: `LIBRERI_TEST_FONTS=1 cargo test -p libreri-helpers fonts::tests::real`.
    #[test]
    fn real() {
        if std::env::var_os("LIBRERI_TEST_FONTS").is_none() {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        download(dir.path(), "xiaolai", &AtomicBool::new(false), |_, _| {}).unwrap();
        assert!(fonts(dir.path())[0].downloaded);
        let n = std::fs::read_dir(dir.path().join("fonts/Xiaolai"))
            .unwrap()
            .count();
        assert!(n > 100, "{n}");
    }
}
