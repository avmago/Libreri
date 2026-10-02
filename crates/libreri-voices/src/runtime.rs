//! ONNX Runtime (MIT, Microsoft), which runs the voices. It is downloaded
//! with the first voice rather than built into Libreri, so the app stays
//! small for people who never use natural voices.

use crate::fetch::{self, Get};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::OnceLock;

/// The release used: the newest with builds for Intel Macs too.
pub const VERSION: &str = "1.23.2";
const RELEASES: &str = "https://github.com/microsoft/onnxruntime/releases/download";

/// The archive for this computer, and the library inside it.
fn package() -> Option<(String, &'static str)> {
    let v = VERSION;
    let (name, lib) = if cfg!(target_os = "macos") {
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x86_64"
        };
        (
            format!("onnxruntime-osx-{arch}-{v}.tgz"),
            "libonnxruntime.1.23.2.dylib",
        )
    } else if cfg!(windows) {
        let arch = if cfg!(target_arch = "aarch64") {
            "arm64"
        } else {
            "x64"
        };
        (format!("onnxruntime-win-{arch}-{v}.zip"), "onnxruntime.dll")
    } else if cfg!(target_os = "linux") {
        let arch = if cfg!(target_arch = "aarch64") {
            "aarch64"
        } else {
            "x64"
        };
        (
            format!("onnxruntime-linux-{arch}-{v}.tgz"),
            "libonnxruntime.so.1.23.2",
        )
    } else {
        return None;
    };
    Some((format!("{RELEASES}/v{v}/{name}"), lib))
}

fn folder(dir: &Path) -> PathBuf {
    dir.join("runtime")
}

/// The library, once downloaded (or `LIBRERI_ORT_LIB` for tests).
pub fn library(dir: &Path) -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("LIBRERI_ORT_LIB") {
        return Some(PathBuf::from(p));
    }
    let (_, lib) = package()?;
    let p = folder(dir).join(lib);
    p.is_file().then_some(p)
}

/// About how much the download is, in bytes.
pub fn size() -> u64 {
    if cfg!(windows) {
        14_000_000
    } else if cfg!(target_os = "macos") {
        12_000_000
    } else {
        8_500_000
    }
}

/// Downloads the runtime unless it is here.
pub fn ensure(
    dir: &Path,
    cancel: &AtomicBool,
    progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    if library(dir).is_some() {
        return Ok(());
    }
    let (url, lib) = package().ok_or("natural voices are not offered on this system")?;
    let root = folder(dir);
    let archive = root.join(url.rsplit('/').next().unwrap_or("runtime"));
    fetch::files(
        &[Get {
            url: url.clone(),
            to: archive.clone(),
        }],
        size(),
        cancel,
        progress,
    )?;
    let bytes = extract(&archive, lib)?;
    let tmp = root.join(format!(".{lib}.part"));
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, root.join(lib)).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&archive);
    Ok(())
}

/// The library file out of a .tgz or .zip.
fn extract(archive: &Path, lib: &str) -> Result<Vec<u8>, String> {
    let file = std::fs::File::open(archive).map_err(|e| e.to_string())?;
    let ends = |name: &str| name.ends_with(&format!("/lib/{lib}"));
    if archive.extension().is_some_and(|e| e == "zip") {
        let mut z = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
        for i in 0..z.len() {
            let mut f = z.by_index(i).map_err(|e| e.to_string())?;
            if ends(f.name()) {
                let mut out = Vec::new();
                f.read_to_end(&mut out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
        }
    } else {
        let mut t = tar::Archive::new(flate2::read::GzDecoder::new(file));
        for entry in t.entries().map_err(|e| e.to_string())? {
            let mut e = entry.map_err(|e| e.to_string())?;
            let name = e
                .path()
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .into_owned();
            if ends(&name) {
                let mut out = Vec::new();
                e.read_to_end(&mut out).map_err(|e| e.to_string())?;
                return Ok(out);
            }
        }
    }
    Err("the ONNX Runtime download did not have its library".into())
}

/// Removes the runtime (when no voice is left).
pub fn remove(dir: &Path) -> Result<(), String> {
    fetch::remove_dir(&folder(dir))
}

static LOADED: OnceLock<Result<(), String>> = OnceLock::new();

/// Loads the runtime once for the whole app.
pub fn load(dir: &Path) -> Result<(), String> {
    LOADED
        .get_or_init(|| {
            let lib = library(dir).ok_or("ONNX Runtime is not downloaded")?;
            let builder = ort::init_from(&lib)
                .map_err(|e| format!("ONNX Runtime could not be loaded: {e}"))?;
            builder.with_name("libreri-voices").commit();
            Ok(())
        })
        .clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_this_computer() {
        let (url, lib) = package().unwrap();
        assert!(url.starts_with(RELEASES) && url.contains(VERSION));
        assert!(lib.contains("onnxruntime"));
    }

    #[test]
    fn takes_the_library_out_of_a_tgz() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("r.tgz");
        let gz = flate2::write::GzEncoder::new(
            std::fs::File::create(&path).unwrap(),
            flate2::Compression::fast(),
        );
        let mut b = tar::Builder::new(gz);
        let mut h = tar::Header::new_gnu();
        h.set_size(3);
        h.set_cksum();
        b.append_data(&mut h, "onnxruntime-x/lib/libx.so", &b"abc"[..])
            .unwrap();
        b.into_inner().unwrap().finish().unwrap();
        assert_eq!(extract(&path, "libx.so").unwrap(), b"abc");
        assert!(extract(&path, "liby.so").is_err());
    }
}
