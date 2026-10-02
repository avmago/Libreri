//! Natural voices for reading aloud (ADR 0030), downloaded on request in
//! Settings › Reader, run on this computer, removed with one click.
//!
//! - **Kokoro 82M** (Apache 2.0): 54 voices in English (US and UK),
//!   Spanish, French, Hindi, Italian, Portuguese, Japanese and Chinese,
//!   from one ~350 MB download.
//! - **Piper**: one small model per voice, in 40+ languages. Only voices
//!   whose licence lets anyone use them are offered.
//!
//! Both run with ONNX Runtime (MIT), downloaded with the first voice and
//! loaded at run time. Words become phonemes with eSpeak NG, run as a
//! separate helper program. Everything lives in `<app data>/voices/`:
//! `runtime/`, `kokoro/` and `piper/<voice>/`.

mod fetch;
pub mod kokoro;
pub mod phonemes;
pub mod piper;
pub mod runtime;
mod wav;

use serde::Serialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

pub use wav::encode as wav;

/// The note written when a download is whole.
const COMPLETE: &str = ".complete";
/// The id of the Kokoro download (its voices are "kokoro:af_heart").
pub const KOKORO: &str = "kokoro";

/// A voice that can read aloud now.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NaturalVoice {
    /// "kokoro:af_heart" or "piper:de_DE-thorsten-high".
    pub id: String,
    pub name: String,
    /// BCP 47: "en-US", "de-DE".
    pub lang: String,
    /// "English (US)".
    pub language: String,
    /// "kokoro" or "piper": the download it belongs to.
    pub pack: String,
    pub gender: Option<String>,
    /// Piper's "x_low", "low", "medium" or "high".
    pub quality: Option<String>,
}

/// Kokoro, as Settings shows it.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KokoroInfo {
    pub downloaded: bool,
    /// In MB, with the runtime when that is not here yet.
    pub size_mb: u32,
    pub licence: String,
    pub homepage: String,
    pub voices: u32,
}

/// A Piper voice of the collection.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiperVoiceInfo {
    /// "piper:de_DE-thorsten-high".
    pub id: String,
    pub name: String,
    pub lang: String,
    pub language: String,
    pub quality: String,
    pub size_mb: u32,
    pub licence: String,
    pub downloaded: bool,
}

/// A language of the Piper collection.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiperLanguage {
    /// "de_DE".
    pub code: String,
    pub name: String,
    pub voices: u32,
}

/// The Piper voices of one language that are free to use, and how many
/// were left out.
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PiperList {
    pub voices: Vec<PiperVoiceInfo>,
    pub left_out: u32,
}

fn source(var: &str, default: &str) -> String {
    std::env::var(var).unwrap_or_else(|_| default.to_owned())
}

/// Something loaded to speak with (kept while it is used).
enum Loaded {
    Kokoro(kokoro::Kokoro),
    Piper(String, piper::Piper),
}

/// The voices of this computer.
pub struct Voices {
    dir: PathBuf,
    loaded: Mutex<Option<Loaded>>,
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

impl Voices {
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            loaded: Mutex::new(None),
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    fn kokoro_dir(&self) -> PathBuf {
        self.dir.join(KOKORO)
    }

    fn piper_dir(&self, key: &str) -> PathBuf {
        self.dir.join("piper").join(key)
    }

    fn runtime_mb(&self) -> u32 {
        if runtime::library(&self.dir).is_some() {
            0
        } else {
            (runtime::size() / 1_000_000) as u32
        }
    }

    pub fn kokoro_downloaded(&self) -> bool {
        self.kokoro_dir().join(COMPLETE).is_file()
    }

    /// The names in Kokoro's voices file (read from its list, not loaded).
    fn kokoro_names(&self) -> Vec<String> {
        let Ok(f) = std::fs::File::open(self.kokoro_dir().join(kokoro::VOICES)) else {
            return Vec::new();
        };
        let Ok(z) = zip::ZipArchive::new(f) else {
            return Vec::new();
        };
        let mut v: Vec<String> = z
            .file_names()
            .filter_map(|n| n.strip_suffix(".npy").map(str::to_owned))
            .filter(|n| kokoro::language(n).is_some())
            .collect();
        v.sort();
        v
    }

