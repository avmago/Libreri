/**
 * The floating player (read aloud, audiobook, podcast) folded into a small
 * box at the bottom left or right. Shared by the book's player and the
 * window's podcast player, and kept on this computer.
 */
import { useEffect, useRef } from "react";
import { create } from "zustand";

export type FloatingSide = "left" | "right";

const KEY = "libreri.floatingPlayer";

function load(): { collapsed: boolean; side: FloatingSide } {
  try {
    const v = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Partial<{
      collapsed: boolean;
      side: FloatingSide;
    }>;
    return { collapsed: v.collapsed === true, side: v.side === "left" ? "left" : "right" };
  } catch {
    return { collapsed: false, side: "right" };
  }
}

interface FloatingState {
  collapsed: boolean;
  side: FloatingSide;
  setCollapsed: (collapsed: boolean) => void;
  setSide: (side: FloatingSide) => void;
}

export const useFloatingBar = create<FloatingState>((set, get) => {
  const keep = () => {
    try {
      localStorage.setItem(KEY, JSON.stringify({ collapsed: get().collapsed, side: get().side }));
    } catch {
      /* not kept */
    }
  };
  return {
    ...load(),
    setCollapsed: (collapsed) => {
      set({ collapsed });
      keep();
    },
    setSide: (side) => {
      set({ side });
      keep();
    },
  };
});

interface MiniPlay {
  playing: boolean;
  busy: boolean;
  toggle: (() => void) | null;
}

/** What the folded box's play button controls (the part shown). */
export const useMiniPlay = create<MiniPlay>(() => ({ playing: false, busy: false, toggle: null }));

/** A player part says whether it is playing and how to toggle it. */
export function useReportPlay(playing: boolean, busy: boolean, toggle: () => void) {
  const ref = useRef(toggle);
  useEffect(() => {
    ref.current = toggle;
  });
  useEffect(() => {
    useMiniPlay.setState({ playing, busy, toggle: () => ref.current() });
  }, [playing, busy]);
  useEffect(() => () => useMiniPlay.setState({ playing: false, busy: false, toggle: null }), []);
}
