/**
 * Natural voices (ADR 0030): Kokoro and Piper voices downloaded in
 * Settings › Reader and run on this computer.
 */
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { commands, unwrap, type NaturalVoicesDto } from "@/lib/ipc";

export const naturalVoicesKey = ["natural-voices"] as const;

export function useNaturalVoices() {
  return useQuery({ queryKey: naturalVoicesKey, queryFn: () => commands.naturalVoices() });
}

function useVoicesMutation<A>(fn: (a: A) => Promise<NaturalVoicesDto>) {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: fn,
    onSuccess: (d) => qc.setQueryData(naturalVoicesKey, d),
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: naturalVoicesKey });
      void qc.invalidateQueries({ queryKey: ["piper-voices"] });
    },
  });
}

export function useDownloadVoice() {
  return useVoicesMutation((id: string) => unwrap(commands.downloadVoice(id)));
}

export function useRemoveVoice() {
  return useVoicesMutation((id: string) => unwrap(commands.removeVoice(id)));
}

export function useSetVoiceOn() {
  return useVoicesMutation(({ id, on }: { id: string; on: boolean }) =>
    unwrap(commands.setVoiceOn(id, on)),
  );
}

export function cancelVoiceDownload() {
  void commands.cancelVoiceDownload();
}

/** The languages of the Piper collection (needs the internet once a week). */
export function usePiperLanguages(enabled: boolean) {
  return useQuery({
    queryKey: ["piper-languages"],
    queryFn: () => unwrap(commands.piperLanguages()),
    enabled,
    staleTime: 60 * 60_000,
    retry: false,
  });
}

/** Piper voices of one language that are free to use. */
export function usePiperVoices(code: string | null) {
  return useQuery({
    queryKey: ["piper-voices", code],
    queryFn: () => unwrap(commands.piperVoices(code!)),
    enabled: !!code,
    staleTime: 10 * 60_000,
    retry: false,
  });
}

/** A Piper voice's sample from the Piper project's sample pages. */
export function piperSample(id: string): string | null {
  // "piper:de_DE-thorsten-high" → samples/de/de_DE/thorsten/high/speaker_0.mp3
  const m = /^piper:(([a-z]+)_[A-Za-z]+)-(.+)-([a-z_]+)$/.exec(id);
  if (!m) return null;
  const [, code, family, name, quality] = m;
  return `https://raw.githubusercontent.com/rhasspy/piper-samples/master/samples/${family}/${code}/${name}/${quality}/speaker_0.mp3`;
}

let opener: (() => void) | null = null;

/** Lets the app open Settings › Reader at the voices. */
export function setVoicesSettingsOpener(fn: () => void) {
  opener = fn;
}

export function openVoicesSettings() {
  opener?.();
}
