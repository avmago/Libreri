//! Reading handwriting with the system's own recogniser: Apple's
//! Vision framework on macOS (it reads handwriting as well as print) and
//! Windows' text recognition. Both are driven by a small script run by the
//! system's own tools (osascript, Windows PowerShell), so nothing native is
//! built into Libreri and a missing recogniser only affects this feature.
//! Linux has none; Tesseract is used there.

use std::path::Path;

/// The system recogniser's name, where there is one.
pub fn system_name() -> Option<&'static str> {
    if cfg!(target_os = "macos") {
        Some("macOS handwriting recognition")
    } else if cfg!(windows) {
        Some("Windows text recognition")
    } else {
        None
    }
}

/// Apple Vision through JavaScript for Automation. Arguments: the image
/// path and, optionally, comma-separated languages ("en-US,fr-FR").
const MAC_SCRIPT: &str = r#"
ObjC.import('Foundation');
ObjC.import('Vision');
function read(url, langs) {
  const req = $.VNRecognizeTextRequest.alloc.init;
  req.recognitionLevel = 0;
  req.usesLanguageCorrection = true;
  if (langs.length) req.recognitionLanguages = $(langs);
  else if (req.respondsToSelector('setAutomaticallyDetectsLanguage:')) req.automaticallyDetectsLanguage = true;
  const handler = $.VNImageRequestHandler.alloc.initWithURLOptions(url, $({}));
  if (!handler.performRequestsError($([req]), null)) throw new Error('Vision could not read the image');
  const out = [];
  const results = req.results;
  for (let i = 0; i < results.count; i++) {
    const obs = results.objectAtIndex(i);
    const cand = obs.topCandidates(1);
    if (cand.count === 0) continue;
    const box = obs.boundingBox;
    out.push({ y: box.origin.y + box.size.height / 2, x: box.origin.x, text: cand.objectAtIndex(0).string.js });
  }
  // Top to bottom (Vision's y grows upwards), then left to right.
  out.sort((a, b) => (Math.abs(a.y - b.y) > 0.02 ? b.y - a.y : a.x - b.x));
  return out.map((o) => o.text).join('\n');
}
function run(argv) {
  const url = $.NSURL.fileURLWithPath(argv[0]);
  const langs = argv[1] ? argv[1].split(',') : [];
  try {
    return read(url, langs);
  } catch (e) {
    if (langs.length) return read(url, []);
    throw e;
  }
}
"#;

/// Windows.Media.Ocr through Windows PowerShell. Arguments: the image path
/// and, optionally, a language tag ("en-US").
const WINDOWS_SCRIPT: &str = r#"
param([string]$Path, [string]$Lang)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName System.Runtime.WindowsRuntime
$null = [Windows.Storage.StorageFile, Windows.Storage, ContentType = WindowsRuntime]
$null = [Windows.Media.Ocr.OcrEngine, Windows.Foundation, ContentType = WindowsRuntime]
$null = [Windows.Graphics.Imaging.BitmapDecoder, Windows.Graphics, ContentType = WindowsRuntime]
$null = [Windows.Globalization.Language, Windows.Globalization, ContentType = WindowsRuntime]
$asTask = ([System.WindowsRuntimeSystemExtensions].GetMethods() | Where-Object {
  $_.Name -eq 'AsTask' -and $_.GetParameters().Count -eq 1 -and $_.GetParameters()[0].ParameterType.Name -eq 'IAsyncOperation`1'
})[0]
function Await($op, [Type]$type) {
  $t = $asTask.MakeGenericMethod($type).Invoke($null, @($op))
  $null = $t.Wait(-1)
  $t.Result
}
$file = Await ([Windows.Storage.StorageFile]::GetFileFromPathAsync($Path)) ([Windows.Storage.StorageFile])
$stream = Await ($file.OpenAsync([Windows.Storage.FileAccessMode]::Read)) ([Windows.Storage.Streams.IRandomAccessStream])
$decoder = Await ([Windows.Graphics.Imaging.BitmapDecoder]::CreateAsync($stream)) ([Windows.Graphics.Imaging.BitmapDecoder])
$bitmap = Await ($decoder.GetSoftwareBitmapAsync()) ([Windows.Graphics.Imaging.SoftwareBitmap])
$engine = $null
if ($Lang) { try { $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromLanguage([Windows.Globalization.Language]::new($Lang)) } catch { } }
if (-not $engine) { $engine = [Windows.Media.Ocr.OcrEngine]::TryCreateFromUserProfileLanguages() }
if (-not $engine) { throw 'No text recognition language is installed in Windows.' }
$result = Await ($engine.RecognizeAsync($bitmap)) ([Windows.Media.Ocr.OcrResult])
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
($result.Lines | ForEach-Object { $_.Text }) -join "`n"
"#;

