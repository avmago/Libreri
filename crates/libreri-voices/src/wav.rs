//! Samples to a WAV file (16-bit, mono), for the web view to play.

/// A short fade at both ends, so sentences start and stop without a click.
const FADE_MS: u32 = 8;

pub fn encode(samples: &[f32], rate: u32) -> Vec<u8> {
    let n = samples.len();
    let fade = ((rate * FADE_MS / 1000) as usize).min(n / 2);
    let data_len = (n * 2) as u32;
    let mut out = Vec::with_capacity(44 + n * 2);
    out.extend(b"RIFF");
    out.extend((36 + data_len).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes()); // PCM
    out.extend(1u16.to_le_bytes()); // mono
    out.extend(rate.to_le_bytes());
    out.extend((rate * 2).to_le_bytes());
    out.extend(2u16.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data_len.to_le_bytes());
    for (i, s) in samples.iter().enumerate() {
        let edge = i.min(n - 1 - i);
        let g = if edge < fade {
            edge as f32 / fade as f32
        } else {
            1.0
        };
        let v = (s * g).clamp(-1.0, 1.0);
        out.extend(((v * 32767.0) as i16).to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn writes_a_header() {
        let w = super::encode(&[0.0, 0.5, -0.5, 0.0], 24_000);
        assert_eq!(&w[..4], b"RIFF");
        assert_eq!(w.len(), 44 + 8);
        assert_eq!(u32::from_le_bytes([w[24], w[25], w[26], w[27]]), 24_000);
    }
}
