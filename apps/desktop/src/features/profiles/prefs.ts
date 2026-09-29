/**
 * Preferences that belong to a profile (not to the computer): shortcuts,
 * auto-lock, reader and notes defaults, library view. Stored in the
 * library with the profile and saved through `save_prefs`.
 */
import { create } from "zustand";
import { commands } from "@/lib/ipc";
import type { HighlightColor } from "@/lib/ipc";

export type LibraryViewMode = "grid" | "list" | "shelf";

export interface ProfilePrefs {
  version: 1;
  /** Lock after this many idle minutes (profiles with a PIN). 0 = never. */
  autoLockMinutes: number;
  /** Shortcut overrides by action id; `null` removes the shortcut. */
  shortcuts: Record<string, string | null>;
  /** j/k, h/l, gg/G, /, n/N in the reader and the library. */
  vimKeys: boolean;
  library: {
    view: LibraryViewMode;
    sidebarCollapsed: boolean;
    confirmTrash: boolean;
  };
  reader: {
    /** 50–150, applied to PDF pages and comics. */
    brightness: number;
    contrast: number;
    /** 60–220 (% of normal) for reflowable books. */
    fontScale: number;
    lineHeight: number;
    /** Open books where you stopped reading. */
    resume: boolean;
  };
  /** ADHD reading (Settings › Reader). The reader's Aa menu can pause it
   * for a while without changing these. */
  adhd: {
    /** The start of each word bold (books whose text Libreri lays out). */
    bionic: boolean;
    /** Share of each word made bold, 0.3–0.7. */
    fixation: number;
    /** Opacity of the rest of the word, 0.4–1. */
    fade: number;
    /** Highlights the line under the pointer. */
    line: boolean;
    /** Dims the page except a strip around the pointer. */
    mask: boolean;
    /** Height of that strip in pixels. */
    maskHeight: number;
  };
  notes: {
    defaultColor: HighlightColor;
    /** Add a link back to the page when quoting into the notebook. */
    linkQuotes: boolean;
  };
  markup: {
    /** Saved signatures (PNG data URLs, trimmed) with width / height. */
    signatures: { id: string; src: string; aspect: number }[];
    /** Stamps you made, shown after the built-in ones. */
    stamps: string[];
    color: string;
    width: number;
    snap: boolean;
  };
  listening: {
    /** Read-aloud speed, 0.5–3. */
    speechRate: number;
    /** Voice name (system voices) or eSpeak voice id; null = the default. */
    voice: string | null;
    /** Turn pages to follow what is being read. */
    follow: boolean;
    /** Audiobook speed, 0.5–3. */
    audioRate: number;
    /** Seconds the back and forward buttons skip. */
    skipBack: number;
    skipForward: number;
  };
  writing: {
    /** Libreri's spell check in notes, comments and text boxes. */
    spellCheck: boolean;
    /** Dictionaries used together ("en-US", "fr", …). */
    languages: string[];
    /** Offer ways to finish the word being typed (Tab takes one). */
    complete: boolean;
  };
}

/** English as the system prefers it. */
function defaultEnglish(): string {
  const l = typeof navigator === "undefined" ? "" : navigator.language;
  return /^en-(GB|IE|IN|NZ)/i.test(l) ? "en-GB" : "en-US";
}

export const DEFAULT_PROFILE_PREFS: ProfilePrefs = {
  version: 1,
  autoLockMinutes: 15,
  shortcuts: {},
  vimKeys: false,
  library: { view: "grid", sidebarCollapsed: false, confirmTrash: true },
  reader: { brightness: 100, contrast: 100, fontScale: 100, lineHeight: 1.55, resume: true },
  adhd: { bionic: false, fixation: 0.5, fade: 1, line: false, mask: false, maskHeight: 120 },
  notes: { defaultColor: "yellow", linkQuotes: true },
  markup: { signatures: [], stamps: [], color: "#dc2626", width: 0.0025, snap: true },
  listening: {
    speechRate: 1,
    voice: null,
    follow: true,
    audioRate: 1,
    skipBack: 15,
    skipForward: 30,
  },
  writing: { spellCheck: true, languages: [defaultEnglish()], complete: true },
};

function safeParse(json: string | null | undefined): Partial<ProfilePrefs> {
  try {
    return json ? (JSON.parse(json) as Partial<ProfilePrefs>) : {};
  } catch {
    return {};
  }
}

/** Reads stored JSON, keeping defaults for anything missing or damaged. */
export function parsePrefs(json: string | null | undefined): ProfilePrefs {
  const raw = safeParse(json);
  const d = DEFAULT_PROFILE_PREFS;
  return {
    ...d,
    ...raw,
    version: 1,
    shortcuts: { ...d.shortcuts, ...raw.shortcuts },
    library: { ...d.library, ...raw.library },
    reader: { ...d.reader, ...raw.reader },
    adhd: { ...d.adhd, ...raw.adhd },
    notes: { ...d.notes, ...raw.notes },
    markup: { ...d.markup, ...raw.markup },
    listening: { ...d.listening, ...raw.listening },
    writing: { ...d.writing, ...raw.writing },
  };
}

type DeepPartial<T> = { [K in keyof T]?: T[K] extends object ? Partial<T[K]> : T[K] };

interface PrefsState {
  prefs: ProfilePrefs;
  /** False until the signed-in profile's prefs are loaded. */
  loaded: boolean;
  /** Keep changes in memory only (guests). */
  persist: boolean;
  load: (json: string, persist: boolean) => void;
  update: (change: DeepPartial<ProfilePrefs>) => void;
}

let timer: ReturnType<typeof setTimeout> | undefined;

function save(prefs: ProfilePrefs) {
  clearTimeout(timer);
  timer = setTimeout(() => void commands.savePrefs(JSON.stringify(prefs)), 300);
}

export const useProfilePrefs = create<PrefsState>((set, get) => ({
  prefs: DEFAULT_PROFILE_PREFS,
  loaded: false,
  persist: false,
  load: (json, persist) => set({ prefs: parsePrefs(json), loaded: true, persist }),
  update: (change) => {
    const cur = get().prefs;
    const next: ProfilePrefs = {
      ...cur,
      ...change,
      shortcuts: change.shortcuts
        ? ({ ...change.shortcuts } as ProfilePrefs["shortcuts"])
        : cur.shortcuts,
      library: { ...cur.library, ...change.library },
      reader: { ...cur.reader, ...change.reader },
      adhd: { ...cur.adhd, ...change.adhd },
      notes: { ...cur.notes, ...change.notes },
      markup: { ...cur.markup, ...change.markup } as ProfilePrefs["markup"],
      listening: { ...cur.listening, ...change.listening },
      writing: { ...cur.writing, ...change.writing } as ProfilePrefs["writing"],
    };
    set({ prefs: next });
    if (get().persist) save(next);
  },
}));

/** Writes any pending change now (before signing out). */
export async function flushPrefs(): Promise<void> {
  if (timer === undefined) return;
  clearTimeout(timer);
  timer = undefined;
  const { prefs, persist } = useProfilePrefs.getState();
  if (persist) await commands.savePrefs(JSON.stringify(prefs));
}
