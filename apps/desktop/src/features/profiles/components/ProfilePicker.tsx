import { useEffect, useState } from "react";
import { LibreriMark } from "@/components/LibreriMark";
import { ArrowLeft, KeyRound, Lock, LogOut } from "lucide-react";
import { Button } from "@/components/ui/button";
import { isIpcError, type LibrarySummary, type ProfileDto } from "@/lib/ipc";
import { useProfiles, useSignIn } from "../api";
import { KIND_LABELS, waitText } from "../model";
import { PinPad } from "./PinPad";
import { ProfileAvatar } from "./ProfileAvatar";
import { RecoveryDialog } from "./RecoveryDialog";

/** "Who's reading?" — shown when a library opens with several profiles or a PIN. */
export function ProfilePicker({
  library,
  onCloseLibrary,
}: {
  library: LibrarySummary;
  onCloseLibrary: () => void;
}) {
  const { data: profiles = [], isPending } = useProfiles();
  const [chosen, setChosen] = useState<ProfileDto | null>(null);
  const signIn = useSignIn();

  const choose = (p: ProfileDto) => {
    if (p.hasPin) setChosen(p);
    else signIn.mutate({ id: p.id });
  };

  // Number keys 1–9 pick a profile.
  useEffect(() => {
    if (chosen) return;
    const onKey = (e: KeyboardEvent) => {
      const n = Number(e.key);
      if (Number.isInteger(n) && n >= 1 && n <= profiles.length && !e.ctrlKey && !e.metaKey) {
        choose(profiles[n - 1]!);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  if (isPending) return null;

  return (
    <main className="flex h-full flex-col items-center justify-center gap-10 overflow-auto bg-sidebar px-6 py-10">
      {chosen ? (
        <Unlock
          profile={profiles.find((p) => p.id === chosen.id) ?? chosen}
          onBack={() => setChosen(null)}
        />
      ) : (
        <>
          <div className="flex flex-col items-center gap-2 text-center">
            <span className="flex items-center gap-2 text-[13px] text-muted-foreground">
              <LibreriMark className="size-5" /> {library.name}
            </span>
            <h1 className="text-[28px] font-semibold tracking-tight">Who's reading?</h1>
          </div>
          <ul className="flex max-w-3xl flex-wrap justify-center gap-5" aria-label="Profiles">
            {profiles.map((p, i) => (
              <li key={p.id}>
                <button
                  type="button"
                  onClick={() => choose(p)}
                  disabled={signIn.isPending}
                  className="group flex w-36 flex-col items-center gap-3 rounded-xl p-4 outline-none hover:bg-background focus-visible:bg-background focus-visible:ring-2 focus-visible:ring-ring"
                >
                  <ProfileAvatar
                    name={p.name}
                    colour={p.colour}
                    guest={p.kind === "guest"}
                    size={84}
                  />
                  <span className="flex max-w-full flex-col items-center gap-0.5">
                    <span className="flex max-w-full items-center gap-1.5 truncate text-[15px] font-medium">
                      {p.name}
                      {p.hasPin && (
                        <Lock
                          className="size-3.5 shrink-0 text-muted-foreground"
                          aria-label="PIN"
                        />
                      )}
                    </span>
                    <span className="text-xs text-muted-foreground">
                      {p.kind === "standard" ? " " : KIND_LABELS[p.kind]}
                      <span className="sr-only">, press {i + 1}</span>
                    </span>
                  </span>
                </button>
              </li>
            ))}
          </ul>
          {signIn.error && <p className="text-destructive">{signIn.error.message}</p>}
          <Button variant="ghost" onClick={onCloseLibrary}>
            <LogOut /> Close library
          </Button>
        </>
      )}
    </main>
  );
}

function Unlock({ profile, onBack }: { profile: ProfileDto; onBack: () => void }) {
  const signIn = useSignIn();
  const [attempt, setAttempt] = useState(0);
  const [wait, setWait] = useState(profile.pinWait);
  const [recovering, setRecovering] = useState(false);

  // A new wait from Rust (after a wrong PIN) replaces the countdown.
  const [seenWait, setSeenWait] = useState(profile.pinWait);
  if (profile.pinWait !== seenWait) {
    setSeenWait(profile.pinWait);
    setWait(profile.pinWait);
  }
  useEffect(() => {
    if (wait <= 0) return;
    const t = setInterval(() => setWait((w) => Math.max(0, w - 1)), 1000);
    return () => clearInterval(t);
  }, [wait]);

  useEffect(() => {
    // Esc goes back, unless a dialog on top (recovery) used it already.
    if (recovering) return;
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && !e.defaultPrevented && onBack();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onBack, recovering]);

  const submit = (pin: string) =>
    signIn.mutate(
      { id: profile.id, pin },
      {
        onError: (e) => {
          setAttempt((a) => a + 1);
          if (isIpcError(e, "pinLocked") || isIpcError(e, "wrongPin")) {
            const m = /(\d+) (second|minute|hour)/.exec(e.message);
            if (m) {
              const n = Number(m[1]);
              setWait(m[2] === "hour" ? n * 3600 : m[2] === "minute" ? n * 60 : n);
            }
          }
        },
      },
    );

  const locked = wait > 0;
  const error = locked
    ? `Too many wrong PINs. Try again in ${waitText(wait)}.`
    : signIn.error
      ? signIn.error.message
      : null;

  return (
    <section className="flex flex-col items-center gap-6" aria-label={`Unlock ${profile.name}`}>
      <div className="flex flex-col items-center gap-3">
        <ProfileAvatar name={profile.name} colour={profile.colour} size={72} />
        <h1 className="text-xl font-semibold">{profile.name}</h1>
      </div>
      <PinPad
        onSubmit={submit}
        disabled={signIn.isPending || locked}
        error={error}
        resetKey={attempt}
      />
      <div className="flex gap-2">
        <Button variant="ghost" onClick={onBack}>
          <ArrowLeft /> Back
        </Button>
        {profile.kind === "owner" ? (
          <Button variant="ghost" onClick={() => setRecovering(true)}>
            <KeyRound /> Forgot PIN?
          </Button>
        ) : (
          <p className="flex items-center px-2 text-xs text-muted-foreground">
            Forgot your PIN? The library's owner can reset it.
          </p>
        )}
      </div>
      <RecoveryDialog open={recovering} onOpenChange={setRecovering} />
    </section>
  );
}
