//! Dictionaries: English (US and UK) comes with Libreri; other languages
//! are downloaded (per computer, into the app's data folder) and removed
//! from Settings.

use serde::Serialize;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// A dictionary Libreri offers.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DictionaryInfo {
    /// "en-US", "fr", "hi-IN".
    pub code: String,
    pub name: String,
    /// About how big the download is, in KB.
    pub size_kb: u32,
    pub built_in: bool,
    pub downloaded: bool,
}

/// Where a dictionary comes from.
#[derive(Clone, Copy)]
enum Source {
    BuiltIn(&'static str, &'static str),
    /// wooorm/dictionaries: `dictionaries/<dir>/index.{aff,dic}`.
    Wooorm(&'static str),
    /// LibreOffice/dictionaries: `<dir>/<file>.{aff,dic}`.
    LibreOffice(&'static str, &'static str),
}

struct Known {
    code: &'static str,
    name: &'static str,
    size_kb: u32,
    source: Source,
}

const EN_US: Source = Source::BuiltIn(
    include_str!("../dict/en-US.aff"),
    include_str!("../dict/en-US.dic"),
);
const EN_GB: Source = Source::BuiltIn(
    include_str!("../dict/en-GB.aff"),
    include_str!("../dict/en-GB.dic"),
);

const fn w(code: &'static str, name: &'static str, size_kb: u32, dir: &'static str) -> Known {
    Known {
        code,
        name,
        size_kb,
        source: Source::Wooorm(dir),
    }
}

const KNOWN: &[Known] = &[
    Known {
        code: "en-US",
        name: "English (US)",
        size_kb: 550,
        source: EN_US,
    },
    Known {
        code: "en-GB",
        name: "English (UK)",
        size_kb: 550,
        source: EN_GB,
    },
    w("en-AU", "English (Australia)", 560, "en-AU"),
    w("en-CA", "English (Canada)", 550, "en-CA"),
    w("en-ZA", "English (South Africa)", 600, "en-ZA"),
    Known {
        code: "hi-IN",
        name: "Hindi",
        size_kb: 5000,
        source: Source::LibreOffice("hi_IN", "hi_IN"),
    },
    Known {
        code: "bn-BD",
        name: "Bengali",
        size_kb: 2240,
        source: Source::LibreOffice("bn_BD", "bn_BD"),
    },
    w("bg", "Bulgarian", 1570, "bg"),
    w("ca", "Catalan", 2880, "ca"),
    w("hr", "Croatian", 730, "hr"),
    w("cs", "Czech", 3650, "cs"),
    w("da", "Danish", 3720, "da"),
    w("nl", "Dutch", 2490, "nl"),
    w("et", "Estonian", 4460, "et"),
    w("fr", "French", 1230, "fr"),
    w("gl", "Galician", 8640, "gl"),
    w("de", "German", 1120, "de"),
    w("de-AT", "German (Austria)", 1120, "de-AT"),
    w("de-CH", "German (Switzerland)", 1120, "de-CH"),
    w("el", "Greek", 19400, "el"),
    w("he", "Hebrew", 5690, "he"),
    w("hu", "Hungarian", 3150, "hu"),
    w("is", "Icelandic", 2450, "is"),
    w("ga", "Irish", 1580, "ga"),
    w("it", "Italian", 1300, "it"),
    w("ko", "Korean", 2860, "ko"),
    w("la", "Latin", 1710, "la"),
    w("lv", "Latvian", 1840, "lv"),
    w("lt", "Lithuanian", 1180, "lt"),
    w("nb", "Norwegian (Bokmål)", 5350, "nb"),
    w("nn", "Norwegian (Nynorsk)", 3340, "nn"),
    w("fa", "Persian", 2580, "fa"),
    w("pl", "Polish", 4680, "pl"),
    w("pt", "Portuguese (Brazil)", 4480, "pt"),
    w("pt-PT", "Portuguese (Portugal)", 1480, "pt-PT"),
    w("ro", "Romanian", 2200, "ro"),
    w("ru", "Russian", 3470, "ru"),
    w("sr", "Serbian", 4450, "sr"),
    w("sk", "Slovak", 3500, "sk"),
    w("sl", "Slovenian", 3080, "sl"),
    w("es", "Spanish", 710, "es"),
    w("sv", "Swedish", 2340, "sv"),
    w("tr", "Turkish", 9120, "tr"),
    w("uk", "Ukrainian", 8490, "uk"),
    w("vi", "Vietnamese", 40, "vi"),
    w("cy", "Welsh", 890, "cy"),
];

fn known(code: &str) -> Option<&'static Known> {
    KNOWN.iter().find(|k| k.code == code)
}

