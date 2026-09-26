import { useCurrentLibrary, WelcomeScreen } from "@/features/library";
import { useSettings } from "@/features/settings";
import { useApplyTheme } from "@/lib/theme";
import { AppShell } from "./AppShell";

export function App() {
  const { data: settings } = useSettings();
  useApplyTheme(settings?.theme ?? "system");

  const { data: library, isPending } = useCurrentLibrary();
  if (isPending) return null;
  return library ? <AppShell library={library} /> : <WelcomeScreen />;
}
