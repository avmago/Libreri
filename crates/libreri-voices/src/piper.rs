//! Piper voices (Rhasspy / Open Home Foundation): one small model per
//! voice, in 40+ languages. The list comes from the `rhasspy/piper-voices`
//! collection on Hugging Face; each voice's MODEL_CARD names its licence,
//! and only voices free to use are offered.

use crate::phonemes;
use ort::session::Session;
use ort::value::Tensor;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use unicode_normalization::UnicodeNormalization;

pub const SOURCE: &str = "https://huggingface.co/rhasspy/piper-voices/resolve/main";

/// One voice of the collection's `voices.json`.
#[derive(Debug, Clone, Deserialize)]
pub struct Entry {
    pub key: String,
    pub name: String,
    pub language: Language,
    pub quality: String,
    #[serde(default)]
    pub num_speakers: u32,
    pub files: BTreeMap<String, FileInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Language {
    pub code: String,
    pub family: String,
    #[serde(default)]
    pub name_english: String,
    #[serde(default)]
    pub country_english: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileInfo {
    pub size_bytes: u64,
}

impl Entry {
    /// The path (in the collection) of the file ending in `suffix`.
    pub fn file(&self, suffix: &str) -> Option<&str> {
        self.files
            .keys()
            .find(|k| k.ends_with(suffix))
            .map(String::as_str)
    }

    pub fn size(&self) -> u64 {
        self.files.values().map(|f| f.size_bytes).sum()
    }

    /// "de_DE" → "de-DE".
    pub fn tag(&self) -> String {
        self.language.code.replace('_', "-")
    }

    /// "German (Germany)".
    pub fn language_name(&self) -> String {
        let l = &self.language;
        if l.country_english.is_empty() {
            l.name_english.clone()
        } else {
            format!("{} ({})", l.name_english, l.country_english)
        }
    }
}

/// Parses `voices.json`, keeping voices that have a model and settings.
pub fn parse_catalogue(json: &str) -> Result<Vec<Entry>, String> {
    let m: BTreeMap<String, Entry> =
        serde_json::from_str(json).map_err(|e| format!("the voice list could not be read: {e}"))?;
    Ok(m.into_values()
        .filter(|e| e.file(".onnx").is_some() && e.file(".onnx.json").is_some())
        .collect())
}

/// The licences a MODEL_CARD names (`* License: CC0`).
pub fn card_licences(card: &str) -> Vec<String> {
    card.lines()
        .filter_map(|l| {
            let l = l.trim().trim_start_matches(['*', '-']).trim();
            let (k, v) = l.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("license")
                .then(|| v.trim().to_owned())
        })
        .filter(|v| !v.is_empty())
        .collect()
}

/// Whether a licence lets anyone use the voice, also for work: public
/// domain, CC0, CC BY (and BY-SA), MIT, Apache, BSD. Not "non-commercial",
/// "no derivatives", or a licence Libreri does not know.
pub fn is_free(licence: &str) -> bool {
    let l = licence.to_ascii_lowercase().replace(['-', '_'], " ");
    let words: Vec<&str> = l
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '.')
        .filter(|w| !w.is_empty())
        .collect();
    let has = |w: &str| words.contains(&w);
    if has("nc") || has("nd") || l.contains("non commercial") || l.contains("noncommercial") {
        return false;
    }
    l.contains("public domain")
        || has("cc0")
        || (has("cc") && has("by"))
        || has("mit")
        || has("apache")
        || has("bsd")
}

/// A voice's settings (`.onnx.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub audio: Audio,
    pub espeak: Espeak,
    #[serde(default)]
    pub inference: Inference,
    pub phoneme_id_map: HashMap<String, Vec<i64>>,
    #[serde(default)]
    pub num_speakers: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Audio {
    pub sample_rate: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Espeak {
    pub voice: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Inference {
    #[serde(default = "noise")]
    pub noise_scale: f32,
    #[serde(default = "one")]
    pub length_scale: f32,
    #[serde(default = "noise_w")]
    pub noise_w: f32,
}

fn noise() -> f32 {
    0.667
}
fn one() -> f32 {
    1.0
}
fn noise_w() -> f32 {
    0.8
}

impl Default for Inference {
    fn default() -> Self {
        Self {
            noise_scale: noise(),
            length_scale: one(),
            noise_w: noise_w(),
        }
    }
}

impl Config {
    /// Phonemes to ids as Piper does: start, then each phoneme followed by
    /// the pad, then the end.
    pub fn ids(&self, phonemes: &str) -> Vec<i64> {
        let get = |s: &str| self.phoneme_id_map.get(s).cloned().unwrap_or_default();
        let pad = get("_");
        let mut ids = get("^");
        ids.extend(&pad);
        for c in phonemes.nfd() {
            if let Some(v) = self.phoneme_id_map.get(c.encode_utf8(&mut [0; 4]) as &str) {
                ids.extend(v);
                ids.extend(&pad);
            }
        }
        ids.extend(get("$"));
        ids
    }
}

pub struct Piper {
    session: Session,
    pub config: Config,
}

impl Piper {
    /// Loads the voice in `folder` (its .onnx and .onnx.json).
    pub fn load(folder: &Path) -> Result<Self, String> {
        let mut model = None;
        let mut config = None;
        for e in std::fs::read_dir(folder)
            .map_err(|e| e.to_string())?
            .flatten()
        {
            let p = e.path();
            let n = p
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            if n.ends_with(".onnx.json") {
                config = Some(p);
            } else if n.ends_with(".onnx") {
                model = Some(p);
            }
        }
        let (model, config) = model.zip(config).ok_or("the voice's files are missing")?;
        let config: Config =
            serde_json::from_str(&std::fs::read_to_string(config).map_err(|e| e.to_string())?)
                .map_err(|e| format!("the voice's settings could not be read: {e}"))?;
        let session = Session::builder()
            .and_then(|mut b| b.commit_from_file(model))
            .map_err(|e| format!("the voice could not be loaded: {e}"))?;
        Ok(Self { session, config })
    }

    pub fn sample_rate(&self) -> u32 {
        self.config.audio.sample_rate
    }

    /// Speaks `text`; `speed` 1 is normal.
    pub fn speak(&mut self, text: &str, speed: f32) -> Result<Vec<f32>, String> {
        let ph = phonemes::phonemize(text, &self.config.espeak.voice)?;
        // Like Piper, each sentence on its own, with a short rest between.
        let rest = vec![0.0; (self.sample_rate() / 5) as usize];
        let mut audio = Vec::new();
        for s in sentences(&ph) {
            let part = self.speak_ids(self.config.ids(s), speed)?;
            if part.is_empty() {
                continue;
            }
            if !audio.is_empty() {
                audio.extend_from_slice(&rest);
            }
            audio.extend(part);
        }
        Ok(audio)
    }

    fn speak_ids(&mut self, ids: Vec<i64>, speed: f32) -> Result<Vec<f32>, String> {
        if ids.len() < 3 {
            return Ok(Vec::new());
        }
        let n = ids.len();
        let inf = &self.config.inference;
        let scales = vec![
            inf.noise_scale,
            inf.length_scale / speed.clamp(0.5, 2.0),
            inf.noise_w,
        ];
        let input = Tensor::from_array(([1usize, n], ids)).map_err(|e| e.to_string())?;
        let lengths = Tensor::from_array(([1usize], vec![n as i64])).map_err(|e| e.to_string())?;
        let scales = Tensor::from_array(([3usize], scales)).map_err(|e| e.to_string())?;
        let multi =
            self.config.num_speakers > 1 && self.session.inputs().iter().any(|i| i.name() == "sid");
        let out = if multi {
            let sid = Tensor::from_array(([1usize], vec![0i64])).map_err(|e| e.to_string())?;
            self.session.run(ort::inputs![
                "input" => input,
                "input_lengths" => lengths,
                "scales" => scales,
                "sid" => sid,
            ])
        } else {
            self.session.run(ort::inputs![
                "input" => input,
                "input_lengths" => lengths,
                "scales" => scales,
            ])
        }
        .map_err(|e| format!("the voice could not speak: {e}"))?;
        let (_, samples) = out[0]
            .try_extract_tensor::<f32>()
            .map_err(|e| e.to_string())?;
        Ok(samples.to_vec())
    }
}

/// Phonemes cut after each sentence's end mark.
pub fn sentences(ph: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut start = 0;
    let b = ph.as_bytes();
    for (i, c) in ph.char_indices() {
        if matches!(c, '.' | '!' | '?') && b.get(i + 1).is_none_or(|n| *n == b' ') {
            out.push(ph[start..=i].trim());
            start = i + 1;
        }
    }
    if !ph[start..].trim().is_empty() {
        out.push(ph[start..].trim());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cuts_sentences() {
        assert_eq!(sentences("a b. c d! e"), ["a b.", "c d!", "e"]);
        assert_eq!(sentences("a.b c."), ["a.b c."]);
    }

    #[test]
    fn licences() {
        let card = "# Model card for thorsten (high)\n\n* Language: de_DE\n\n## Dataset\n\n* URL: https://x\n* License: CC0\n";
        assert_eq!(card_licences(card), ["CC0"]);
        for free in [
            "CC0",
            "CC BY 4.0",
            "CC-BY-SA-4.0",
            "MIT",
            "Public Domain",
            "Apache 2.0",
            "cc-by 4.0",
        ] {
            assert!(is_free(free), "{free}");
        }
        for not in [
            "CC BY-NC-SA 4.0",
            "CC-BY-NC 4.0",
            "CC BY-ND 4.0",
            "See URL",
            "",
            "Non-commercial use only",
            "Blizzard",
        ] {
            assert!(!is_free(not), "{not}");
        }
    }

    #[test]
    fn reads_the_catalogue() {
        let json = r#"{"de_DE-thorsten-high": {"key": "de_DE-thorsten-high", "name": "thorsten",
          "language": {"code": "de_DE", "family": "de", "region": "DE", "name_native": "Deutsch",
            "name_english": "German", "country_english": "Germany"},
          "quality": "high", "num_speakers": 1, "speaker_id_map": {},
          "files": {"de/de_DE/thorsten/high/de_DE-thorsten-high.onnx": {"size_bytes": 113895201, "md5_digest": "x"},
            "de/de_DE/thorsten/high/de_DE-thorsten-high.onnx.json": {"size_bytes": 4875, "md5_digest": "y"},
            "de/de_DE/thorsten/high/MODEL_CARD": {"size_bytes": 279, "md5_digest": "z"}}, "aliases": []},
          "xx-broken": {"key": "xx-broken", "name": "b", "language": {"code": "xx", "family": "xx"},
            "quality": "low", "files": {}}}"#;
        let c = parse_catalogue(json).unwrap();
        assert_eq!(c.len(), 1);
        let e = &c[0];
        assert_eq!(e.tag(), "de-DE");
        assert_eq!(e.language_name(), "German (Germany)");
        assert_eq!(
            e.file("MODEL_CARD"),
            Some("de/de_DE/thorsten/high/MODEL_CARD")
        );
        assert_eq!(e.size(), 113895201 + 4875 + 279);
    }

    #[test]
    fn ids_like_piper() {
        let cfg: Config = serde_json::from_str(
            r#"{"audio": {"sample_rate": 22050}, "espeak": {"voice": "en-us"},
                "phoneme_id_map": {"_": [0], "^": [1], "$": [2], "a": [14], "b": [15], " ": [3], "\u0301": [9]}}"#,
        )
        .unwrap();
        assert_eq!(cfg.ids("ab a"), [1, 0, 14, 0, 15, 0, 3, 0, 14, 0, 2]);
        // Accents are split off (NFD) and looked up on their own.
        assert_eq!(cfg.ids("á"), [1, 0, 14, 0, 9, 0, 2]);
        assert!((cfg.inference.noise_w - 0.8).abs() < 1e-6);
    }
}
