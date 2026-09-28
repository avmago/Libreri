//! Reading handwriting with the system's own recogniser (Phase 8a): Apple's
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

fn run(mut cmd: std::process::Command) -> Result<String, String> {
    let out = cmd
        .output()
        .map_err(|e| format!("the system's handwriting recognition could not start: {e}"))?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
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
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
}

/// Reads the text in an image with the system's recogniser.
pub fn recognize_system(image: &Path, language: Option<&str>) -> Result<String, String> {
    let lang = language.and_then(system_language).unwrap_or("");
    let dir = std::env::temp_dir();
    if cfg!(target_os = "macos") {
        let script = dir.join("libreri-ink.js");
        std::fs::write(&script, MAC_SCRIPT).map_err(|e| e.to_string())?;
        let mut cmd = std::process::Command::new("/usr/bin/osascript");
        cmd.args(["-l", "JavaScript"])
            .arg(&script)
            .arg(image)
            .arg(lang);
        run(cmd)
    } else if cfg!(windows) {
        let script = dir.join("libreri-ink.ps1");
        // A byte-order mark so Windows PowerShell reads the script as UTF-8.
        let mut bytes = vec![0xEF, 0xBB, 0xBF];
        bytes.extend_from_slice(WINDOWS_SCRIPT.as_bytes());
        std::fs::write(&script, bytes).map_err(|e| e.to_string())?;
        let mut cmd = std::process::Command::new("powershell.exe");
        cmd.args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .arg(image)
        .arg(lang);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x0800_0000); // no console window
        }
        run(cmd)
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
}