    pub fn kokoro_info(&self) -> KokoroInfo {
        let downloaded = self.kokoro_downloaded();
        KokoroInfo {
            downloaded,
            size_mb: (kokoro::SIZE / 1_000_000) as u32 + self.runtime_mb(),
            licence: "Apache 2.0".into(),
            homepage: "https://huggingface.co/hexgrad/Kokoro-82M".into(),
            voices: if downloaded {
                self.kokoro_names().len() as u32
            } else {
                54
            },
        }
    }

    /// The Piper voices downloaded here.
    fn piper_downloaded(&self) -> Vec<StoredPiper> {
        let Ok(rd) = std::fs::read_dir(self.dir.join("piper")) else {
            return Vec::new();
        };
        let mut v: Vec<StoredPiper> = rd
            .flatten()
            .filter(|e| e.path().join(COMPLETE).is_file())
            .filter_map(|e| {
                serde_json::from_str(&std::fs::read_to_string(e.path().join(COMPLETE)).ok()?).ok()
            })
            .collect();
        v.sort_by(|a, b| a.key.cmp(&b.key));
        v
    }

    /// Every voice that can speak now.
    pub fn voices(&self) -> Vec<NaturalVoice> {
        let mut out = Vec::new();
        if self.kokoro_downloaded() {
            for n in self.kokoro_names() {
                let Some((tag, _, lang)) = kokoro::language(&n) else {
                    continue;
                };
                let (name, gender) = kokoro::display_name(&n);
                out.push(NaturalVoice {
                    id: format!("kokoro:{n}"),
                    name,
                    lang: tag.into(),
                    language: lang.into(),
                    pack: KOKORO.into(),
                    gender: gender.map(str::to_owned),
                    quality: None,
                });
            }
        }
        for p in self.piper_downloaded() {
            out.push(NaturalVoice {
                id: format!("piper:{}", p.key),
                name: title(&p.name),
                lang: p.lang,
                language: p.language,
                pack: format!("piper:{}", p.key),
                gender: None,
                quality: Some(p.quality),
            });
        }
        out
    }

    // ---- The Piper collection --------------------------------------------

    fn catalogue_path(&self) -> PathBuf {
        self.dir.join("piper-voices.json")
    }

    /// The collection's list (kept for a week).
    pub fn piper_catalogue(&self) -> Result<Vec<piper::Entry>, String> {
        let path = self.catalogue_path();
        let fresh = std::fs::metadata(&path)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| SystemTime::now().duration_since(t).ok())
            .is_some_and(|age| age < Duration::from_secs(7 * 24 * 3600));
        if !fresh {
            let url = format!("{}/voices.json", source("LIBRERI_PIPER_URL", piper::SOURCE));
            match fetch::text(&url).and_then(|t| piper::parse_catalogue(&t).map(|_| t)) {
                Ok(text) => {
                    std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
                    std::fs::write(&path, text).map_err(|e| e.to_string())?;
                }
                // Offline: an old list is better than none.
                Err(e) if !path.is_file() => return Err(e),
                Err(_) => {}
            }
        }
        piper::parse_catalogue(&std::fs::read_to_string(&path).map_err(|e| e.to_string())?)
    }

    pub fn piper_languages(&self) -> Result<Vec<PiperLanguage>, String> {
        let mut m: BTreeMap<String, PiperLanguage> = BTreeMap::new();
        for e in self.piper_catalogue()? {
            m.entry(e.language.code.clone())
                .or_insert_with(|| PiperLanguage {
                    code: e.language.code.clone(),
                    name: e.language_name(),
                    voices: 0,
                })
                .voices += 1;
        }
        let mut v: Vec<PiperLanguage> = m.into_values().collect();
        v.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(v)
    }

