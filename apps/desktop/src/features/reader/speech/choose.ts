import type { Voice } from "./engine";

/** "en-US" → "en". */
export const baseLang = (lang: string | null | undefined) =>
  (lang || "").toLowerCase().split(/[-_]/)[0] || "";

/** The voices Libreri suggests first when nothing is chosen: the most
 * natural Kokoro voice of each language. */
const PREFERRED: Record<string, string[]> = {
  "en-us": ["kokoro:af_heart", "kokoro:af_bella", "kokoro:am_michael"],
  "en-gb": ["kokoro:bf_emma", "kokoro:bm_george"],
  en: ["kokoro:af_heart", "kokoro:bf_emma"],
  es: ["kokoro:ef_dora"],
  fr: ["kokoro:ff_siwis"],
  hi: ["kokoro:hf_alpha"],
  it: ["kokoro:if_sara"],
  ja: ["kokoro:jf_alpha"],
  pt: ["kokoro:pf_dora"],
  zh: ["kokoro:zf_xiaoxiao"],
};

/**
 * The voice to read a book in `lang` with: the one chosen for that
 * language, else the one last chosen (when it speaks the language, or
 * the book does not say), else the best natural voice for it, else none
 * (the system picks).
 */
export function chooseVoice(
  voices: Voice[],
  prefs: { voice: string | null; voiceFor?: Record<string, string> },
  lang: string | undefined,
): string | null {
  const has = (id: string | null | undefined) => !!id && voices.some((v) => v.id === id);
  const base = baseLang(lang || (typeof navigator === "undefined" ? "" : navigator.language));
  const forLang = prefs.voiceFor?.[base];
  if (has(forLang)) return forLang!;
  const last = voices.find((v) => v.id === prefs.voice);
  if (last && (!lang || baseLang(last.lang) === base)) return last.id;
  const full = (lang || "").toLowerCase();
  for (const id of [...(PREFERRED[full] ?? []), ...(PREFERRED[base] ?? [])]) if (has(id)) return id;
  const natural = voices.find((v) => v.group !== "system" && baseLang(v.lang) === base);
  if (natural) return natural.id;
  if (last && !voices.some((v) => baseLang(v.lang) === base)) return last.id;
  return null;
}
