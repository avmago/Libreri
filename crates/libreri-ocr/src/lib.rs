//! Optional OCR models (ADR 0029): downloaded on request in Settings, run
//! on this computer, removed with one click.
//!
//! Tesseract stays the default. A model reads scanned pages more like a
//! person does: headings, tables, formulas and charts come out as text in
//! reading order, at the cost of a large download and seconds per page.
//!
//! - **PaddleOCR-VL 1.6** (PaddlePaddle, Apache 2.0, 0.9 billion
//!   parameters, 109 languages): PP-DocLayoutV3 finds the regions of a page
//!   and their reading order, then PaddleOCR-VL reads each one. Both run
//!   with candle through `oar-ocr-vl`, on the graphics chip of Apple
//!   computers and on the processor elsewhere.
//! - **TeleOCR** is listed but not offered yet: there is no Rust version
//!   of it to run, and it has to be tested before Libreri can carry one.
//!
//! Files come from Hugging Face, into `<app data>/ocr-models/<model>/`. A
//! model counts as downloaded when its `.complete` note is there, written
//! after every file arrived whole.

mod paddle;
mod text;

use libreri_formats::ocr::PageReader;
use libreri_helpers::download::{self, StallReader};
use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub use text::{html_table_text, region_text};

/// Where files come from: `{repo}` and `{file}` are filled in.
const SOURCE: &str = "https://huggingface.co/{repo}/resolve/main/{file}";

/// The note written when every file of a model is here.
const COMPLETE: &str = ".complete";

/// A model Libreri can read pages with.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrModelInfo {
    /// "paddleocr-vl".
    pub id: String,
    pub name: String,
    pub description: String,
    /// About how big the download is, in MB.
    pub size_mb: u32,
    pub licence: String,
    /// Where it comes from, for people who want to know more.
    pub homepage: String,
    pub downloaded: bool,
    /// Offered for download (false: listed as coming later).
    pub available: bool,
    /// Why it is not offered yet.
    pub note: Option<String>,
}

struct File {
    repo: &'static str,
    name: &'static str,
    /// Saved under this folder of the model's (for a second checkpoint).
    folder: Option<&'static str>,
}

struct Known {
    id: &'static str,
    name: &'static str,
    description: &'static str,
    size_mb: u32,
    licence: &'static str,
    homepage: &'static str,
    files: &'static [File],
    note: Option<&'static str>,
}

const PADDLE_REPO: &str = "PaddlePaddle/PaddleOCR-VL-1.6";
const LAYOUT_REPO: &str = "PaddlePaddle/PP-DocLayoutV3_safetensors";

const KNOWN: &[Known] = &[
    Known {
        id: "paddleocr-vl",
        name: "PaddleOCR-VL 1.6",
        description: "Reads text, tables, formulas and charts on scanned pages in reading order, in 109 languages. Best on Apple computers with their graphics chip; slower elsewhere.",
        size_mb: 2_070,
        licence: "Apache 2.0",
        homepage: "https://huggingface.co/PaddlePaddle/PaddleOCR-VL-1.6",
        files: &[
            File { repo: PADDLE_REPO, name: "config.json", folder: None },
            File { repo: PADDLE_REPO, name: "preprocessor_config.json", folder: None },
            File { repo: PADDLE_REPO, name: "generation_config.json", folder: None },
            File { repo: PADDLE_REPO, name: "chat_template.jinja", folder: None },
            File { repo: PADDLE_REPO, name: "tokenizer.json", folder: None },
            File { repo: PADDLE_REPO, name: "LICENSE", folder: None },
            File { repo: PADDLE_REPO, name: "model.safetensors", folder: None },
            File { repo: LAYOUT_REPO, name: "config.json", folder: Some("layout") },
            File { repo: LAYOUT_REPO, name: "model.safetensors", folder: Some("layout") },
        ],
        note: None,
    },
    Known {
        id: "teleocr",
        name: "TeleOCR",
        description: "A 1.2 billion parameter model for text, tables, formulas and charts, strong on photographed and curved pages. English and Chinese.",
        size_mb: 2_400,
        licence: "Apache 2.0",
        homepage: "https://huggingface.co/XingChen-AGI/TeleOCR",
        files: &[],
        note: Some("Coming later: Libreri cannot run it yet, and it has to be tested on real scans first."),
    },
];

