/**
 * Between an open audiobook and the open text it reads: where each text
 * book is (for "add a sync point here"), where the text should follow the
 * audio, and requests to play from a place in the text.
 */
import { create } from "zustand";

export interface Place {
  locator: string;
  progress: number;
  label: string;
}

interface ListeningState {
  /** Where each open book is. */
  places: Record<string, Place>;
  /** Text book id → where to follow the audiobook to (0–1). */
  follow: Record<string, { progress: number; at: number }>;
  /** Audiobook id → play from this many seconds. */
  playFrom: Record<string, { t: number; at: number }>;
  setPlace: (bookId: string, place: Place) => void;
  followTo: (bookId: string, progress: number) => void;
  requestPlay: (audioId: string, t: number) => void;
  clearPlay: (audioId: string) => void;
}

export const useListening = create<ListeningState>((set) => ({
  places: {},
  follow: {},
  playFrom: {},
  setPlace: (bookId, place) => set((s) => ({ places: { ...s.places, [bookId]: place } })),
  followTo: (bookId, progress) =>
    set((s) => ({ follow: { ...s.follow, [bookId]: { progress, at: Date.now() } } })),
  requestPlay: (audioId, t) =>
    set((s) => ({ playFrom: { ...s.playFrom, [audioId]: { t, at: Date.now() } } })),
  clearPlay: (audioId) =>
    set((s) => {
      const playFrom = { ...s.playFrom };
      delete playFrom[audioId];
      return { playFrom };
    }),
}));
