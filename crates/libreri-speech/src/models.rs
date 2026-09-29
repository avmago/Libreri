//! Whisper models: which exist, which are downloaded (per computer, in
//! the app's data folder), downloading and removing them.

use libreri_helpers::download::{self, StallReader};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// Where models come from; `{name}` is the file name.
const SOURCE: &str = "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{name}";

/// A model Libreri offers.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    /// About how big the download is, in MB.
    pub size_mb: u32,
    pub description: String,
    /// Offered first; the bigger ones are in "More models".
    pub recommended: bool,
    pub downloaded: bool,
}

struct Known {
    id: &'static str,
    file: &'static str,
    name: &'static str,
    size_mb: u32,
    description: &'static str,
    recommended: bool,
}

const KNOWN: &[Known] = &[
    Known {
        id: "tiny",
        file: "ggml-tiny.bin",
        name: "Tiny",
        size_mb: 75,
        description:
            "Fastest; good enough for dictating notes and finding your place in audiobooks.",
        recommended: true,
    },
    Known {
        id: "small",
        file: "ggml-small.bin",
        name: "Small",
        size_mb: 466,
        description: "Much more accurate; a little slower. A good default on most computers.",
        recommended: true,
    },
    Known {
        id: "medium",
        file: "ggml-medium.bin",
        name: "Medium",
        size_mb: 1500,
        description:
            "More accurate still, for difficult recordings and accents. Needs a fast computer.",
        recommended: false,
    },
    Known {
        id: "large-v3-turbo",
        file: "ggml-large-v3-turbo.bin",
        name: "Large (turbo)",
        size_mb: 1620,
        description:
            "The most accurate. Best on computers with a strong graphics chip (Apple silicon).",
        recommended: false,
    },
];

fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

/// The file of a downloaded model, if it is there.
pub fn model_path(dir: &Path, id: &str) -> Option<PathBuf> {
    let k = known(id)?;
    let p = dir.join(k.file);
    p.is_file().then_some(p)
}

/// Every model, and whether it is downloaded.
pub fn models(dir: &Path) -> Vec<ModelInfo> {
    KNOWN
        .iter()
        .map(|k| ModelInfo {
            id: k.id.to_owned(),
            name: k.name.to_owned(),
            size_mb: k.size_mb,
            description: k.description.to_owned(),
            recommended: k.recommended,
            downloaded: dir.join(k.file).is_file(),
        })
        .collect()
}

/// Downloads a model. `progress` gets bytes so far and the total when
/// known; `cancel` stops it.
pub fn download(
    dir: &Path,
    id: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<PathBuf, String> {
    let k = known(id).ok_or("unknown model")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let url = std::env::var("LIBRERI_WHISPER_URL")
        .unwrap_or_else(|_| SOURCE.to_owned())
        .replace("{name}", k.file);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(libreri_helpers::download::BODY_LIMIT))
        .max_redirects(8)
        .build()
        .into();
    let res = agent.get(&url).call().map_err(|e| match e {
        ureq::Error::StatusCode(c) => format!("the download failed (HTTP {c})"),
        ureq::Error::HostNotFound => "the download failed; check the internet connection".into(),
        other => format!("the download failed: {other}"),
    })?;
    let total = res
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    let tmp = dir.join(format!(".{}.download", k.file));
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let body = res
        .into_body()
        .into_with_config()
        .limit(4 * 1024 * 1024 * 1024)
        .reader();
    let mut reader = StallReader::new(body, download::STALL, Some(cancel));
    let mut buf = vec![0u8; 256 * 1024];
    let mut done = 0u64;
    let mut head = Vec::new();
    let fail = |tmp: &Path, msg: String| {
        let _ = std::fs::remove_file(tmp);
        msg
    };
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err(fail(&tmp, "cancelled".into()));
        }
        let n = reader.read(&mut buf).map_err(|e| {
            if download::is_cancelled(&e) {
                fail(&tmp, "cancelled".into())
            } else {
                fail(&tmp, format!("the download stopped: {e}"))
            }
        })?;
        if n == 0 {
            break;
        }
        if head.len() < 4 {
            head.extend_from_slice(&buf[..n.min(4)]);
        }
        file.write_all(&buf[..n])
            .map_err(|e| fail(&tmp, e.to_string()))?;
        done += n as u64;
        progress(done, total);
    }
    drop(file);
    // ggml model files start with "lmgg" (0x67676d6c, little-endian).
    if head.get(..4) != Some(b"lmgg".as_slice()) || done < 1_000_000 {
        return Err(fail(&tmp, "the download was not a speech model".into()));
    }
    if total.is_some_and(|t| t != done) {
        return Err(fail(&tmp, "the download was cut short".into()));
    }
    let dest = dir.join(k.file);
    std::fs::rename(&tmp, &dest).map_err(|e| e.to_string())?;
    Ok(dest)
}

/// Removes a downloaded model.
pub fn remove(dir: &Path, id: &str) -> Result<(), String> {
    let k = known(id).ok_or("unknown model")?;
    match std::fs::remove_file(dir.join(k.file)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_and_removes_models() {
        let dir = tempfile::tempdir().unwrap();
        assert!(models(dir.path()).iter().all(|m| !m.downloaded));
        std::fs::write(dir.path().join("ggml-tiny.bin"), b"lmgg").unwrap();
        let list = models(dir.path());
        assert!(list.iter().find(|m| m.id == "tiny").unwrap().downloaded);
        assert_eq!(list.iter().filter(|m| m.recommended).count(), 2);
        assert!(model_path(dir.path(), "tiny").is_some());
        remove(dir.path(), "tiny").unwrap();
        assert!(model_path(dir.path(), "tiny").is_none());
        assert!(remove(dir.path(), "nope").is_err());
    }
}
