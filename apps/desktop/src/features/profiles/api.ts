import { useMutation, useQuery, useQueryClient, type QueryClient } from "@tanstack/react-query";
import { commands, unwrap, type NewProfile, type ProfileKind, type SessionDto } from "@/lib/ipc";
import { useRecoveryCode } from "./store";

/** Under the library key, so they are dropped when another library opens. */
export const sessionKey = ["lib", "session"] as const;
export const profilesKey = ["lib", "profiles"] as const;
export const collectionsKey = ["lib", "collections"] as const;

export function useCurrentSession() {
  return useQuery({ queryKey: sessionKey, queryFn: () => unwrap(commands.currentSession()) });
}

export function useProfiles() {
  return useQuery({ queryKey: profilesKey, queryFn: () => unwrap(commands.listProfiles()) });
}

/** Everything read for one profile is thrown away when someone else signs in. */
function switchTo(qc: QueryClient, session: SessionDto | null) {
  qc.removeQueries({
    queryKey: ["lib"],
    predicate: (q) => q.queryKey[1] !== "session",
  });
  qc.setQueryData(sessionKey, session);
}

export function useSignIn() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ id, pin }: { id: string; pin?: string }) =>
      unwrap(commands.signIn(id, pin ?? null)),
    onSuccess: (session) => switchTo(qc, session),
    onError: () => void qc.invalidateQueries({ queryKey: profilesKey }),
  });
}

export function useSignOut() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.signOut()),
    onSuccess: () => switchTo(qc, null),
  });
}

export function useRecoverOwner() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: ({ code, pin }: { code: string; pin: string }) =>
      unwrap(commands.recoverOwner(code, pin)),
    onSuccess: async (nextCode) => {
      // Shown by the app, because the picker that asked goes away.
      useRecoveryCode.getState().show(nextCode);
      const session = await unwrap(commands.currentSession());
      switchTo(qc, session);
    },
  });
}

function useProfileMutation<A, R>(fn: (a: A) => Promise<R>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: profilesKey });
      void qc.invalidateQueries({ queryKey: sessionKey });
    },
  });
}

export function useCreateProfile() {
  return useProfileMutation((p: NewProfile) => unwrap(commands.createProfile(p)));
}

export function useUpdateProfile() {
  return useProfileMutation((p: { id: string; name: string; colour: string; kind: ProfileKind }) =>
    unwrap(commands.updateProfile(p.id, p.name, p.colour, p.kind)),
  );
}

export function useSetPin() {
  return useProfileMutation(
    (p: { id: string; currentPin?: string | null; newPin: string | null }) =>
      unwrap(commands.setProfilePin(p.id, p.currentPin ?? null, p.newPin)),
  );
}

export function useDeleteProfile() {
  return useProfileMutation((id: string) => unwrap(commands.deleteProfile(id)));
}

export function useSetAllowedFolders() {
  return useProfileMutation((p: { id: string; folders: string[] }) =>
    unwrap(commands.setAllowedFolders(p.id, p.folders)),
  );
}

/** What the signed-in profile may do (all false while signed out). */
export function usePermissions() {
  const { data } = useCurrentSession();
  return {
    editLibrary: data?.canEditLibrary ?? false,
    manageProfiles: data?.canManageProfiles ?? false,
    keepsData: data?.keepsData ?? false,
    kind: data?.profile.kind ?? null,
  };
}
