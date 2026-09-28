/**
 * Reading maths from pictures (Phase 8b): off until turned on in
 * Settings › Writing, which downloads the model to this computer.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap } from "@/lib/ipc";

export const mathsKey = ["maths-settings"] as const;

export function useMathsSettings() {
  return useQuery({ queryKey: mathsKey, queryFn: () => commands.mathsSettings() });
}

export function useSetMathsFromPictures() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (on: boolean) => unwrap(commands.setMathsFromPictures(on)),
    onSuccess: (d) => qc.setQueryData(mathsKey, d),
  });
}

export function useDownloadMathsModel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.downloadMathsModel()),
    onSettled: () => void qc.invalidateQueries({ queryKey: mathsKey }),
  });
}

export function useRemoveMathsModel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: () => unwrap(commands.removeMathsModel()),
    onSuccess: (d) => qc.setQueryData(mathsKey, d),
  });
}

export const cancelMathsDownload = () => commands.cancelMathsDownload();

/** A picture (data URL) of maths as LaTeX. */
export const mathsFromPicture = (dataUrl: string) => unwrap(commands.mathsFromPicture(dataUrl));
