//! Transcribing speech with whisper.cpp.

use serde::Serialize;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// A stretch of speech and its text (times in seconds).
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

/// A loaded model, shared between transcriptions.
pub struct Transcriber {
    ctx: WhisperContext,
    pub model: String,
}

impl Transcriber {
    /// Loads a model file (a few seconds for the big ones).
    pub fn load(path: &Path, model: &str) -> Result<Self, String> {
        whisper_rs::install_logging_hooks();
        let path = path
            .to_str()
            .ok_or("the model's folder name cannot be used")?;
        let ctx = WhisperContext::new_with_params(path, WhisperContextParameters::default())
            .map_err(|e| format!("the speech model could not be loaded: {e}"))?;
        Ok(Self {
            ctx,
            model: model.to_owned(),
        })
    }

    /// Transcribes 16 kHz mono samples. `lang` is a language code ("en"),
    /// or None to let the model tell. `cancel` stops it early.
    pub fn transcribe(
        &self,
        samples: &[f32],
        lang: Option<&str>,
        cancel: Option<Arc<AtomicBool>>,
    ) -> Result<Vec<Segment>, String> {
        if samples.len() < crate::audio::RATE as usize / 4 {
            return Ok(Vec::new());
        }
        let mut state = self.ctx.create_state().map_err(|e| e.to_string())?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        let threads = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .clamp(1, 8);
        params.set_n_threads(threads as i32);
        let lang = lang.map(|l| l.split(['-', '_']).next().unwrap_or(l).to_ascii_lowercase());
        params.set_language(Some(lang.as_deref().unwrap_or("auto")));
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_no_context(true);
        if let Some(c) = cancel {
            params.set_abort_callback_safe(move || c.load(Ordering::SeqCst));
        }
        state
            .full(params, samples)
            .map_err(|e| format!("transcribing failed: {e}"))?;
        let mut out = Vec::new();
        for i in 0..state.full_n_segments() {
            let Some(seg) = state.get_segment(i) else {
                continue;
            };
            let text = seg
                .to_str_lossy()
                .map(|s| s.trim().to_owned())
                .unwrap_or_default();
            // Whisper marks silence and noise like "[BLANK_AUDIO]" or "(music)".
            if text.is_empty() || is_noise(&text) {
                continue;
            }
            out.push(Segment {
                start: seg.start_timestamp() as f64 / 100.0,
                end: seg.end_timestamp() as f64 / 100.0,
                text,
            });
        }
        Ok(out)
    }
}

fn is_noise(text: &str) -> bool {
    let t = text.trim();
    (t.starts_with('[') && t.ends_with(']')) || (t.starts_with('(') && t.ends_with(')'))
}

/// Segments as one text, sentences run together.
pub fn join(segments: &[Segment]) -> String {
    segments
        .iter()
        .map(|s| s.text.trim())
        .filter(|t| !t.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joins_and_drops_noise() {
        let s = |t: &str| Segment {
            start: 0.0,
            end: 1.0,
            text: t.into(),
        };
        assert!(is_noise("[BLANK_AUDIO]") && is_noise(" (wind blowing) ") && !is_noise("Hello."));
        assert_eq!(
            join(&[s(" Hello. "), s("How are you?")]),
            "Hello. How are you?"
        );
    }

    /// With a model: `LIBRERI_WHISPER_MODEL=ggml-tiny.bin LIBRERI_WHISPER_AUDIO=speech.mp3
    /// cargo test -p libreri-speech transcribe::tests::real -- --nocapture`.
    #[test]
    fn real() {
        let (Some(model), Some(audio)) = (
            std::env::var_os("LIBRERI_WHISPER_MODEL"),
            std::env::var_os("LIBRERI_WHISPER_AUDIO"),
        ) else {
            return;
        };
        let t = Transcriber::load(Path::new(&model), "test").unwrap();
        let (pcm, _) = crate::audio::decode(Path::new(&audio), 0.0, Some(30.0)).unwrap();
        let segs = t.transcribe(&pcm, None, None).unwrap();
        println!("{segs:#?}");
        assert!(!segs.is_empty());
    }
}
