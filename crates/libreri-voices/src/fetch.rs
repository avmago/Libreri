//! Downloads with progress, cancelling and resuming whole files.

use libreri_helpers::download::{self, StallReader};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub(crate) fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(download::BODY_LIMIT))
        .max_redirects(8)
        .build()
        .into()
}

fn http_error(e: ureq::Error, what: &str) -> String {
    match e {
        ureq::Error::StatusCode(c) => format!("the download of {what} failed (HTTP {c})"),
        ureq::Error::HostNotFound => "the download failed; check the internet connection".into(),
        other => format!("the download failed: {other}"),
    }
}

/// A small file (a list or a note), as text.
pub(crate) fn text(url: &str) -> Result<String, String> {
    agent()
        .get(url)
        .call()
        .map_err(|e| http_error(e, url))?
        .into_body()
        .read_to_string()
        .map_err(|e| format!("the download failed: {e}"))
}

/// One file to fetch.
pub(crate) struct Get {
    pub url: String,
    pub to: PathBuf,
}

/// Fetches `files` one after another. `progress` gets bytes so far and
/// about how many in all (`total`, or more once known). Files already here
/// whole are kept, so a stopped download goes on where it was.
pub(crate) fn files(
    files: &[Get],
    total: u64,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let agent = agent();
    let mut done = 0u64;
    for f in files {
        let name =
            f.to.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
        let folder = f.to.parent().ok_or("bad path")?;
        std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        let res = agent.get(&f.url).call().map_err(|e| http_error(e, &name))?;
        let length: Option<u64> = res
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok());
        if let (Some(len), Ok(meta)) = (length, std::fs::metadata(&f.to)) {
            if meta.len() == len {
                done += len;
                progress(done, total.max(done));
                continue;
            }
        }
        let tmp = folder.join(format!(".{name}.part"));
        let fail = |msg: String| {
            let _ = std::fs::remove_file(&tmp);
            msg
        };
        let body = res
            .into_body()
            .into_with_config()
            .limit(4 * 1024 * 1024 * 1024)
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
            return Err(fail(format!("{name} did not arrive whole")));
        }
        std::fs::rename(&tmp, &f.to).map_err(|e| fail(e.to_string()))?;
    }
    Ok(())
}

/// Removes a folder; a missing one is fine.
pub(crate) fn remove_dir(dir: &Path) -> Result<(), String> {
    match std::fs::remove_dir_all(dir) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}
