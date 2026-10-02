//! Kokoro 82M (hexgrad, Apache 2.0): 54 voices in nine languages from one
//! model. The ONNX export and the voices file come from the kokoro-onnx
//! project's releases (MIT).

use crate::phonemes;
use ort::session::Session;
use ort::value::Tensor;
use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::OnceLock;

pub const SAMPLE_RATE: u32 = 24_000;
/// Most phonemes the model takes at once.
const MAX_PHONEMES: usize = 510;
pub const MODEL: &str = "kokoro-v1.0.onnx";
pub const VOICES: &str = "voices-v1.0.bin";
pub const SOURCE: &str =
    "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0";
/// Both files, in bytes.
pub const SIZE: u64 = 325_532_387 + 28_214_398;

fn vocab() -> &'static HashMap<char, i64> {
    static V: OnceLock<HashMap<char, i64>> = OnceLock::new();
    V.get_or_init(|| {
        let m: HashMap<String, i64> =
            serde_json::from_str(include_str!("kokoro_vocab.json")).unwrap_or_default();
        m.into_iter()
            .filter_map(|(k, v)| Some((k.chars().next()?, v)))
            .collect()
    })
}

/// A voice's language from its name's first letter ("af_heart": American
/// English): (BCP 47 tag, eSpeak voice, name).
pub fn language(voice: &str) -> Option<(&'static str, &'static str, &'static str)> {
    Some(match voice.chars().next()? {
        'a' => ("en-US", "en-us", "English (US)"),
        'b' => ("en-GB", "en-gb", "English (UK)"),
        'e' => ("es", "es", "Spanish"),
        'f' => ("fr", "fr-fr", "French"),
        'h' => ("hi", "hi", "Hindi"),
        'i' => ("it", "it", "Italian"),
        'j' => ("ja", "ja", "Japanese"),
        'p' => ("pt-BR", "pt-br", "Portuguese (Brazil)"),
        'z' => ("zh", "cmn", "Chinese"),
        _ => return None,
    })
}

/// "af_heart" → "Heart"; the second letter says female or male.
pub fn display_name(voice: &str) -> (String, Option<&'static str>) {
    let rest = voice.split_once('_').map_or(voice, |(_, r)| r);
    let mut c = rest.chars();
    let name = c
        .next()
        .map(|f| f.to_uppercase().chain(c).collect())
        .unwrap_or_default();
    let gender = match voice.chars().nth(1) {
        Some('f') => Some("female"),
        Some('m') => Some("male"),
        _ => None,
    };
    (name, gender)
}

/// The voices file: a NumPy .npz of `(510, 1, 256)` float arrays.
pub fn read_voices(path: &Path) -> Result<HashMap<String, Vec<f32>>, String> {
    let f = std::fs::File::open(path).map_err(|e| e.to_string())?;
    let mut z = zip::ZipArchive::new(f).map_err(|e| format!("the voices file is damaged: {e}"))?;
    let mut out = HashMap::new();
    for i in 0..z.len() {
        let mut e = z.by_index(i).map_err(|e| e.to_string())?;
        let Some(name) = e.name().strip_suffix(".npy").map(str::to_owned) else {
            continue;
        };
        let mut b = Vec::new();
        e.read_to_end(&mut b).map_err(|e| e.to_string())?;
        out.insert(name, npy_f32(&b)?);
    }
    Ok(out)
}

/// The numbers of a little-endian float32 .npy file.
fn npy_f32(b: &[u8]) -> Result<Vec<f32>, String> {
    if b.len() < 10 || &b[..6] != b"\x93NUMPY" {
        return Err("not a NumPy array".into());
    }
    let (len, start) = if b[6] == 1 {
        (u16::from_le_bytes([b[8], b[9]]) as usize, 10)
    } else {
        if b.len() < 12 {
            return Err("not a NumPy array".into());
        }
        (u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize, 12)
    };
    let header = std::str::from_utf8(b.get(start..start + len).ok_or("short array")?)
        .map_err(|e| e.to_string())?;
    if !header.contains("'<f4'") {
        return Err("the voices file has an unexpected number type".into());
    }
    Ok(b[start + len..]
        .chunks_exact(4)
        .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect())
}

/// Phonemes to the model's token numbers (unknown ones are dropped).
pub fn tokens(phonemes: &str) -> Vec<i64> {
    let v = vocab();
    phonemes
        .chars()
        .filter_map(|c| v.get(&c).copied())
        .collect()
}

