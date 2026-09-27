import { useEffect, useState } from "react";
import type { Theme } from "@/lib/ipc";

/** Classes to put on <html> for a theme preference. */
export function themeClasses(theme: Theme, systemDark: boolean): string[] {
  switch (theme) {
    case "light":
      return [];
    case "dark":
      return ["dark"];
    case "highContrast":
      return ["dark", "hc"];
    case "system":
      return systemDark ? ["dark"] : [];
  }
}

function useSystemDark(): boolean {
  const query = "(prefers-color-scheme: dark)";
  const [dark, setDark] = useState(() => window.matchMedia?.(query).matches ?? false);
  useEffect(() => {
    const mql = window.matchMedia?.(query);
    if (!mql) return;
    const onChange = (e: MediaQueryListEvent) => setDark(e.matches);
    mql.addEventListener("change", onChange);
    return () => mql.removeEventListener("change", onChange);
  }, []);
  return dark;
}

/** Accent colours offered in Settings; `null` is the default (black). */
export const ACCENTS: { value: string | null; label: string }[] = [
  { value: null, label: "Black" },
  { value: "#2563eb", label: "Blue" },
  { value: "#4f46e5", label: "Indigo" },
  { value: "#7c3aed", label: "Violet" },
  { value: "#db2777", label: "Pink" },
  { value: "#dc2626", label: "Red" },
  { value: "#ea580c", label: "Orange" },
  { value: "#15803d", label: "Green" },
  { value: "#0f766e", label: "Teal" },
];

/**
 * Applies the theme (and accent colour) to the document and follows the OS
 * when set to System. High contrast keeps its own colours.
 */
export function useApplyTheme(theme: Theme, accent: string | null = null): void {
  const systemDark = useSystemDark();
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("dark", "hc");
    const classes = themeClasses(theme, systemDark);
    root.classList.add(...classes);
    const useAccent = accent && !classes.includes("hc");
    for (const v of ["--primary", "--ring", "--accent"]) {
      if (useAccent) root.style.setProperty(v, accent);
      else root.style.removeProperty(v);
    }
    for (const v of ["--primary-foreground", "--accent-foreground"]) {
      if (useAccent) root.style.setProperty(v, "#ffffff");
      else root.style.removeProperty(v);
    }
  }, [theme, systemDark, accent]);
}

/** The next theme when cycling with the toggle shortcut. */
export function nextTheme(theme: Theme): Theme {
  const order: Theme[] = ["system", "light", "dark", "highContrast"];
  return order[(order.indexOf(theme) + 1) % order.length] ?? "system";
}
