//! Language data for Tesseract (OCR).
//!
//! Tesseract needs one `<code>.traineddata` file per language. Package
//! managers install English (and sometimes more); Libreri downloads others
//! on request from the Tesseract project's `tessdata_fast` set into its own
//! folder on this computer. When a book needs languages from both places,
//! the system's files are copied next to the downloaded ones, because
//! Tesseract reads from a single folder.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Where language files are downloaded from (`{code}` is replaced).
const SOURCE: &str = "https://github.com/tesseract-ocr/tessdata_fast/raw/main/{code}.traineddata";

/// Languages offered for download: Tesseract code and English name.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("eng", "English"),
    ("ara", "Arabic"),
    ("ben", "Bengali"),
    ("bul", "Bulgarian"),
    ("cat", "Catalan"),
    ("ces", "Czech"),
    ("chi_sim", "Chinese (simplified)"),
    ("chi_tra", "Chinese (traditional)"),
    ("dan", "Danish"),
    ("deu", "German"),
    ("ell", "Greek"),
    ("est", "Estonian"),
    ("fas", "Persian"),
    ("fin", "Finnish"),
    ("fra", "French"),
    ("grc", "Ancient Greek"),
    ("guj", "Gujarati"),
    ("heb", "Hebrew"),
    ("hin", "Hindi"),
    ("hrv", "Croatian"),
    ("hun", "Hungarian"),
    ("ind", "Indonesian"),
    ("isl", "Icelandic"),
    ("ita", "Italian"),
    ("jpn", "Japanese"),
    ("kan", "Kannada"),
    ("kor", "Korean"),
    ("lat", "Latin"),
    ("lav", "Latvian"),
    ("lit", "Lithuanian"),
    ("mal", "Malayalam"),
    ("mar", "Marathi"),
    ("msa", "Malay"),
    ("nld", "Dutch"),
    ("nor", "Norwegian"),
    ("pan", "Punjabi"),
    ("pol", "Polish"),
    ("por", "Portuguese"),
    ("ron", "Romanian"),
    ("rus", "Russian"),
    ("san", "Sanskrit"),
    ("slk", "Slovak"),
    ("slv", "Slovenian"),
    ("spa", "Spanish"),
    ("srp", "Serbian"),
    ("swe", "Swedish"),
    ("tam", "Tamil"),
    ("tel", "Telugu"),
    ("tha", "Thai"),
    ("tur", "Turkish"),
    ("ukr", "Ukrainian"),
    ("urd", "Urdu"),
    ("vie", "Vietnamese"),
];

/// The English name of a Tesseract language code.
pub fn language_name(code: &str) -> Option<&'static str> {
    LANGUAGES.iter().find(|(c, _)| *c == code).map(|(_, n)| *n)
}

/// The Tesseract code for a book's language ("en", "fr-CA", "fre",
/// "German").
pub fn from_book_language(lang: &str) -> Option<&'static str> {
    let l = lang.trim().to_ascii_lowercase();
    let short = l.split(['-', '_']).next().unwrap_or("");
    let map: &[(&[&str], &str)] = &[
        (&["en", "eng", "english"], "eng"),
        (&["ar", "ara", "arabic"], "ara"),
        (&["bn", "ben", "bengali"], "ben"),
        (&["bg", "bul", "bulgarian"], "bul"),
        (&["ca", "cat", "catalan"], "cat"),
        (&["cs", "ces", "cze", "czech"], "ces"),
        (&["zh", "chi", "zho", "chinese"], "chi_sim"),
        (&["da", "dan", "danish"], "dan"),
        (&["de", "deu", "ger", "german"], "deu"),
        (&["el", "ell", "gre", "greek"], "ell"),
        (&["et", "est", "estonian"], "est"),
        (&["fa", "fas", "per", "persian"], "fas"),
        (&["fi", "fin", "finnish"], "fin"),
        (&["fr", "fra", "fre", "french"], "fra"),
        (&["grc"], "grc"),
        (&["gu", "guj", "gujarati"], "guj"),
        (&["he", "heb", "hebrew"], "heb"),
        (&["hi", "hin", "hindi"], "hin"),
        (&["hr", "hrv", "croatian"], "hrv"),
        (&["hu", "hun", "hungarian"], "hun"),
        (&["id", "ind", "indonesian"], "ind"),
        (&["is", "isl", "ice", "icelandic"], "isl"),
        (&["it", "ita", "italian"], "ita"),
        (&["ja", "jpn", "japanese"], "jpn"),
        (&["kn", "kan", "kannada"], "kan"),
        (&["ko", "kor", "korean"], "kor"),
        (&["la", "lat", "latin"], "lat"),
        (&["lv", "lav", "latvian"], "lav"),
        (&["lt", "lit", "lithuanian"], "lit"),
        (&["ml", "mal", "malayalam"], "mal"),
        (&["mr", "mar", "marathi"], "mar"),
        (&["ms", "msa", "may", "malay"], "msa"),
        (&["nl", "nld", "dut", "dutch"], "nld"),
        (&["no", "nb", "nn", "nor", "norwegian"], "nor"),
        (&["pa", "pan", "punjabi"], "pan"),
        (&["pl", "pol", "polish"], "pol"),
        (&["pt", "por", "portuguese"], "por"),
        (&["ro", "ron", "rum", "romanian"], "ron"),
        (&["ru", "rus", "russian"], "rus"),
        (&["sa", "san", "sanskrit"], "san"),
        (&["sk", "slk", "slo", "slovak"], "slk"),
        (&["sl", "slv", "slovenian"], "slv"),
        (&["es", "spa", "spanish"], "spa"),
        (&["sr", "srp", "serbian"], "srp"),
        (&["sv", "swe", "swedish"], "swe"),
        (&["ta", "tam", "tamil"], "tam"),
        (&["te", "tel", "telugu"], "tel"),
        (&["th", "tha", "thai"], "tha"),
        (&["tr", "tur", "turkish"], "tur"),
        (&["uk", "ukr", "ukrainian"], "ukr"),
        (&["ur", "urd", "urdu"], "urd"),
        (&["vi", "vie", "vietnamese"], "vie"),
    ];
    if l == "zh-tw" || l == "zh-hant" || l == "zh_tw" {
        return Some("chi_tra");
    }
    map.iter()
        .find(|(names, _)| names.contains(&short) || names.contains(&l.as_str()))
        .map(|(_, code)| *code)
}

