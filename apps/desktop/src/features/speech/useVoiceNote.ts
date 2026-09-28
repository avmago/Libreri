import { useState } from "react";
import { toast } from "sonner";
import { saveVoiceNote, transcribeVoiceNote, useSpeechSettings } from "./api";
import { useRecorder } from "./useRecorder";
import type { RecordedVoice } from "./voice";

/** Longest voice note, in seconds. */
const LONGEST = 15 * 60;

/**
 * Recording a voice note: start, then finish (saved as FLAC in the
 * profile's notes folder and, if Settings say so and a model is there,
 * written down) or cancel.
 */
export function useVoiceNote(lang?: string | null) {
  const { data: settings } = useSpeechSettings();
  const [phase, setPhase] = useState<"idle" | "recording" | "saving" | "transcribing">("idle");
  const [full, setFull] = useState(false);
  const rec = useRecorder({ maxSeconds: LONGEST, onFull: () => setFull(true) });

  const start = async () => {
    setFull(false);
    const error = await rec.start();
    if (error) {
      toast.error(error);
      return false;
    }
    setPhase("recording");
    return true;
  };

  const finish = async (): Promise<RecordedVoice | null> => {
    const pcm = rec.stop();
    if (!pcm || pcm.length < 8000) {
      setPhase("idle");
      toast("The recording was too short to keep");
      return null;
    }
    setPhase("saving");
    try {
      const saved = await saveVoiceNote(pcm);
      let transcript: string | null = null;
      if (settings?.transcribeNotes && settings.model) {
        setPhase("transcribing");
        transcript = await transcribeVoiceNote(saved.path, lang).catch((e: unknown) => {
          toast.error("The voice note was saved but could not be written down", {
            description: String(e),
          });
          return null;
        });
      }
      return { path: saved.path, duration: saved.duration ?? 0, transcript: transcript || null };
    } catch (e) {
      toast.error("Could not save the voice note", { description: String(e) });
      return null;
    } finally {
      setPhase("idle");
    }
  };

  const cancel = () => {
    rec.cancel();
    setPhase("idle");
  };

  return { phase, full, level: rec.level, seconds: rec.seconds, start, finish, cancel };
}

export type VoiceNoteRecording = ReturnType<typeof useVoiceNote>;