fn known(id: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.id == id)
}

fn model_dir(dir: &Path, id: &str) -> PathBuf {
    dir.join(id)
}

/// Whether a model is downloaded whole.
pub fn is_downloaded(dir: &Path, id: &str) -> bool {
    known(id).is_some_and(|k| !k.files.is_empty()) && model_dir(dir, id).join(COMPLETE).is_file()
}

/// Every model, and whether it is downloaded.
pub fn models(dir: &Path) -> Vec<OcrModelInfo> {
    KNOWN
        .iter()
        .map(|k| OcrModelInfo {
            id: k.id.to_owned(),
            name: k.name.to_owned(),
            description: k.description.to_owned(),
            size_mb: k.size_mb,
            licence: k.licence.to_owned(),
            homepage: k.homepage.to_owned(),
            downloaded: is_downloaded(dir, k.id),
            available: !k.files.is_empty(),
            note: k.note.map(str::to_owned),
        })
        .collect()
}

/// The name a model's text is kept with ("PaddleOCR-VL 1.6").
pub fn engine_name(id: &str) -> Option<&'static str> {
    known(id).map(|k| k.name)
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(download::BODY_LIMIT))
        .max_redirects(8)
        .build()
        .into()
}

fn url(source: &str, f: &File) -> String {
    source.replace("{repo}", f.repo).replace("{file}", f.name)
}

/// Downloads a model's files. `progress` gets bytes so far and about how
/// many there are in all; `cancel` stops it. Files already here whole are
/// kept, so a stopped download goes on where it was.
pub fn download(
    dir: &Path,
    id: &str,
    cancel: &AtomicBool,
    progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let source = std::env::var("LIBRERI_OCR_URL").unwrap_or_else(|_| SOURCE.to_owned());
    download_from(&source, dir, id, cancel, progress)
}

fn download_from(
    source: &str,
    dir: &Path,
    id: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let k = known(id).ok_or("unknown model")?;
    if k.files.is_empty() {
        return Err(k
            .note
            .unwrap_or("this model cannot be downloaded yet")
            .into());
    }
    let root = model_dir(dir, id);
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(root.join(COMPLETE));
    let total = u64::from(k.size_mb) * 1_000_000;
    let agent = agent();
    let mut done = 0u64;
    for f in k.files {
        let folder = f.folder.map_or_else(|| root.clone(), |d| root.join(d));
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let target = folder.join(f.name);
        let res = agent.get(&url(source, f)).call().map_err(|e| match e {
            ureq::Error::StatusCode(c) => format!("the download of {} failed (HTTP {c})", f.name),
            ureq::Error::HostNotFound => {
                "the download failed; check the internet connection".into()
            }
            other => format!("the download failed: {other}"),
        })?;
        let length: Option<u64> = res
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        // Already here whole (an earlier, stopped download).
        if let (Some(len), Ok(meta)) = (length, std::fs::metadata(&target)) {
            if meta.len() == len {
                done += len;
                progress(done, total.max(done));
                continue;
            }
        }
        let tmp = folder.join(format!(".{}.part", f.name));
        let fail = |msg: String| {
            let _ = std::fs::remove_file(&tmp);
            msg
        };
        let body = res
            .into_body()
            .into_with_config()
            .limit(8 * 1024 * 1024 * 1024)
            .reader();
        let mut reader = StallReader::new(body, download::STALL, Some(cancel));
        let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        let mut buf = vec![0u8; 256 * 1024];
        let mut got = 0u64;
        loop {
            if cancel.load(Ordering::SeqCst) {
                drop(file);
                return Err(fail("cancelled".into()));
            }
            let n = reader.read(&mut buf).map_err(|e| {
                if download::is_cancelled(&e) {
                    fail("cancelled".into())
                } else {
                    fail(format!("the download stopped: {e}"))
                }
            })?;
            if n == 0 {
                break;
            }
            file.write_all(&buf[..n]).map_err(|e| fail(e.to_string()))?;
            got += n as u64;
            done += n as u64;
            progress(done, total.max(done));
        }
        file.flush().map_err(|e| fail(e.to_string()))?;
        drop(file);
        if length.is_some_and(|l| l != got) || got == 0 {
            return Err(fail(format!("{} did not arrive whole", f.name)));
        }
        std::fs::rename(&tmp, &target).map_err(|e| fail(e.to_string()))?;
    }
    let note = serde_json::json!({
        "model": id,
        "files": k.files.iter().map(|f| format!("{}/{}", f.repo, f.name)).collect::<Vec<_>>(),
    });
    std::fs::write(root.join(COMPLETE), note.to_string()).map_err(|e| e.to_string())
}

