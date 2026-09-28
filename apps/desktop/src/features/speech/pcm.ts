/**
 * Microphone audio for speech recognition: whisper wants 16 kHz mono
 * 16-bit samples. Recording happens at the device's rate (usually 44.1 or
 * 48 kHz) and is turned into that here.
 */

export const RATE = 16_000;

/** Mono samples at `from` Hz to 16-bit samples at 16 kHz (averaging each
 * output sample's window, so nothing folds back into speech). */
export function toPcm16(input: Float32Array, from: number): Int16Array {
  const ratio = from / RATE;
  const n = Math.floor(input.length / ratio);
  const out = new Int16Array(n);
  for (let i = 0; i < n; i++) {
    const a = Math.floor(i * ratio);
    const b = Math.max(a + 1, Math.floor((i + 1) * ratio));
    let sum = 0;
    for (let j = a; j < b && j < input.length; j++) sum += input[j]!;
    const v = Math.max(-1, Math.min(1, sum / (b - a)));
    out[i] = Math.round(v * 32767);
  }
  return out;
}

/** Joins recorded buffers. */
export function concat(parts: Float32Array[]): Float32Array {
  const out = new Float32Array(parts.reduce((n, p) => n + p.length, 0));
  let at = 0;
  for (const p of parts) {
    out.set(p, at);
    at += p.length;
  }
  return out;
}

export function rms(buf: Float32Array): number {
  let s = 0;
  for (let i = 0; i < buf.length; i++) s += buf[i]! * buf[i]!;
  return Math.sqrt(s / Math.max(1, buf.length));
}

/**
 * Tells when someone has stopped speaking, so dictation can send what was
 * said while they think. Loudness is compared with the quietest recent
 * level (the room), so it works with quiet and noisy microphones.
 */
export class PauseDetector {
  private floor = 1;
  private spoke = false;
  private quietFor = 0;
  private length = 0;

  constructor(
    private readonly pause = 0.7,
    private readonly shortest = 1.2,
    private readonly longest = 25,
  ) {}

  /** Feeds a buffer's loudness and length (seconds). Says when the part
   * so far ends: "send" it (something was said) or "drop" it (silence). */
  push(level: number, seconds: number): "send" | "drop" | null {
    this.length += seconds;
    // The room level drifts up slowly (doubling in about 15 s), so a louder
    // room is learned but speech is not mistaken for it.
    this.floor = Math.min(level, this.floor * (1 + 0.05 * seconds) + 0.00001);
    const speaking = level > Math.max(0.012, this.floor * 3);
    if (speaking) {
      this.spoke = true;
      this.quietFor = 0;
    } else {
      this.quietFor += seconds;
    }
    const pause = this.spoke && this.quietFor >= this.pause && this.length >= this.shortest;
    if (pause || this.length >= this.longest) {
      const said = this.spoke;
      this.reset();
      return said ? "send" : "drop";
    }
    // Long silence before anything is said: let it go.
    if (!this.spoke && this.quietFor >= 3) {
      this.reset();
      return "drop";
    }
    return null;
  }

  /** Whether anything was said since the last part. */
  get heard(): boolean {
    return this.spoke;
  }

  reset() {
    this.spoke = false;
    this.quietFor = 0;
    this.length = 0;
  }
}
