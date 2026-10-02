/**
 * The voices that read aloud: natural voices downloaded in Settings ›
 * Reader (Kokoro and Piper, ADR 0030), and the system's own (Web Speech
 * in the web view: macOS, Windows and many Linux systems), or eSpeak NG
 * through Libreri where the web view has none.
 */
import { commands, unwrap } from "@/lib/ipc";

export interface Voice {
  /** Stable name to remember (voiceURI, eSpeak's voice id, or
   * "kokoro:af_heart" / "piper:de_DE-thorsten-high"). */
  id: string;
  name: string;
  lang: string;
  /** Where it comes from: a natural voice's download, or the system. */
  group: "kokoro" | "piper" | "system";
}

export interface SpeakOptions {
  voice: string | null;
  rate: number;
  lang?: string;
}

export interface Engine {
  kind: "system" | "espeak" | "mixed";
  voices: Voice[];
  /** Resolves true when the text was spoken to the end, false when stopped. */
  speak(text: string, opts: SpeakOptions): Promise<boolean>;
  stop(): void;
  /** Gets the next sentence ready while this one is spoken. */
  prepare?(text: string, opts: SpeakOptions): void;
}

async function webVoices(): Promise<SpeechSynthesisVoice[]> {
  if (typeof speechSynthesis === "undefined") return [];
  const now = speechSynthesis.getVoices();
  if (now.length) return now;
  // Voices arrive a moment after the page loads.
  await new Promise<void>((ok) => {
    const done = () => ok();
    speechSynthesis.addEventListener("voiceschanged", done, { once: true });
    setTimeout(done, 1500);
  });
  return speechSynthesis.getVoices();
}

/** The voice for `id`, or the best one for the book's language. */
function pick(list: SpeechSynthesisVoice[], id: string | null, lang?: string) {
  const chosen = id ? list.find((v) => v.voiceURI === id) : undefined;
  if (chosen) return chosen;
  const want = (lang || navigator.language || "en").toLowerCase();
  const base = want.split("-")[0]!;
  return (
    list.find((v) => v.lang.toLowerCase() === want && v.localService) ??
    list.find((v) => v.lang.toLowerCase().startsWith(base) && v.default) ??
    list.find((v) => v.lang.toLowerCase().startsWith(base)) ??
    list.find((v) => v.default) ??
    list[0]
  );
}

class WebEngine implements Engine {
  kind = "system" as const;
  voices: Voice[];
  private stopped = false;

  constructor(private readonly list: SpeechSynthesisVoice[]) {
    this.voices = list.map((v) => ({
      id: v.voiceURI,
      name: v.name,
      lang: v.lang,
      group: "system" as const,
    }));
  }

  speak(text: string, opts: SpeakOptions): Promise<boolean> {
    this.stopped = false;
    return new Promise((resolve, reject) => {
      const u = new SpeechSynthesisUtterance(text);
      const v = pick(this.list, opts.voice, opts.lang);
      if (v) {
        u.voice = v;
        u.lang = v.lang;
      } else if (opts.lang) u.lang = opts.lang;
      u.rate = Math.min(3, Math.max(0.5, opts.rate));
      let settled = false;
      const done = (ok: boolean) => {
        if (settled) return;
        settled = true;
        clearTimeout(guard);
        resolve(ok && !this.stopped);
      };
      // Some engines never say they finished: go on after a generous time.
      const guard = setTimeout(() => done(true), 4000 + (text.length * 120) / u.rate);
      u.onend = () => done(true);
      u.onerror = (e) => {
        if (e.error === "interrupted" || e.error === "canceled") return done(false);
        // "not-allowed" (WebKit wants a click first), "synthesis-failed"…:
        // say so rather than skip through the book in silence.
        if (settled) return;
        settled = true;
        clearTimeout(guard);
        reject(new Error(`The system voice could not speak (${e.error}).`));
      };
      speechSynthesis.speak(u);
    });
  }

  stop() {
    this.stopped = true;
    speechSynthesis.cancel();
  }
}

class EspeakEngine implements Engine {
  kind = "espeak" as const;
  voices: Voice[];
  constructor(list: Omit<Voice, "group">[]) {
    this.voices = list.map((v) => ({ ...v, group: "system" as const }));
  }

  async speak(text: string, opts: SpeakOptions): Promise<boolean> {
    const voice = opts.voice ?? (opts.lang ? opts.lang.toLowerCase() : null);
    return unwrap(commands.systemSpeak(text, voice, opts.rate)).catch(() => false);
  }

  stop() {
    void commands.systemStopSpeaking();
  }
}

let chosen: Promise<Engine | null> | null = null;

/** The system's engine (found once). */
function systemEngine(again: boolean): Promise<Engine | null> {
  if (again) chosen = null;
  chosen ??= (async () => {
    const web = await webVoices();
    if (web.length) return new WebEngine(web);
    const es = await unwrap(commands.systemVoices()).catch(() => []);
    if (es.length) return new EspeakEngine(es);
    return null;
  })();
  return chosen;
}

export const isNatural = (id: string | null | undefined) =>
  !!id && (id.startsWith("kokoro:") || id.startsWith("piper:"));

/**
 * Natural voices: each sentence is made on this computer as a WAV file and
 * played. The next sentence is made while this one plays.
 */
class NaturalEngine {
  private audio: HTMLAudioElement | null = null;
  private finish: ((ok: boolean) => void) | null = null;
  private cache = new Map<string, Promise<string>>();

  private key(text: string, voice: string, speed: number) {
    return `${voice}|${speed}|${text}`;
  }

  /** The model's own speed (0.5–2); the rest is played faster. */
  private speeds(rate: number) {
    const native = Math.min(2, Math.max(0.5, rate));
    return { native, play: Math.min(3, Math.max(0.5, rate)) / native };
  }

