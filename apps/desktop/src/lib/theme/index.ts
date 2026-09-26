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

/** Applies the theme to the document and follows the OS when set to System. */
export function useApplyTheme(theme: Theme): void {
  const systemDark = useSystemDark();
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("dark", "hc");
    root.classList.add(...themeClasses(theme, systemDark));
  }, [theme, systemDark]);
}

/** The next theme when cycling with the toggle shortcut. */
export function nextTheme(theme: Theme): Theme {
  const order: Theme[] = ["system", "light", "dark", "highContrast"];
  return order[(order.indexOf(theme) + 1) % order.length] ?? "system";
}