    fn licences_path(&self) -> PathBuf {
        self.dir.join("piper-licences.json")
    }

    /// The licences of these voices, read from their MODEL_CARDs (kept).
    fn licences(&self, entries: &[&piper::Entry]) -> BTreeMap<String, Vec<String>> {
        let path = self.licences_path();
        let mut known: BTreeMap<String, Vec<String>> = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
            .unwrap_or_default();
        let base = source("LIBRERI_PIPER_URL", piper::SOURCE);
        let missing: Vec<(String, String)> = entries
            .iter()
            .filter(|e| !known.contains_key(&e.key))
            .filter_map(|e| Some((e.key.clone(), format!("{base}/{}", e.file("MODEL_CARD")?))))
            .collect();
        if !missing.is_empty() {
            // A few at a time: a language has up to a dozen voices.
            let found: Vec<(String, Option<Vec<String>>)> = std::thread::scope(|s| {
                let handles: Vec<_> = missing
                    .iter()
                    .map(|(k, url)| {
                        s.spawn(move || {
                            (
                                k.clone(),
                                fetch::text(url).ok().map(|c| piper::card_licences(&c)),
                            )
                        })
                    })
                    .collect();
                handles.into_iter().filter_map(|h| h.join().ok()).collect()
            });
            let mut changed = false;
            for (k, l) in found {
                if let Some(l) = l {
                    known.insert(k, l);
                    changed = true;
                }
            }
            if changed {
                if let Ok(t) = serde_json::to_string(&known) {
                    let _ = std::fs::create_dir_all(&self.dir);
                    let _ = std::fs::write(&path, t);
                }
            }
        }
        known
    }

    /// The voices of one language (`de_DE`) that are free to use.
    pub fn piper_voices(&self, code: &str) -> Result<PiperList, String> {
        let all = self.piper_catalogue()?;
        let these: Vec<&piper::Entry> = all.iter().filter(|e| e.language.code == code).collect();
        let licences = self.licences(&these);
        let mut voices = Vec::new();
        let mut left_out = 0;
        for e in these {
            let ls = licences.get(&e.key).cloned().unwrap_or_default();
            if ls.is_empty() || !ls.iter().all(|l| piper::is_free(l)) {
                left_out += 1;
                continue;
            }
            voices.push(PiperVoiceInfo {
                id: format!("piper:{}", e.key),
                name: title(&e.name),
                lang: e.tag(),
                language: e.language_name(),
                quality: e.quality.clone(),
                size_mb: (e.size() / 1_000_000).max(1) as u32,
                licence: ls.join(", "),
                downloaded: self.piper_dir(&e.key).join(COMPLETE).is_file(),
            });
        }
        let rank = |q: &str| match q {
            "high" => 0,
            "medium" => 1,
            "low" => 2,
            _ => 3,
        };
        voices.sort_by(|a, b| {
            a.name
                .cmp(&b.name)
                .then(rank(&a.quality).cmp(&rank(&b.quality)))
        });
        Ok(PiperList { voices, left_out })
    }

    // ---- Downloading and removing -----------------------------------------

