/**
 * The voices that read aloud: the system's own (Web Speech in the web
 * view: macOS, Windows and many Linux systems), or eSpeak NG through
 * Libreri where the web view has none.
 */
import { commands, unwrap } from "@/lib/ipc";

export interface Voice {
  /** Stable name to remember (voiceURI, or eSpeak's voice id). */
  id: string;
  name: string;
  lang: string;
}

export interface SpeakOptions {
  voice: string | null;
  rate: number;
  lang?: string;
}

export interface Engine {
  kind: "system" | "espeak";
  voices: Voice[];
  /** Resolves true when the text was spoken to the end, false when stopped. */
  speak(text: string, opts: SpeakOptions): Promise<boolean>;
  stop(): void;
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
    this.voices = list.map((v) => ({ id: v.voiceURI, name: v.name, lang: v.lang }));
  }

  speak(text: string, opts: SpeakOptions): Promise<boolean> {
    this.stopped = false;
    return new Promise((resolve) => {
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
      u.onerror = (e) => done(e.error === "interrupted" || e.error === "canceled" ? false : true);
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
  constructor(public voices: Voice[]) {}

  async speak(text: string, opts: SpeakOptions): Promise<boolean> {
    const voice = opts.voice ?? (opts.lang ? opts.lang.toLowerCase() : null);
    return unwrap(commands.systemSpeak(text, voice, opts.rate)).catch(() => false);
  }

  stop() {
    void commands.systemStopSpeaking();
  }
}

let chosen: Promise<Engine | null> | null = null;

/** The engine to use on this computer (found once). */
export function getEngine(again = false): Promise<Engine | null> {
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