/// A language tag the system recognisers know, from a two-letter code.
pub fn system_language(code: &str) -> Option<&'static str> {
    let two = code.split(['-', '_']).next()?.to_ascii_lowercase();
    Some(match two.as_str() {
        "en" => "en-US",
        "fr" => "fr-FR",
        "de" => "de-DE",
        "es" => "es-ES",
        "it" => "it-IT",
        "pt" => "pt-BR",
        "nl" => "nl-NL",
        "sv" => "sv-SE",
        "da" => "da-DK",
        "nb" | "no" => "nb-NO",
        "pl" => "pl-PL",
        "ru" => "ru-RU",
        "uk" => "uk-UA",
        "zh" => "zh-Hans",
        "ja" => "ja-JP",
        "ko" => "ko-KR",
        _ => return None,
    })
}

/// How long the system's recogniser may take.
const RECOGNISE_TIME: std::time::Duration = std::time::Duration::from_secs(60);

/// A temporary script file with a name of its own, so two readings at once
/// never share one; removed when dropped.
struct TempScript(std::path::PathBuf);

impl TempScript {
    fn new(ext: &str, contents: &[u8]) -> Result<Self, String> {
        static COUNT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "libreri-ink-{}-{nanos}-{n}.{ext}",
            std::process::id()
        ));
        std::fs::write(&path, contents).map_err(|e| e.to_string())?;
        Ok(Self(path))
    }
}

impl Drop for TempScript {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// Runs `cmd` and returns what it printed, stopping it after `limit`.
fn run(mut cmd: std::process::Command, limit: std::time::Duration) -> Result<String, String> {
    use std::io::Read;
    use std::process::Stdio;
    let mut child = cmd
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("the system's handwriting recognition could not start: {e}"))?;
    let drain = |pipe: Option<Box<dyn Read + Send>>| {
        std::thread::spawn(move || {
            let mut buf = Vec::new();
            if let Some(mut p) = pipe {
                let _ = p.read_to_end(&mut buf);
            }
            buf
        })
    };
    let stdout = drain(
        child
            .stdout
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let stderr = drain(
        child
            .stderr
            .take()
            .map(|p| Box::new(p) as Box<dyn Read + Send>),
    );
    let until = std::time::Instant::now() + limit;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if std::time::Instant::now() < until => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err("the system's handwriting recognition took too long".into());
            }
            Err(e) => return Err(e.to_string()),
        }
    };
    let out_bytes = stdout.join().unwrap_or_default();
    let err_bytes = stderr.join().unwrap_or_default();
    if !status.success() {
        let err = String::from_utf8_lossy(&err_bytes);
        let line = err
            .lines()
            .rev()
            .find(|l| !l.trim().is_empty())
            .unwrap_or("unknown error");
        return Err(format!(
            "the system could not read the handwriting: {}",
            line.trim()
        ));
    }
    Ok(String::from_utf8_lossy(&out_bytes).trim().to_owned())
}

/// Reads the text in an image with the system's recogniser.
pub fn recognize_system(image: &Path, language: Option<&str>) -> Result<String, String> {
    let lang = language.and_then(system_language).unwrap_or("");
    if cfg!(target_os = "macos") {
        let script = TempScript::new("js", MAC_SCRIPT.as_bytes())?;
        let mut cmd = std::process::Command::new("/usr/bin/osascript");
        cmd.args(["-l", "JavaScript"])
            .arg(&script.0)
            .arg(image)
            .arg(lang);
        run(cmd, RECOGNISE_TIME)
    } else if cfg!(windows) {
        // A byte-order mark so Windows PowerShell reads the script as UTF-8.
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(WINDOWS_SCRIPT.as_bytes());
        let script = TempScript::new("ps1", &bytes)?;
        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script.0)
        .arg(image)
        .arg(lang);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // no console window
        }
        run(cmd, RECOGNISE_TIME)
    } else {
        Err("this system has no handwriting recognition of its own; use Tesseract".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_languages() {
        assert_eq!(system_language("en"), Some("en-US"));
        assert_eq!(system_language("fr-CA"), Some("fr-FR"));
        assert_eq!(system_language("xx"), None);
        if cfg!(target_os = "linux") {
            assert!(system_name().is_none());
            assert!(recognize_system(Path::new("x.png"), None).is_err());
        }
    }

    #[test]
    fn temporary_scripts_have_names_of_their_own() {
        let a = TempScript::new("js", b"1").unwrap();
        let b = TempScript::new("js", b"2").unwrap();
        assert_ne!(a.0, b.0);
        let path = a.0.clone();
        drop(a);
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn a_recogniser_that_hangs_is_stopped() {
        let mut cmd = std::process::Command::new("sleep");
        cmd.arg("10");
        let started = std::time::Instant::now();
        let e = run(cmd, std::time::Duration::from_millis(200)).unwrap_err();
        assert!(e.contains("too long"), "{e}");
        assert!(started.elapsed() < std::time::Duration::from_secs(5));
        let mut echo = std::process::Command::new("echo");
        echo.arg("hello");
        assert_eq!(run(echo, RECOGNISE_TIME).unwrap(), "hello");
    }
}