/// A language Tesseract can read on this computer, or could after a
/// download.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OcrLanguage {
    pub code: String,
    pub name: String,
    /// Came with Tesseract.
    pub built_in: bool,
    /// Downloaded by Libreri.
    pub downloaded: bool,
}

fn is_language_code(code: &str) -> bool {
    !code.is_empty()
        && code.len() <= 24
        && code
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}

/// Tesseract's own data folder and the languages in it.
pub fn system_languages() -> (Option<PathBuf>, Vec<String>) {
    let Some(mut cmd) = crate::command("tesseract") else {
        return (None, Vec::new());
    };
    let Ok(out) = cmd.arg("--list-langs").output() else {
        return (None, Vec::new());
    };
    // Older versions print the list on stderr.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let mut dir = None;
    let mut langs = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("List of available languages") {
            dir = line
                .split('"')
                .nth(1)
                .map(PathBuf::from)
                .filter(|p| p.is_dir());
        } else if is_language_code(line) && line != "osd" {
            langs.push(line.to_owned());
        }
    }
    (dir, langs)
}

/// Languages downloaded into `dir`.
pub fn downloaded(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let code = name.strip_suffix(".traineddata")?;
            (is_language_code(code) && code != "osd").then(|| code.to_owned())
        })
        .collect();
    out.sort();
    out
}

/// Every language usable now plus the ones offered for download.
pub fn languages(dir: &Path) -> Vec<OcrLanguage> {
    let (_, system) = system_languages();
    let mine = downloaded(dir);
    let mut out: Vec<OcrLanguage> = LANGUAGES
        .iter()
        .map(|(code, name)| OcrLanguage {
            code: (*code).into(),
            name: (*name).into(),
            built_in: system.iter().any(|s| s == code),
            downloaded: mine.iter().any(|s| s == code),
        })
        .collect();
    // Languages Tesseract has that Libreri does not list.
    for code in system.iter().chain(&mine) {
        if !out.iter().any(|l| &l.code == code) {
            out.push(OcrLanguage {
                code: code.clone(),
                name: code.clone(),
                built_in: system.contains(code),
                downloaded: mine.contains(code),
            });
        }
    }
    out
}

/// Makes sure Tesseract can read `langs` and says which data folder to
/// give it: `None` for its own, or `dir` with the system's files copied in.
pub fn prepare(dir: &Path, langs: &[String]) -> Result<Option<PathBuf>, String> {
    let (system_dir, system) = system_languages();
    let mine = downloaded(dir);
    let missing: Vec<&str> = langs
        .iter()
        .filter(|l| !system.contains(l) && !mine.contains(l))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        let names: Vec<&str> = missing
            .iter()
            .map(|c| language_name(c).unwrap_or(c))
            .collect();
        return Err(format!(
            "OCR needs the language data for {}. Get it in Settings › Helper programs.",
            names.join(", ")
        ));
    }
    if langs.iter().all(|l| system.contains(l)) {
        return Ok(None);
    }
    let system_dir = system_dir.ok_or("Tesseract's own language folder could not be found")?;
    for l in langs {
        if !mine.contains(l) {
            let file = format!("{l}.traineddata");
            std::fs::copy(system_dir.join(&file), dir.join(&file))
                .map_err(|e| format!("could not copy {file}: {e}"))?;
        }
    }
    Ok(Some(dir.to_path_buf()))
}

