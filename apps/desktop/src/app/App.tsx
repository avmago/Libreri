import { Toaster } from "sonner";
import { useCurrentLibrary, WelcomeScreen } from "@/features/library";
import { useSettings } from "@/features/settings";
import { useApplyTheme } from "@/lib/theme";
import { AppShell } from "./AppShell";

export function App() {
  const { data: settings } = useSettings();
  const theme = settings?.theme ?? "system";
  useApplyTheme(theme);

  const { data: library, isPending } = useCurrentLibrary();
  if (isPending) return null;
  return (
    <>
      {library ? <AppShell library={library} /> : <WelcomeScreen />}
      <Toaster
        position="bottom-right"
        theme={theme === "system" ? "system" : theme === "light" ? "light" : "dark"}
        toastOptions={{
          classNames: {
            toast:
              "!bg-popover !text-popover-foreground !border !border-border !font-sans !text-[13px]",
            description: "!text-muted-foreground whitespace-pre-line",
          },
        }}
      />
    </>
  );
}
