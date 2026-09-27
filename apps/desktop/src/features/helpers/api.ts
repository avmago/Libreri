import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap, type Helper } from "@/lib/ipc";

export const helpersKey = ["helpers"] as const;

export function useHelpers() {
  return useQuery({
    queryKey: helpersKey,
    queryFn: () => unwrap(commands.helpersStatus()),
    staleTime: 10_000,
  });
}

export function useInstallHelper() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (helper: Helper) => unwrap(commands.installHelper(helper)),
    onSuccess: (data) => qc.setQueryData(helpersKey, data),
    onSettled: () => void qc.invalidateQueries({ queryKey: helpersKey }),
  });
}