/// Removes a model and everything it downloaded.
pub fn remove(dir: &Path, id: &str) -> Result<(), String> {
    known(id).ok_or("unknown model")?;
    match std::fs::remove_dir_all(model_dir(dir, id)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// Loads a downloaded model to read pages with (a few GB of memory: load
/// once and keep while pages are read).
pub fn load(dir: &Path, id: &str) -> Result<Arc<dyn PageReader>, String> {
    if !is_downloaded(dir, id) {
        return Err("the model is not downloaded (Settings › Helper programs)".into());
    }
    match id {
        "paddleocr-vl" => Ok(Arc::new(paddle::Paddle::load(&model_dir(dir, id))?)),
        _ => Err("Libreri cannot run this model yet".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_models() {
        let dir = tempfile::tempdir().unwrap();
        let list = models(dir.path());
        assert_eq!(list.len(), 2);
        assert!(list[0].available && !list[0].downloaded);
        assert!(!list[1].available && list[1].note.is_some());
        assert_eq!(engine_name("paddleocr-vl"), Some("PaddleOCR-VL 1.6"));
    }

    #[test]
    fn complete_note_marks_a_download_and_remove_clears_it() {
        let dir = tempfile::tempdir().unwrap();
        let m = dir.path().join("paddleocr-vl");
        std::fs::create_dir_all(&m).unwrap();
        assert!(!is_downloaded(dir.path(), "paddleocr-vl"));
        std::fs::write(m.join(COMPLETE), "{}").unwrap();
        assert!(is_downloaded(dir.path(), "paddleocr-vl"));
        // TeleOCR cannot be downloaded, whatever is on disk.
        std::fs::create_dir_all(dir.path().join("teleocr")).unwrap();
        std::fs::write(dir.path().join("teleocr").join(COMPLETE), "{}").unwrap();
        assert!(!is_downloaded(dir.path(), "teleocr"));
        remove(dir.path(), "paddleocr-vl").unwrap();
        assert!(!m.exists());
        remove(dir.path(), "paddleocr-vl").unwrap();
        assert!(download(dir.path(), "teleocr", &AtomicBool::new(false), |_, _| {}).is_err());
    }

    #[test]
    fn downloads_from_a_local_server() {
        use std::io::BufRead;
        // A tiny HTTP server that answers every file with its own name.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = std::io::BufReader::new(stream.try_clone().unwrap());
                let mut first = String::new();
                reader.read_line(&mut first).unwrap();
                loop {
                    let mut l = String::new();
                    if reader.read_line(&mut l).unwrap() == 0 || l == "\r\n" {
                        break;
                    }
                }
                let path = first.split_whitespace().nth(1).unwrap_or("/").to_owned();
                let body = path.into_bytes();
                let mut s = stream;
                let _ = write!(
                    s,
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                let _ = s.write_all(&body);
            }
        });
        let source = format!("http://127.0.0.1:{port}/{{repo}}/{{file}}");
        let dir = tempfile::tempdir().unwrap();
        let mut last = 0;
        download_from(
            &source,
            dir.path(),
            "paddleocr-vl",
            &AtomicBool::new(false),
            |d, _| last = d,
        )
        .unwrap();
        assert!(last > 0);
        assert!(is_downloaded(dir.path(), "paddleocr-vl"));
        let layout = dir.path().join("paddleocr-vl/layout/model.safetensors");
        assert_eq!(
            std::fs::read_to_string(layout).unwrap(),
            "/PaddlePaddle/PP-DocLayoutV3_safetensors/model.safetensors"
        );
    }
}
