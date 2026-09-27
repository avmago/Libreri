import { useSyncExternalStore } from "react";

function subscribe(callback: () => void) {
  const observer = new MutationObserver(callback);
  observer.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
  return () => observer.disconnect();
}

const isDark = () => {
  const c = document.documentElement.classList;
  return c.contains("dark") || c.contains("hc");
};

/** True while the app shows a dark theme (including high contrast). */
export function useAppDark(): boolean {
  return useSyncExternalStore(subscribe, isDark, () => false);
}