/// Downloads one language into `dir`. `progress` gets bytes so far and the
/// total when known.
pub fn download(
    dir: &Path,
    code: &str,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<(), String> {
    if !is_language_code(code) {
        return Err("unknown language".into());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let url = std::env::var("LIBRERI_TESSDATA_URL")
        .unwrap_or_else(|_| SOURCE.to_owned())
        .replace("{code}", code);
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(60)))
        .max_redirects(5)
        .build()
        .into();
    let mut res = agent.get(&url).call().map_err(|e| match e {
        ureq::Error::StatusCode(404) => "that language is not available".to_owned(),
        ureq::Error::StatusCode(c) => format!("the download failed (HTTP {c})"),
        ureq::Error::HostNotFound => "the download failed; check the internet connection".into(),
        other => format!("the download failed: {other}"),
    })?;
    let total = res
        .headers()
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    let tmp = dir.join(format!(".{code}.download"));
    let mut file = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
    let mut reader = res
        .body_mut()
        .with_config()
        .limit(200 * 1024 * 1024)
        .reader();
    let mut buf = vec![0u8; 64 * 1024];
    let mut done = 0u64;
    let mut head = Vec::new();
    loop {
        let n = reader.read(&mut buf).map_err(|e| {
            let _ = std::fs::remove_file(&tmp);
            format!("the download stopped: {e}")
        })?;
        if n == 0 {
            break;
        }
        if head.len() < 64 {
            head.extend_from_slice(&buf[..n.min(64)]);
        }
        file.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        done += n as u64;
        progress(done, total);
    }
    drop(file);
    // An HTML error page or a tiny file is not language data.
    let looks_html = head.iter().take(64).any(|&b| b == b'<')
        && String::from_utf8_lossy(&head)
            .to_lowercase()
            .contains("html");
    if done < 10_000 || looks_html {
        let _ = std::fs::remove_file(&tmp);
        return Err("the download was not language data".into());
    }
    std::fs::rename(&tmp, dir.join(format!("{code}.traineddata"))).map_err(|e| e.to_string())
}

/// Removes a downloaded language.
pub fn remove(dir: &Path, code: &str) -> Result<(), String> {
    if !is_language_code(code) {
        return Err("unknown language".into());
    }
    match std::fs::remove_file(dir.join(format!("{code}.traineddata"))) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.to_string()),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn book_languages_map_to_codes() {
        assert_eq!(from_book_language("en"), Some("eng"));
        assert_eq!(from_book_language("en-GB"), Some("eng"));
        assert_eq!(from_book_language("fre"), Some("fra"));
        assert_eq!(from_book_language("German"), Some("deu"));
        assert_eq!(from_book_language("zh-TW"), Some("chi_tra"));
        assert_eq!(from_book_language("xx"), None);
    }

    #[test]
    fn downloaded_and_prepared() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("fra.traineddata"), b"x").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"x").unwrap();
        assert_eq!(downloaded(dir.path()), vec!["fra"]);
        let err = prepare(dir.path(), &["zzz".into()]).unwrap_err();
        assert!(err.contains("zzz"), "{err}");
        let (_, system) = system_languages();
        if system.contains(&"eng".to_owned()) {
            assert_eq!(prepare(dir.path(), &["eng".into()]).unwrap(), None);
            let used = prepare(dir.path(), &["eng".into(), "fra".into()]).unwrap();
            assert_eq!(used.as_deref(), Some(dir.path()));
            assert!(dir.path().join("eng.traineddata").is_file());
        }
        remove(dir.path(), "fra").unwrap();
        assert!(!downloaded(dir.path()).contains(&"fra".to_owned()));
        assert!(remove(dir.path(), "../x").is_err());
    }

    #[test]
    fn downloads_from_a_mirror() {
        // A local file server stands in for the real source.
        let dir = tempfile::tempdir().unwrap();
        let server = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = server.local_addr().unwrap().port();
        let handle = std::thread::spawn(move || {
            let (mut s, _) = server.accept().unwrap();
            let mut req = [0u8; 1024];
            let _ = s.read(&mut req).unwrap();
            let body = vec![7u8; 20_000];
            let head = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n",
                body.len()
            );
            s.write_all(head.as_bytes()).unwrap();
            s.write_all(&body).unwrap();
        });
        std::env::set_var(
            "LIBRERI_TESSDATA_URL",
            format!("http://127.0.0.1:{port}/{{code}}.traineddata"),
        );
        let mut last = 0;
        download(dir.path(), "ita", |d, _| last = d).unwrap();
        std::env::remove_var("LIBRERI_TESSDATA_URL");
        handle.join().unwrap();
        assert_eq!(last, 20_000);
        assert_eq!(downloaded(dir.path()), vec!["ita"]);
    }
}