  private make(text: string, voice: string, native: number): Promise<string> {
    const k = this.key(text, voice, native);
    let p = this.cache.get(k);
    if (!p) {
      p = unwrap(commands.speakNatural(voice, text, native)).then((b64) => {
        const bytes = Uint8Array.from(atob(b64), (c) => c.charCodeAt(0));
        return URL.createObjectURL(new Blob([bytes], { type: "audio/wav" }));
      });
      p.catch(() => this.cache.delete(k));
      this.cache.set(k, p);
      // Keep the last few only.
      while (this.cache.size > 4) {
        const first = this.cache.keys().next().value!;
        void this.cache.get(first)?.then(
          (u) => URL.revokeObjectURL(u),
          () => {},
        );
        this.cache.delete(first);
      }
    }
    return p;
  }

  prepare(text: string, voice: string, rate: number) {
    void this.make(text, voice, this.speeds(rate).native).catch(() => {});
  }

  async speak(text: string, voice: string, rate: number): Promise<boolean> {
    this.stop();
    const a = player();
    const { native, play } = this.speeds(rate);
    let stopped = false;
    let fail: (e: Error) => void = () => {};
    const ended = new Promise<boolean>((resolve, reject) => {
      fail = reject;
      this.finish = (ok) => {
        stopped = !ok;
        resolve(ok);
      };
    });
    // Errors (no eSpeak NG, a damaged download) reach the caller.
    const url = await Promise.race([this.make(text, voice, native), ended.then(() => null)]);
    if (url === null || stopped) return false;
    a.src = url;
    a.playbackRate = play;
    a.preservesPitch = true;
    this.audio = a;
    const done = (ok: boolean) => {
      if (this.audio !== a) return;
      this.audio = null;
      const f = this.finish;
      this.finish = null;
      f?.(ok);
    };
    a.onended = () => done(true);
    a.onerror = () => done(true);
    a.play().catch((e: Error) => {
      if (this.audio !== a) return;
      this.audio = null;
      this.finish = null;
      fail(new Error(`The voice could not be played (${e.name}).`));
    });
    return ended;
  }

  stop() {
    this.audio?.pause();
    this.audio?.removeAttribute("src");
    this.audio = null;
    const f = this.finish;
    this.finish = null;
    f?.(false);
  }
}

const natural = new NaturalEngine();

/** One audio element for natural voices, kept so that WebKit, which lets
 * a page play sound only after a click, keeps allowing it. */
let shared: HTMLAudioElement | null = null;
function player(): HTMLAudioElement {
  shared ??= new Audio();
  return shared;
}

/** A tenth of a second of silence, as a WAV file. */
function silence(): Blob {
  const n = 2205;
  const b = new DataView(new ArrayBuffer(44 + n * 2));
  const w = (o: number, t: string) => [...t].forEach((c, i) => b.setUint8(o + i, c.charCodeAt(0)));
  w(0, "RIFF");
  b.setUint32(4, 36 + n * 2, true);
  w(8, "WAVEfmt ");
  b.setUint32(16, 16, true);
  b.setUint16(20, 1, true);
  b.setUint16(22, 1, true);
  b.setUint32(24, 22050, true);
  b.setUint32(28, 44100, true);
  b.setUint16(32, 2, true);
  b.setUint16(34, 16, true);
  w(36, "data");
  b.setUint32(40, n * 2, true);
  return new Blob([b.buffer], { type: "audio/wav" });
}

/**
 * Call straight from the click that starts reading (before anything is
 * awaited): WebKit (macOS) allows speech and sound only right after a
 * click, and reading a PDF's text first takes too long for that.
 */
export function primeSpeech() {
  try {
    if (typeof speechSynthesis !== "undefined") {
      const u = new SpeechSynthesisUtterance(" ");
      u.volume = 0;
      speechSynthesis.speak(u);
    }
  } catch {
    /* no system speech */
  }
  try {
    const a = player();
    if (a.src.startsWith("blob:") && !a.paused) return;
    a.src = URL.createObjectURL(silence());
    void a.play().catch(() => {});
  } catch {
    /* no audio */
  }
}

/**
 * Natural voices that are switched on, with the system's: what the voice
 * menu offers. `speak` sends each voice to the engine it belongs to.
 */
class MixedEngine implements Engine {
  kind = "mixed" as const;
  voices: Voice[];

  constructor(
    private readonly system: Engine | null,
    naturalVoices: Voice[],
  ) {
    this.voices = [...naturalVoices, ...(system?.voices ?? [])];
  }

  speak(text: string, opts: SpeakOptions): Promise<boolean> {
    if (isNatural(opts.voice)) return natural.speak(text, opts.voice!, opts.rate);
    if (this.system) return this.system.speak(text, opts);
    return Promise.resolve(false);
  }

  prepare(text: string, opts: SpeakOptions) {
    if (isNatural(opts.voice)) natural.prepare(text, opts.voice!, opts.rate);
  }

  stop() {
    natural.stop();
    this.system?.stop();
  }
}

/** The voices of this computer: natural ones (looked up each time, as
 * they are downloaded or switched off in Settings) and the system's. */
export async function getEngine(again = false): Promise<Engine | null> {
  const system = await systemEngine(again);
  const list = await commands.naturalVoices().catch(() => null);
  const voices: Voice[] = (list?.voices ?? [])
    .filter((v) => v.on)
    .map((v) => ({
      id: v.id,
      name: v.name,
      lang: v.lang,
      group: v.id.startsWith("kokoro:") ? "kokoro" : "piper",
    }));
  if (!voices.length) return system;
  return new MixedEngine(system, voices);
}
