import { Toaster } from "sonner";
import { useCloseLibrary, useCurrentLibrary, WelcomeScreen } from "@/features/library";
import {
  ProfilePicker,
  RecoveryCodeHost,
  useCurrentSession,
  useSessionEvents,
} from "@/features/profiles";
import { useSettings } from "@/features/settings";
import type { LibrarySummary } from "@/lib/ipc";
import { useApplyTheme } from "@/lib/theme";
import { AppShell } from "./AppShell";

export function App() {
  const { data: settings } = useSettings();
  const theme = settings?.theme ?? "system";
  useApplyTheme(theme, settings?.accent ?? null);

  const { data: library, isPending } = useCurrentLibrary();
  if (isPending) return null;
  return (
    <>
      {library ? <LibraryGate library={library} /> : <WelcomeScreen />}
      <RecoveryCodeHost />
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

/** Asks who is reading, then shows the library as that profile. */
function LibraryGate({ library }: { library: LibrarySummary }) {
  const { data: session, isPending } = useCurrentSession();
  const closeLibrary = useCloseLibrary();
  useSessionEvents();
  if (isPending) return null;
  if (!session) {
    return <ProfilePicker library={library} onCloseLibrary={() => closeLibrary.mutate()} />;
  }
  // A new key per profile: nothing from one person's view leaks into the next.
  return <AppShell key={session.profile.id} library={library} session={session} />;
}
