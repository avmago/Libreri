//! Audio in and out: decoding audio files (MP3, AAC/M4B, FLAC, Ogg Vorbis,
//! WAV) to the 16 kHz mono samples whisper needs, and saving recorded voice
//! notes as FLAC (lossless, about half the size of WAV, plays everywhere).

use std::fs::File;
use std::path::Path;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error as SymErr;
use symphonia::core::formats::{FormatOptions, SeekMode, SeekTo};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use symphonia::core::units::Time;

/// The rate whisper works at.
pub const RATE: u32 = 16_000;

/// Mono samples at `rate`.
pub struct Pcm {
    pub samples: Vec<f32>,
    pub rate: u32,
}

/// Reads `len` seconds of a file from `start` seconds (everything when
/// `len` is None), as mono samples at 16 kHz. Returns the samples and the
/// time they really start at (seeking lands on a packet boundary).
pub fn decode(path: &Path, start: f64, len: Option<f64>) -> Result<(Vec<f32>, f64), String> {
    let file = File::open(path).map_err(|e| e.to_string())?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(&ext.to_ascii_lowercase());
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| format!("this audio cannot be read: {e}"))?;
    let mut format = probed.format;
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or("there is no audio in this file")?;
    let id = track.id;
    let rate = track
        .codec_params
        .sample_rate
        .ok_or("unknown sample rate")?;
    let time_base = track.codec_params.time_base;
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|e| format!("this audio cannot be decoded: {e}"))?;
    let mut at = 0.0;
    if start > 0.0 {
        let seeked = format
            .seek(
                SeekMode::Coarse,
                SeekTo::Time {
                    time: Time::from(start),
                    track_id: Some(id),
                },
            )
            .map_err(|e| format!("could not go to {start:.0} s: {e}"))?;
        at = match time_base {
            Some(tb) => {
                let t = tb.calc_time(seeked.actual_ts);
                t.seconds as f64 + t.frac
            }
            None => start,
        };
    }
    let want = len.map(|l| (l * f64::from(rate)) as usize);
    let mut mono: Vec<f32> = Vec::new();
    let mut buf: Option<SampleBuffer<f32>> = None;
    loop {
        if want.is_some_and(|w| mono.len() >= w) {
            break;
        }
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymErr::IoError(_)) | Err(SymErr::ResetRequired) => break,
            Err(e) => return Err(e.to_string()),
        };
        if packet.track_id() != id {
            continue;
        }
        let decoded = match decoder.decode(&packet) {
            Ok(d) => d,
            // A damaged packet: skip it.
            Err(SymErr::DecodeError(_)) => continue,
            Err(e) => return Err(e.to_string()),
        };
        let spec = *decoded.spec();
        let channels = spec.channels.count().max(1);
        let b = buf.get_or_insert_with(|| SampleBuffer::new(decoded.capacity() as u64, spec));
        if b.capacity() < decoded.capacity() * channels {
            *b = SampleBuffer::new(decoded.capacity() as u64, spec);
        }
        b.copy_interleaved_ref(decoded);
        mono.extend(
            b.samples()
                .chunks_exact(channels)
                .map(|f| f.iter().sum::<f32>() / channels as f32),
        );
    }
    if let Some(w) = want {
        mono.truncate(w);
    }
    Ok((resample(&mono, rate, RATE), at))
}

/// Length of an audio file in seconds, when the file says.
pub fn duration(path: &Path) -> Option<f64> {
    let file = File::open(path).ok()?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }
    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .ok()?;
    let track = probed.format.default_track()?;
    let frames = track.codec_params.n_frames?;
    let rate = track.codec_params.sample_rate?;
    Some(frames as f64 / f64::from(rate))
}

