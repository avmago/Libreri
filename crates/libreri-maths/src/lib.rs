//! Reading maths from pictures as LaTeX (ADR 0025).
//!
//! Off by default: Libreri copies LaTeX exactly where a book has it. When
//! turned on in Settings, the pix2tex model (Lukas Blecher, MIT licence,
//! `data/LICENSE-pix2tex`) is downloaded once (about 100 MB, from the
//! project's own GitHub release) and run on this computer with candle, a
//! pure-Rust engine, so nothing leaves the computer and nothing else needs
//! installing. The download is checked against a known SHA-256 before it
//! is used; a bad or missing file only turns the feature off.

mod model;
mod prepare;
mod tokens;

use libreri_helpers::download;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub use tokens::{decode, tidy};

const URL: &str = "https://github.com/lukas-blecher/LaTeX-OCR/releases/download/v0.0.1/weights.pth";
const SIZE: u64 = 102_113_875;
const SHA256: &str = "a63d9141c53d266cb682fb5a8bd83bd5cbe283145e0e78ebdc0f895195a1dfaa";

/// Whether the model is here.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub downloaded: bool,
    pub size_mb: u32,
}

fn weights(dir: &Path) -> PathBuf {
    dir.join("maths").join("pix2tex-weights.pth")
}

pub fn status(dir: &Path) -> ModelStatus {
    ModelStatus {
        downloaded: std::fs::metadata(weights(dir)).is_ok_and(|m| m.len() == SIZE),
        size_mb: (SIZE / 1_000_000) as u32,
    }
}

/// Downloads the model into `<dir>/maths/`. `progress` gets bytes so far
/// and the total.
pub fn download(
    dir: &Path,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let url = std::env::var("LIBRERI_MATHS_URL").unwrap_or_else(|_| URL.into());
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(download::BODY_LIMIT))
        .max_redirects(8)
        .build()
        .into();
    let res = agent.get(&url).call().map_err(|e| match e {
        ureq::Error::StatusCode(c) => format!("the download failed (HTTP {c})"),
        ureq::Error::HostNotFound => "the download failed; check the internet connection".into(),
        other => format!("the download failed: {other}"),
    })?;
    let body = res
        .into_body()
        .into_with_config()
        .limit(SIZE + 1024)
        .reader();
    let mut body = download::StallReader::new(body, download::STALL, Some(cancel));
    let target = weights(dir);
    let folder = target.parent().ok_or("no folder")?;
    std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
    let tmp = folder.join(".pix2tex.download");
    let fail = |msg: String| {
        let _ = std::fs::remove_file(&tmp);
        msg
    };
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut hash = Sha256::new();
    let mut buf = vec![0u8; 256 * 1024];
    let mut got = 0u64;
    loop {
        if cancel.load(Ordering::SeqCst) {
            drop(file);
            return Err(fail("cancelled".into()));
        }
        let n = body.read(&mut buf).map_err(|e| {
            if download::is_cancelled(&e) {
                fail("cancelled".into())
            } else {
                fail(format!("the download stopped: {e}"))
            }
        })?;
        if n == 0 {
            break;
        }
        hash.update(&buf[..n]);
        file.write_all(&buf[..n]).map_err(|e| fail(e.to_string()))?;
        got += n as u64;
        progress(got, SIZE);
    }
    drop(file);
    let sum: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
    if got != SIZE || sum != SHA256 {
        return Err(fail("the download was not the expected file".into()));
    }
    std::fs::rename(&tmp, &target).map_err(|e| fail(e.to_string()))
}

/// Removes the model.
pub fn remove(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir.join("maths")) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Sizes (typical mark height in pixels) pictures are read at, most
/// likely first.
const MARK_HEIGHTS: [f32; 4] = [8.0, 6.5, 10.0, 5.0];

/// The loaded model (about 400 MB of memory; load once and keep).
pub struct Reader {
    model: model::Model,
}

impl Reader {
    pub fn load(dir: &Path) -> Result<Self, String> {
        if !status(dir).downloaded {
            return Err("the maths model is not downloaded".into());
        }
        model::Model::load(&weights(dir))
            .map(|model| Self { model })
            .map_err(|e| format!("the maths model could not be loaded: {e}"))
    }

    /// Reads a picture of a formula (PNG, JPEG or WebP bytes) as LaTeX.
    pub fn read(&self, picture: &[u8]) -> Result<String, String> {
        let img = image::load_from_memory(picture).map_err(|e| e.to_string())?;
        self.read_image(&img)
    }

    /// The model reads writing of one size best, and pictures come at
    /// any size: it is read at a few sizes around the usual one and the
    /// reading the model is surest of is kept.
    pub fn read_image(&self, img: &image::DynamicImage) -> Result<String, String> {
        let mark = prepare::mark_height(img).ok_or("there is no writing in the picture")?;
        let mut best: Option<(f32, Vec<u32>)> = None;
        for target in MARK_HEIGHTS {
            let scale = (target / mark).clamp(0.05, 4.0);
            let input =
                prepare::prepare(img, scale, &self.model.device).map_err(|e| e.to_string())?;
            let (ids, sure) = self
                .model
                .tokens(&input, model::MAX_SEQ)
                .map_err(|e| e.to_string())?;
            if best.as_ref().is_none_or(|(b, _)| sure > *b) {
                best = Some((sure, ids));
            }
            // Nearly certain: no need to look further.
            if sure > -0.003 {
                break;
            }
        }
        let (_, ids) = best.ok_or("nothing was read")?;
        Ok(decode(&ids))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!status(dir.path()).downloaded);
        assert!(Reader::load(dir.path()).is_err());
        remove(dir.path()).unwrap();
    }

    /// With `LIBRERI_MATHS_TEST=<folder with weights.pth and PNGs>`, reads
    /// each picture (run with `--release`).
    #[test]
    fn reads_pictures() {
        let Ok(folder) = std::env::var("LIBRERI_MATHS_TEST") else {
            return;
        };
        let folder = PathBuf::from(folder);
        let model = model::Model::load(&folder.join("weights.pth")).unwrap();
        let reader = Reader { model };
        let mut names: Vec<_> = std::fs::read_dir(&folder)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|x| x == "png"))
            .collect();
        names.sort();
        for p in names {
            let t = std::time::Instant::now();
            let got = reader.read(&std::fs::read(&p).unwrap()).unwrap();
            println!("{} ({:?}): {got}", p.display(), t.elapsed());
        }
    }
}
