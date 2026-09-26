import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap, type SettingsDto, type Theme } from "@/lib/ipc";

export const settingsKey = ["settings"] as const;

export function useSettings() {
  return useQuery({ queryKey: settingsKey, queryFn: () => commands.getSettings() });
}

export function useSetTheme() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (theme: Theme) => unwrap(commands.setTheme(theme)),
    onSuccess: (settings: SettingsDto) => qc.setQueryData(settingsKey, settings),
  });
}

export function useForgetRecentLibrary() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => unwrap(commands.forgetRecentLibrary(path)),
    onSuccess: (settings: SettingsDto) => qc.setQueryData(settingsKey, settings),
  });
}
