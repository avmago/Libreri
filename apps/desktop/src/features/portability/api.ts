import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { open } from "@tauri-apps/plugin-dialog";
import {
  commands,
  unwrap,
  type BackupSettingsChange,
  type CitationStyle,
  type ForeignImportDto,
  type ProfileMappingDto,
} from "@/lib/ipc";
import { usePortability } from "./store";

const libKey = ["lib"] as const;
const backupKey = [...libKey, "backups"] as const;

export function useCitation(ids: string[] | null, style: CitationStyle) {
  return useQuery({
    queryKey: [...libKey, "citation", style, ids],
    queryFn: () => unwrap(commands.copyCitation(ids ?? [], style)),
    enabled: ids !== null && ids.length > 0,
  });
}

export function useArchiveSummary(path: string | null) {
  return useQuery({
    queryKey: [...libKey, "archive", path],
    queryFn: () => unwrap(commands.inspectArchive(path!)),
    enabled: path !== null,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

export function useImportArchive() {
  return useMutation({
    mutationFn: ({ path, profiles }: { path: string; profiles: ProfileMappingDto[] }) =>
      unwrap(commands.importArchive(path, profiles)),
  });
}

export function useHealthCheck(enabled: boolean) {
  return useQuery({
    queryKey: [...libKey, "health"],
    queryFn: () => unwrap(commands.healthCheck()),
    enabled,
    staleTime: 0,
    gcTime: 0,
  });
}

export function useRepairHealth() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.repairHealth()),
    onSuccess: () => void qc.invalidateQueries({ queryKey: libKey }),
  });
}

export function useBackupSettings() {
  return useQuery({
    queryKey: backupKey,
    queryFn: () => unwrap(commands.getBackupSettings()),
  });
}

export function useSetBackupSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (change: BackupSettingsChange) => unwrap(commands.setBackupSettings(change)),
    onSuccess: (data) => qc.setQueryData(backupKey, data),
  });
}

export function useBackUpNow() {
  return useMutation({ mutationFn: () => unwrap(commands.backUpNow()) });
}

/** Asks for a `.libreri` file, then opens the import dialog for it. */
export async function pickArchiveToImport() {
  const path = await open({
    title: "Import a Libreri archive",
    filters: [{ name: "Libreri archive", extensions: ["libreri"] }],
  });
  if (typeof path === "string") usePortability.getState().openImport(path);
}

export function useForeignSummary(path: string | null) {
  return useQuery({
    queryKey: [...libKey, "foreign", path],
    queryFn: () => unwrap(commands.inspectForeign(path!)),
    enabled: path !== null,
    staleTime: 0,
    gcTime: 0,
    retry: false,
  });
}

export function useImportForeign() {
  return useMutation({
    mutationFn: ({ path, options }: { path: string; options: ForeignImportDto }) =>
      unwrap(commands.importForeign(path, options)),
  });
}