fn files(dir: &Path, code: &str) -> (PathBuf, PathBuf) {
    (
        dir.join(format!("{code}.aff")),
        dir.join(format!("{code}.dic")),
    )
}

fn is_downloaded(dir: &Path, code: &str) -> bool {
    let (a, d) = files(dir, code);
    a.is_file() && d.is_file()
}

/// Every dictionary, and whether it can be used on this computer.
pub fn dictionaries(dir: &Path) -> Vec<DictionaryInfo> {
    KNOWN
        .iter()
        .map(|k| {
            let built_in = matches!(k.source, Source::BuiltIn(..));
            DictionaryInfo {
                code: k.code.to_owned(),
                name: k.name.to_owned(),
                size_kb: k.size_kb,
                built_in,
                downloaded: built_in || is_downloaded(dir, k.code),
            }
        })
        .collect()
}

/// The `.aff` and `.dic` text of a dictionary that can be used.
pub fn read(dir: &Path, code: &str) -> Result<(String, String), String> {
    let k = known(code).ok_or("unknown dictionary")?;
    if let Source::BuiltIn(aff, dic) = k.source {
        return Ok((aff.to_owned(), dic.to_owned()));
    }
    let (a, d) = files(dir, code);
    let aff = std::fs::read_to_string(a)
        .map_err(|_| format!("download the {} dictionary first", k.name))?;
    let dic = std::fs::read_to_string(d)
        .map_err(|_| format!("download the {} dictionary first", k.name))?;
    Ok((aff, dic))
}

fn urls(k: &Known) -> Vec<(String, String)> {
    let custom = std::env::var("LIBRERI_DICTIONARY_URL").ok();
    let mut out = Vec::new();
    if let Some(base) = custom {
        let b = base.trim_end_matches('/');
        out.push((format!("{b}/{}.aff", k.code), format!("{b}/{}.dic", k.code)));
    }
    match k.source {
        Source::BuiltIn(..) => {}
        Source::Wooorm(dir) => {
            let gh = format!(
                "https://raw.githubusercontent.com/wooorm/dictionaries/main/dictionaries/{dir}"
            );
            out.push((format!("{gh}/index.aff"), format!("{gh}/index.dic")));
            let cdn =
                format!("https://cdn.jsdelivr.net/gh/wooorm/dictionaries@main/dictionaries/{dir}");
            out.push((format!("{cdn}/index.aff"), format!("{cdn}/index.dic")));
        }
        Source::LibreOffice(dir, file) => {
            let gh = format!(
                "https://raw.githubusercontent.com/LibreOffice/dictionaries/master/{dir}/{file}"
            );
            out.push((format!("{gh}.aff"), format!("{gh}.dic")));
            let cdn =
                format!("https://cdn.jsdelivr.net/gh/LibreOffice/dictionaries@master/{dir}/{file}");
            out.push((format!("{cdn}.aff"), format!("{cdn}.dic")));
        }
    }
    out
}

fn fetch(
    agent: &ureq::Agent,
    url: &str,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<Vec<u8>, String> {
    let mut res = agent.get(url).call().map_err(|e| match e {
        ureq::Error::StatusCode(c) => format!("the download failed (HTTP {c})"),
        ureq::Error::HostNotFound => "the download failed; check the internet connection".into(),
        other => format!("the download failed: {other}"),
    })?;
    let mut reader = res
        .body_mut()
        .with_config()
        .limit(200 * 1024 * 1024)
        .reader();
    let mut out = Vec::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Err("cancelled".into());
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("the download stopped: {e}"))?;
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
        progress(out.len() as u64);
    }
    Ok(out)
}