/// Cuts long phoneme text at spaces into parts the model takes.
pub fn batches(phonemes: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for w in phonemes.split(' ') {
        if !cur.is_empty() && cur.chars().count() + 1 + w.chars().count() > MAX_PHONEMES {
            out.push(std::mem::take(&mut cur));
        }
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(&w.chars().take(MAX_PHONEMES).collect::<String>());
    }
    if !cur.trim().is_empty() {
        out.push(cur);
    }
    out
}

pub struct Kokoro {
    session: Session,
    voices: HashMap<String, Vec<f32>>,
    tokens_input: String,
}

impl Kokoro {
    pub fn load(dir: &Path) -> Result<Self, String> {
        let session = Session::builder()
            .and_then(|mut b| b.commit_from_file(dir.join(MODEL)))
            .map_err(|e| format!("the Kokoro model could not be loaded: {e}"))?;
        let voices = read_voices(&dir.join(VOICES))?;
        // Older exports name the token input "input_ids".
        let tokens_input = if session.inputs().iter().any(|i| i.name() == "input_ids") {
            "input_ids"
        } else {
            "tokens"
        }
        .to_owned();
        Ok(Self {
            session,
            voices,
            tokens_input,
        })
    }

    pub fn voice_names(&self) -> Vec<String> {
        let mut v: Vec<String> = self.voices.keys().cloned().collect();
        v.sort();
        v
    }

    /// Speaks `text` with `voice` ("af_heart"); 24 kHz samples.
    pub fn speak(&mut self, text: &str, voice: &str, speed: f32) -> Result<Vec<f32>, String> {
        let style_all = self
            .voices
            .get(voice)
            .ok_or("this Kokoro voice is not here")?;
        let (_, espeak, _) = language(voice).ok_or("unknown Kokoro voice")?;
        let ph = phonemes::phonemize(text, espeak)?;
        let mut audio = Vec::new();
        for part in batches(&ph) {
            let toks = tokens(&part);
            if toks.is_empty() {
                continue;
            }
            // One style row per phoneme count: n phonemes use row n - 1.
            let rows = style_all.len() / 256;
            let row = toks.len().clamp(1, rows) - 1;
            let style = style_all[row * 256..row * 256 + 256].to_vec();
            let mut ids = Vec::with_capacity(toks.len() + 2);
            ids.push(0);
            ids.extend(&toks);
            ids.push(0);
            let n = ids.len();
            let t = Tensor::from_array(([1usize, n], ids)).map_err(|e| e.to_string())?;
            let s = Tensor::from_array(([1usize, 256], style)).map_err(|e| e.to_string())?;
            let sp = Tensor::from_array(([1usize], vec![speed.clamp(0.5, 2.0)]))
                .map_err(|e| e.to_string())?;
            let out = self
                .session
                .run(ort::inputs![
                    self.tokens_input.as_str() => t,
                    "style" => s,
                    "speed" => sp,
                ])
                .map_err(|e| format!("Kokoro could not speak: {e}"))?;
            let (_, samples) = out[0]
                .try_extract_tensor::<f32>()
                .map_err(|e| e.to_string())?;
            audio.extend_from_slice(samples);
        }
        Ok(audio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_match_kokoro_onnx() {
        // kokoro-onnx's tokens for these phonemes.
        assert_eq!(
            tokens("ðə hˈɑːɹbɚ, ænd."),
            [81, 83, 16, 50, 156, 69, 158, 123, 44, 85, 3, 16, 72, 56, 46, 4]
        );
        assert_eq!(vocab().len(), 114);
    }

    #[test]
    fn names_and_languages() {
        assert_eq!(display_name("af_heart"), ("Heart".into(), Some("female")));
        assert_eq!(display_name("bm_george").0, "George");
        assert_eq!(language("zf_xiaoxiao").unwrap().1, "cmn");
        assert!(language("qq_x").is_none());
    }

    #[test]
    fn cuts_long_text() {
        let long = vec!["ab"; 300].join(" ");
        let b = batches(&long);
        assert!(b.len() == 2 && b.iter().all(|p| p.chars().count() <= MAX_PHONEMES));
        assert_eq!(b.join(" "), long);
    }

    #[test]
    fn reads_npy() {
        let mut b = b"\x93NUMPY\x01\x00".to_vec();
        let h = "{'descr': '<f4', 'fortran_order': False, 'shape': (2,), }";
        b.extend((h.len() as u16).to_le_bytes());
        b.extend(h.as_bytes());
        b.extend(1.5f32.to_le_bytes());
        b.extend((-2.0f32).to_le_bytes());
        assert_eq!(npy_f32(&b).unwrap(), [1.5, -2.0]);
    }
}
