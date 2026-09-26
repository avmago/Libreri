import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap, type LibrarySummary } from "@/lib/ipc";

export const currentLibraryKey = ["library", "current"] as const;
const settingsKey = ["settings"] as const;

export function useCurrentLibrary() {
  return useQuery({ queryKey: currentLibraryKey, queryFn: () => commands.currentLibrary() });
}

function useLibraryMutation<A>(fn: (args: A) => Promise<LibrarySummary>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: (library) => {
      qc.setQueryData(currentLibraryKey, library);
      // The recent-libraries list changed too.
      void qc.invalidateQueries({ queryKey: settingsKey });
    },
  });
}

export function useCreateLibrary() {
  return useLibraryMutation(({ path, name }: { path: string; name?: string }) =>
    unwrap(commands.createLibrary(path, name ?? null)),
  );
}

export function useOpenLibrary() {
  return useLibraryMutation(({ path, force = false }: { path: string; force?: boolean }) =>
    unwrap(commands.openLibrary(path, force)),
  );
}

export function useCloseLibrary() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => commands.closeLibrary(),
    onSuccess: () => qc.setQueryData(currentLibraryKey, null),
  });
}