/// Changes the sample rate: an averaging low-pass (so nothing folds back
/// into speech frequencies), then linear interpolation.
pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let smoothed: Vec<f32> = if from > to {
        let k = (from as f64 / to as f64).ceil() as usize;
        let mut out = Vec::with_capacity(input.len());
        let mut sum = 0.0f32;
        for i in 0..input.len() {
            sum += input[i];
            if i >= k {
                sum -= input[i - k];
            }
            out.push(sum / k.min(i + 1) as f32);
        }
        // Undo the half-window delay.
        let shift = k / 2;
        out.drain(..shift.min(out.len()));
        out
    } else {
        input.to_vec()
    };
    let ratio = f64::from(from) / f64::from(to);
    let n = (smoothed.len() as f64 / ratio).floor() as usize;
    (0..n)
        .map(|i| {
            let x = i as f64 * ratio;
            let j = x.floor() as usize;
            let f = (x - j as f64) as f32;
            let a = smoothed[j];
            let b = *smoothed.get(j + 1).unwrap_or(&a);
            a + (b - a) * f
        })
        .collect()
}

/// Encodes mono samples (−1…1) as a FLAC file.
pub fn to_flac(samples: &[f32], rate: u32) -> Result<Vec<u8>, String> {
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;
    let ints: Vec<i32> = samples
        .iter()
        .map(|s| (s.clamp(-1.0, 1.0) * 32767.0).round() as i32)
        .collect();
    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|e| format!("{e:?}"))?;
    let source = flacenc::source::MemSource::from_samples(&ints, 1, 16, rate as usize);
    let stream = flacenc::encode_with_fixed_block_size(&config, source, config.block_size)
        .map_err(|e| format!("{e:?}"))?;
    let mut sink = flacenc::bitsink::ByteSink::new();
    stream.write(&mut sink).map_err(|e| format!("{e:?}"))?;
    let mut out = sink.as_slice().to_vec();
    // STREAMINFO's smallest block size leaves out the last, shorter block
    // (the FLAC format says so); some readers take a mismatch as variable
    // block sizes and find no frames.
    if out.len() > 12 && &out[..4] == b"fLaC" {
        let (max0, max1) = (out[10], out[11]);
        out[8] = max0;
        out[9] = max1;
    }
    Ok(out)
}

/// Samples from 16-bit little-endian PCM bytes (what the interface sends).
pub fn from_i16_le(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tone(rate: u32, secs: f64, hz: f64) -> Vec<f32> {
        (0..(f64::from(rate) * secs) as usize)
            .map(|i| (0.5 * (i as f64 / f64::from(rate) * hz * std::f64::consts::TAU).sin()) as f32)
            .collect()
    }

    #[test]
    fn resamples_to_16k() {
        let t = tone(48_000, 1.0, 440.0);
        let r = resample(&t, 48_000, RATE);
        assert!((r.len() as i64 - 16_000).abs() < 5, "{}", r.len());
        // Still a 440 Hz tone: about 880 zero crossings a second.
        let crossings = r
            .windows(2)
            .filter(|w| (w[0] < 0.0) != (w[1] < 0.0))
            .count();
        assert!((860..=900).contains(&crossings), "{crossings}");
    }

    #[test]
    fn writes_flac_that_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let t = tone(RATE, 2.0, 300.0);
        let flac = to_flac(&t, RATE).unwrap();
        assert_eq!(&flac[..4], b"fLaC");
        let p = dir.path().join("note.flac");
        std::fs::write(&p, &flac).unwrap();
        let (back, at) = decode(&p, 0.0, None).unwrap();
        assert_eq!(at, 0.0);
        assert!((back.len() as i64 - t.len() as i64).abs() < 10);
        let err: f32 = back
            .iter()
            .zip(&t)
            .map(|(a, b)| (a - b).abs())
            .fold(0.0, f32::max);
        assert!(err < 0.001, "{err}");
        assert!((duration(&p).unwrap() - 2.0).abs() < 0.01);
        // A window from the middle.
        let (part, at) = decode(&p, 1.0, Some(0.5)).unwrap();
        assert!(at <= 1.0 && at > 0.5, "{at}");
        assert_eq!(part.len(), 8000);
    }

    #[test]
    fn reads_pcm_from_the_interface() {
        let bytes = [0x00, 0x40, 0x00, 0xC0];
        assert_eq!(from_i16_le(&bytes), [0.5, -0.5]);
    }
}