    /// Downloads "kokoro" or "piper:<voice>" (and the runtime the first
    /// time). Only free Piper voices can be downloaded.
    pub fn download(
        &self,
        id: &str,
        cancel: &AtomicBool,
        mut progress: impl FnMut(u64, u64),
    ) -> Result<(), String> {
        let (gets, total, folder, note) = if id == KOKORO {
            let base = source("LIBRERI_KOKORO_URL", kokoro::SOURCE);
            let d = self.kokoro_dir();
            let gets = [kokoro::MODEL, kokoro::VOICES]
                .iter()
                .map(|f| fetch::Get {
                    url: format!("{base}/{f}"),
                    to: d.join(f),
                })
                .collect::<Vec<_>>();
            (
                gets,
                kokoro::SIZE,
                d,
                serde_json::json!({ "files": [kokoro::MODEL, kokoro::VOICES] }),
            )
        } else if let Some(key) = id.strip_prefix("piper:") {
            let all = self.piper_catalogue()?;
            let e = all.iter().find(|e| e.key == key).ok_or("unknown voice")?;
            let ls = self.licences(&[e]).get(key).cloned().unwrap_or_default();
            if ls.is_empty() || !ls.iter().all(|l| piper::is_free(l)) {
                return Err("this voice's licence does not let everyone use it".into());
            }
            let base = source("LIBRERI_PIPER_URL", piper::SOURCE);
            let d = self.piper_dir(key);
            let gets = e
                .files
                .keys()
                .map(|f| fetch::Get {
                    url: format!("{base}/{f}"),
                    to: d.join(f.rsplit('/').next().unwrap_or(f)),
                })
                .collect::<Vec<_>>();
            let note = serde_json::to_value(StoredPiper {
                key: key.into(),
                name: e.name.clone(),
                lang: e.tag(),
                language: e.language_name(),
                quality: e.quality.clone(),
                licence: ls.join(", "),
            })
            .map_err(|e| e.to_string())?;
            (gets, e.size(), d, note)
        } else {
            return Err("unknown voice".into());
        };
        let rt = if runtime::library(&self.dir).is_some() {
            0
        } else {
            runtime::size()
        };
        let all = total + rt;
        runtime::ensure(&self.dir, cancel, |d, _| progress(d, all))?;
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(folder.join(COMPLETE));
        fetch::files(&gets, total, cancel, |d, t| progress(rt + d, rt + t))?;
        std::fs::write(folder.join(COMPLETE), note.to_string()).map_err(|e| e.to_string())
    }

    /// Removes a download; the runtime goes with the last one.
    pub fn remove(&self, id: &str) -> Result<(), String> {
        let folder = if id == KOKORO {
            self.kokoro_dir()
        } else if let Some(key) = id.strip_prefix("piper:") {
            if key.is_empty() || key.contains(['/', '\\']) || key.starts_with('.') {
                return Err("unknown voice".into());
            }
            self.piper_dir(key)
        } else {
            return Err("unknown voice".into());
        };
        *lock(&self.loaded) = None;
        fetch::remove_dir(&folder)?;
        if !self.kokoro_downloaded() && self.piper_downloaded().is_empty() {
            // The runtime may still be loaded in this run; it is freed
            // when Libreri closes and simply not used again.
            let _ = runtime::remove(&self.dir);
        }
        Ok(())
    }

    // ---- Speaking -----------------------------------------------------------

    /// Speaks `text` with a voice; returns samples and their rate.
    pub fn speak(&self, voice: &str, text: &str, speed: f32) -> Result<(Vec<f32>, u32), String> {
        runtime::load(&self.dir)?;
        let mut slot = lock(&self.loaded);
        if let Some(name) = voice.strip_prefix("kokoro:") {
            if !self.kokoro_downloaded() {
                return Err("Kokoro is not downloaded (Settings › Reader)".into());
            }
            if !matches!(*slot, Some(Loaded::Kokoro(_))) {
                *slot = None;
                *slot = Some(Loaded::Kokoro(kokoro::Kokoro::load(&self.kokoro_dir())?));
            }
            let Some(Loaded::Kokoro(k)) = slot.as_mut() else {
                unreachable!()
            };
            Ok((k.speak(text, name, speed)?, kokoro::SAMPLE_RATE))
        } else if let Some(key) = voice.strip_prefix("piper:") {
            let d = self.piper_dir(key);
            if !d.join(COMPLETE).is_file() {
                return Err("this voice is not downloaded (Settings › Reader)".into());
            }
            if !matches!(&*slot, Some(Loaded::Piper(k, _)) if k == key) {
                *slot = None;
                *slot = Some(Loaded::Piper(key.into(), piper::Piper::load(&d)?));
            }
            let Some(Loaded::Piper(_, p)) = slot.as_mut() else {
                unreachable!()
            };
            let rate = p.sample_rate();
            Ok((p.speak(text, speed)?, rate))
        } else {
            Err("unknown voice".into())
        }
    }

