import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  commands,
  unwrap,
  type BookMetadata,
  type BookQuery,
  type BookUserState,
  type ImportModeDto,
  type LibrarySummary,
} from "@/lib/ipc";
import { toView } from "./model";

export const currentLibraryKey = ["library", "current"] as const;
const settingsKey = ["settings"] as const;
/** Everything read from the open library lives under this key. */
export const libKey = ["lib"] as const;

export function useCurrentLibrary() {
  return useQuery({ queryKey: currentLibraryKey, queryFn: () => commands.currentLibrary() });
}

function useLibraryMutation<A>(fn: (args: A) => Promise<LibrarySummary>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: (library) => {
      qc.removeQueries({ queryKey: libKey });
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
    onSuccess: () => {
      qc.removeQueries({ queryKey: libKey });
      qc.setQueryData(currentLibraryKey, null);
    },
  });
}

// ---- Reading ---------------------------------------------------------------

export function useBooks(query: BookQuery) {
  return useQuery({
    queryKey: [...libKey, "books", query],
    queryFn: async () => (await unwrap(commands.listBooks(query))).map(toView),
    placeholderData: (previous) => previous,
  });
}

export function useBook(id: string | null) {
  return useQuery({
    queryKey: [...libKey, "book", id],
    queryFn: async () => toView(await unwrap(commands.getBook(id!))),
    enabled: id !== null,
  });
}

export function useFacets() {
  return useQuery({
    queryKey: [...libKey, "facets"],
    queryFn: () => unwrap(commands.libraryFacets()),
  });
}

export function useFolders() {
  return useQuery({
    queryKey: [...libKey, "folders"],
    queryFn: () => unwrap(commands.listFolders()),
  });
}

// ---- Changing --------------------------------------------------------------

/** A mutation that refreshes every library list when it succeeds. */
function useLibMutation<A, R>(fn: (args: A) => Promise<R>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSettled: () => qc.invalidateQueries({ queryKey: libKey }),
  });
}

export const useUpdateBook = () =>
  useLibMutation(({ id, metadata }: { id: string; metadata: BookMetadata }) =>
    unwrap(commands.updateBook(id, metadata)),
  );

export const useSetBookState = () =>
  useLibMutation(({ id, user }: { id: string; user: BookUserState }) =>
    unwrap(commands.setBookState(id, user)),
  );

export const useMoveBooks = () =>
  useLibMutation(({ ids, folder }: { ids: string[]; folder: string }) =>
    unwrap(commands.moveBooks(ids, folder)),
  );

export const useTrashBooks = () =>
  useLibMutation((ids: string[]) => unwrap(commands.trashBooks(ids)));

export const useCreateFolder = () =>
  useLibMutation(({ parent, name }: { parent: string; name: string }) =>
    unwrap(commands.createFolder(parent, name)),
  );

export const useRenameFolder = () =>
  useLibMutation(({ path, name }: { path: string; name: string }) =>
    unwrap(commands.renameFolder(path, name)),
  );

export const useMoveFolder = () =>
  useLibMutation(({ path, parent }: { path: string; parent: string }) =>
    unwrap(commands.moveFolder(path, parent)),
  );

export const useTrashFolder = () =>
  useLibMutation((path: string) => unwrap(commands.trashFolder(path)));

export const useImportPaths = () =>
  useMutation({
    mutationFn: ({
      paths,
      folder,
      mode,
    }: {
      paths: string[];
      folder: string;
      mode: ImportModeDto;
    }) => unwrap(commands.importPaths(paths, folder, mode)),
  });
