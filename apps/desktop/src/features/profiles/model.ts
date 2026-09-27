import type { ProfileKind } from "@/lib/ipc";

/** Avatar colours (names are stored; hex values are for display only). */
export const PROFILE_COLOURS: Record<string, string> = {
  graphite: "#52525b",
  red: "#dc2626",
  orange: "#ea580c",
  amber: "#b45309",
  green: "#15803d",
  teal: "#0f766e",
  blue: "#2563eb",
  indigo: "#4f46e5",
  violet: "#7c3aed",
  pink: "#db2777",
};

export const COLOUR_NAMES = Object.keys(PROFILE_COLOURS);

export function colourOf(name: string): string {
  return PROFILE_COLOURS[name] ?? PROFILE_COLOURS.graphite!;
}

/** "Jane Smith" → "JS", "Guest" → "G". */
export function initials(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean);
  const letters = words.length > 1 ? [words[0]!, words[words.length - 1]!] : words;
  return letters.map((w) => [...w][0]!.toUpperCase()).join("") || "?";
}

export const KIND_LABELS: Record<ProfileKind, string> = {
  owner: "Owner",
  standard: "Standard",
  kids: "Kids",
  guest: "Guest",
};

export const KIND_HINTS: Record<ProfileKind, string> = {
  owner: "Manages profiles and the whole library.",
  standard: "Reads, makes notes and organises the library.",
  kids: "Reads and makes notes in the folders you choose. Cannot import, edit or delete.",
  guest: "Reads without a profile. Nothing is kept after they leave.",
};

/** Mirrors the Rust rule: six digits, not a run, repeat or 121212 pattern. */
export function pinProblem(pin: string): string | null {
  if (!/^\d{6}$/.test(pin)) return "A PIN is exactly 6 digits.";
  const d = [...pin].map(Number);
  const steps = d.slice(1).map((v, i) => v - d[i]!);
  const run = steps.every((s) => s === steps[0]) && [0, 1, -1].includes(steps[0]!);
  const alternating = d[0] === d[2] && d[2] === d[4] && d[1] === d[3] && d[3] === d[5];
  if (run || alternating) return "Too easy to guess. Avoid repeated digits and runs like 123456.";
  return null;
}

/** "in 30 seconds", "in 5 minutes". */
export function waitText(seconds: number): string {
  if (seconds >= 3600) return `${Math.ceil(seconds / 3600)} h`;
  if (seconds >= 60) return `${Math.ceil(seconds / 60)} min`;
  return `${Math.max(1, Math.ceil(seconds))} s`;
}