    /// Frees the loaded voice's memory.
    pub fn unload(&self) {
        *lock(&self.loaded) = None;
    }
}

/// What is kept about a downloaded Piper voice (its `.complete` note).
#[derive(Debug, Clone, Serialize, serde::Deserialize)]
struct StoredPiper {
    key: String,
    name: String,
    lang: String,
    language: String,
    quality: String,
    licence: String,
}

/// "en_US-libritts_r" voice names: "thorsten_emotional" → "Thorsten emotional".
fn title(name: &str) -> String {
    let s = name.replace('_', " ");
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_downloaded() {
        let dir = tempfile::tempdir().unwrap();
        let v = Voices::new(dir.path().to_path_buf());
        assert!(v.voices().is_empty());
        let k = v.kokoro_info();
        assert!(!k.downloaded && k.voices == 54 && k.size_mb > 350);
        assert!(v.speak("kokoro:af_heart", "hi", 1.0).is_err());
        assert!(v.remove("piper:../x").is_err());
        v.remove(KOKORO).unwrap();
    }

    #[test]
    fn lists_downloaded_piper_voices() {
        let dir = tempfile::tempdir().unwrap();
        let v = Voices::new(dir.path().to_path_buf());
        let d = dir.path().join("piper/de_DE-thorsten-high");
        std::fs::create_dir_all(&d).unwrap();
        let note = StoredPiper {
            key: "de_DE-thorsten-high".into(),
            name: "thorsten".into(),
            lang: "de-DE".into(),
            language: "German (Germany)".into(),
            quality: "high".into(),
            licence: "CC0".into(),
        };
        std::fs::write(d.join(COMPLETE), serde_json::to_string(&note).unwrap()).unwrap();
        let list = v.voices();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "piper:de_DE-thorsten-high");
        assert_eq!(list[0].name, "Thorsten");
        v.remove("piper:de_DE-thorsten-high").unwrap();
        assert!(v.voices().is_empty());
    }

    #[test]
    fn titles() {
        assert_eq!(title("thorsten_emotional"), "Thorsten emotional");
    }

    /// Speaks with a real Kokoro download when `LIBRERI_TEST_VOICES` points
    /// to a folder with `kokoro/` and `LIBRERI_ORT_LIB` to the runtime.
    #[test]
    fn speaks_with_kokoro_when_there() {
        let Some(dir) = std::env::var_os("LIBRERI_TEST_VOICES") else {
            return;
        };
        let v = Voices::new(PathBuf::from(dir));
        let (audio, rate) = v
            .speak(
                "kokoro:af_heart",
                "The harbour was quiet that morning, and Dr. Smith read 3 chapters.",
                1.0,
            )
            .unwrap();
        assert_eq!(rate, 24_000);
        let secs = audio.len() as f32 / rate as f32;
        assert!((2.5..8.0).contains(&secs), "{secs}");
        let loud = audio.iter().map(|s| s.abs()).sum::<f32>() / audio.len() as f32;
        assert!(loud > 0.01, "{loud}");
        if let Some(out) = std::env::var_os("LIBRERI_TEST_WAV") {
            std::fs::write(&out, wav(&audio, rate)).unwrap();
        }
        // A Piper voice, when one is there too.
        if v.voices()
            .iter()
            .any(|x| x.id == "piper:en_US-lessac-medium")
        {
            let (audio, rate) = v
                .speak(
                    "piper:en_US-lessac-medium",
                    "Smith read 3 chapters. Then he slept.",
                    1.0,
                )
                .unwrap();
            assert_eq!(rate, 22_050);
            let secs = audio.len() as f32 / rate as f32;
            assert!((1.5..6.0).contains(&secs), "{secs}");
            if let Some(out) = std::env::var_os("LIBRERI_TEST_WAV") {
                let mut p = PathBuf::from(out);
                p.set_extension("piper.wav");
                std::fs::write(p, wav(&audio, rate)).unwrap();
            }
        }
    }
}
