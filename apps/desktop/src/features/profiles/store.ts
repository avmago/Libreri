import { create } from "zustand";

/**
 * A recovery code to show once. Kept outside the component that received
 * it, because signing in replaces the screen that asked for it.
 */
export const useRecoveryCode = create<{ code: string | null; show: (c: string | null) => void }>(
  (set) => ({ code: null, show: (code) => set({ code }) }),
);
