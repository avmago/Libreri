import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap, type SpeechSettingsDto } from "@/lib/ipc";

export const speechKey = ["speech"] as const;

/** Models, the one in use, the language spoken. */
export function useSpeechSettings() {
  return useQuery({
    queryKey: speechKey,
    queryFn: () => commands.speechSettings(),
  });
}

export function useSetSpeechSettings() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (s: Pick<SpeechSettingsDto, "model" | "language" | "transcribeNotes">) =>
      unwrap(commands.setSpeechSettings(s.model, s.language, s.transcribeNotes)),
    onSuccess: (data) => qc.setQueryData(speechKey, data),
  });
}

export function useDownloadSpeechModel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.downloadSpeechModel(id)),
    onSettled: () => void qc.invalidateQueries({ queryKey: speechKey }),
  });
}

export function useRemoveSpeechModel() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrap(commands.removeSpeechModel(id)),
    onSuccess: (data) => qc.setQueryData(speechKey, data),
  });
}

/** Base64 of 16-bit PCM, for sending recordings to Rust. */
export function pcmBase64(pcm: Int16Array): string {
  const bytes = new Uint8Array(pcm.buffer, pcm.byteOffset, pcm.byteLength);
  let s = "";
  for (let i = 0; i < bytes.length; i += 0x8000)
    s += String.fromCharCode(...bytes.subarray(i, i + 0x8000));
  return btoa(s);
}

export function transcribe(pcm: Int16Array, lang?: string | null): Promise<string> {
  return unwrap(commands.transcribePcm(pcmBase64(pcm), lang ?? null));
}

export function saveVoiceNote(pcm: Int16Array) {
  return unwrap(commands.saveVoiceNote(pcmBase64(pcm)));
}

export function transcribeVoiceNote(path: string, lang?: string | null): Promise<string> {
  return unwrap(commands.transcribeVoiceNote(path, lang ?? null));
}

export function cancelSpeechModelDownload(id: string) {
  return commands.cancelSpeechModelDownload(id);
}
