/** How the speech feature opens Settings › Speech (set by the app shell,
 * which owns navigation). */
let opener: (() => void) | null = null;

export function setSpeechSettingsOpener(fn: () => void) {
  opener = fn;
}

export function openSpeechSettings() {
  opener?.();
}

const ua = typeof navigator === "undefined" ? "" : navigator.userAgent;

/** How to start the system's own dictation, when it has one. */
export const SYSTEM_DICTATION: string | null = /Mac/.test(ua)
  ? "macOS Dictation (press the Fn or Globe key twice)"
  : /Windows/.test(ua)
    ? "Windows voice typing (Windows key + H)"
    : null;
