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
    // Switch at once; the saved settings follow.
    onMutate: (theme: Theme) => {
      const before = qc.getQueryData<SettingsDto>(settingsKey);
      if (before) qc.setQueryData(settingsKey, { ...before, theme });
      return { before };
    },
    onError: (_e, _t, ctx) => {
      if (ctx?.before) qc.setQueryData(settingsKey, ctx.before);
    },
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

export function useSetAccent() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (accent: string | null) => unwrap(commands.setAccent(accent)),
    onSuccess: (settings: SettingsDto) => qc.setQueryData(settingsKey, settings),
  });
}