/// Downloads a dictionary. `progress` gets bytes so far and the expected
/// total; `cancel` stops it.
pub fn download(
    dir: &Path,
    code: &str,
    cancel: &AtomicBool,
    mut progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let k = known(code).ok_or("unknown dictionary")?;
    if matches!(k.source, Source::BuiltIn(..)) {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_connect(Some(Duration::from_secs(20)))
        .timeout_recv_body(Some(Duration::from_secs(120)))
        .max_redirects(8)
        .build()
        .into();
    let total = u64::from(k.size_kb) * 1024;
    let mut last_error = String::from("the download failed");
    for (aff_url, dic_url) in urls(k) {
        let result = fetch(&agent, &aff_url, cancel, &mut |_| {}).and_then(|aff| {
            let dic = fetch(&agent, &dic_url, cancel, &mut |n| progress(n, total))?;
            Ok((aff, dic))
        });
        match result {
            Ok((aff, dic)) => return save(dir, code, &aff, &dic),
            Err(e) if e == "cancelled" => return Err(e),
            Err(e) => last_error = e,
        }
    }
    Err(last_error)
}

fn save(dir: &Path, code: &str, aff: &[u8], dic: &[u8]) -> Result<(), String> {
    let aff = std::str::from_utf8(aff).map_err(|_| "the dictionary is not in UTF-8".to_owned())?;
    let dic = std::str::from_utf8(dic).map_err(|_| "the dictionary is not in UTF-8".to_owned())?;
    // Make sure it can be used before keeping it.
    spellbook::Dictionary::new(aff, dic)
        .map_err(|e| format!("the dictionary could not be read: {e}"))?;
    let (a, d) = files(dir, code);
    for (path, text) in [(&a, aff), (&d, dic)] {
        let tmp = path.with_extension("download");
        let mut f = std::fs::File::create(&tmp).map_err(|e| e.to_string())?;
        f.write_all(text.as_bytes()).map_err(|e| e.to_string())?;
        drop(f);
        std::fs::rename(&tmp, path).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Removes a downloaded dictionary (built-in ones stay).
pub fn remove(dir: &Path, code: &str) -> Result<(), String> {
    let k = known(code).ok_or("unknown dictionary")?;
    if matches!(k.source, Source::BuiltIn(..)) {
        return Err("the English dictionaries come with Libreri".into());
    }
    let (a, d) = files(dir, code);
    for p in [a, d] {
        match std::fs::remove_file(p) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.to_string()),
        }
    }
    Ok(())
}

/// The stems of a `.dic` file (the words before any affix flags), for
/// completing words.
pub fn stems(dic: &str) -> Vec<String> {
    dic.lines()
        .skip(1)
        .filter_map(|l| {
            let w = l.split(['/', '\t']).next()?.trim();
            (w.chars().count() > 1
                && w.chars()
                    .all(|c| c.is_alphabetic() || c == '\'' || c == '-'))
            .then(|| w.to_owned())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn english_comes_with_libreri() {
        let dir = tempfile::tempdir().unwrap();
        let list = dictionaries(dir.path());
        let us = list.iter().find(|d| d.code == "en-US").unwrap();
        assert!(us.built_in && us.downloaded);
        assert!(!list.iter().find(|d| d.code == "fr").unwrap().downloaded);
        let (aff, dic) = read(dir.path(), "en-GB").unwrap();
        let d = spellbook::Dictionary::new(&aff, &dic).unwrap();
        assert!(d.check("colour") && d.check("organise"));
        assert!(read(dir.path(), "fr").is_err());
        assert!(remove(dir.path(), "en-US").is_err());
        let s = stems(&dic);
        assert!(s.len() > 40_000 && s.iter().any(|w| w == "lighthouse"));
    }

    #[test]
    fn keeps_a_downloaded_dictionary() {
        let dir = tempfile::tempdir().unwrap();
        let (aff, dic) = read(dir.path(), "en-US").unwrap();
        save(dir.path(), "fr", aff.as_bytes(), dic.as_bytes()).unwrap();
        assert!(
            dictionaries(dir.path())
                .iter()
                .find(|d| d.code == "fr")
                .unwrap()
                .downloaded
        );
        assert!(save(dir.path(), "de", &[0xff, 0xfe], b"1\nx").is_err());
        remove(dir.path(), "fr").unwrap();
        assert!(!is_downloaded(dir.path(), "fr"));
    }

    /// With the network: `LIBRERI_TEST_DICTIONARY=fr cargo test -p libreri-spell real -- --nocapture`.
    #[test]
    fn real() {
        let Ok(code) = std::env::var("LIBRERI_TEST_DICTIONARY") else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        download(dir.path(), &code, &AtomicBool::new(false), |n, t| {
            if n % (512 * 1024) < 65_536 {
                println!("{n} / {t}");
            }
        })
        .unwrap();
        let (aff, dic) = read(dir.path(), &code).unwrap();
        spellbook::Dictionary::new(&aff, &dic).unwrap();
    }
}
