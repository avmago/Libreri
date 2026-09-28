import { useCallback, useEffect, useRef, useState } from "react";
import { PauseDetector, concat, rms, toPcm16 } from "./pcm";

export type RecorderState = "idle" | "starting" | "recording";

interface Session {
  stream: MediaStream;
  ctx: AudioContext;
  node: ScriptProcessorNode;
  parts: Float32Array[];
  detector: PauseDetector;
  started: number;
}

interface Options {
  /** Dictation: called with each part at a pause in speech. */
  onPart?: (pcm: Int16Array) => void;
  /** Called once when `maxSeconds` is reached (the caller stops). */
  onFull?: () => void;
  maxSeconds?: number;
}

/**
 * Records from the microphone as 16 kHz 16-bit mono (what speech
 * recognition wants and what voice notes are saved as). With `onPart`,
 * the recording is handed over at each pause (dictation); otherwise
 * `stop()` returns all of it (voice notes).
 */
export function useRecorder(options: Options = {}) {
  const [state, setState] = useState<RecorderState>("idle");
  const [level, setLevel] = useState(0);
  const [seconds, setSeconds] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const session = useRef<Session | null>(null);
  const opts = useRef(options);
  useEffect(() => {
    opts.current = options;
  });

  const cleanup = useCallback(() => {
    const s = session.current;
    session.current = null;
    if (!s) return;
    s.node.onaudioprocess = null;
    s.node.disconnect();
    s.stream.getTracks().forEach((t) => t.stop());
    void s.ctx.close().catch(() => {});
    setState("idle");
    setLevel(0);
  }, []);

  useEffect(() => cleanup, [cleanup]);

  /** Starts recording. Resolves to null, or what went wrong. */
  const start = useCallback(async (): Promise<string | null> => {
    if (session.current) return null;
    setError(null);
    setSeconds(0);
    setState("starting");
    try {
      if (!navigator.mediaDevices?.getUserMedia) throw new Error("unsupported");
      const stream = await navigator.mediaDevices.getUserMedia({
        audio: { channelCount: 1, echoCancellation: true, noiseSuppression: true },
      });
      const ctx = new AudioContext();
      const source = ctx.createMediaStreamSource(stream);
      // A script processor works in every web view Libreri runs in (an
      // audio worklet would need a separate script file).
      const node = ctx.createScriptProcessor(4096, 1, 1);
      const s: Session = {
        stream,
        ctx,
        node,
        parts: [],
        detector: new PauseDetector(),
        started: performance.now(),
      };
      let full = false;
      node.onaudioprocess = (e) => {
        if (session.current !== s || full) return;
        const buf = new Float32Array(e.inputBuffer.getChannelData(0));
        s.parts.push(buf);
        const l = rms(buf);
        setLevel(Math.min(1, l * 6));
        const secs = (performance.now() - s.started) / 1000;
        setSeconds(secs);
        const { onPart, onFull, maxSeconds } = opts.current;
        if (onPart) {
          const said = s.detector.push(l, buf.length / ctx.sampleRate);
          if (said === "send") onPart(toPcm16(concat(s.parts), ctx.sampleRate));
          if (said) s.parts = [];
        }
        if (maxSeconds && secs >= maxSeconds) {
          full = true;
          onFull?.();
        }
      };
      source.connect(node);
      node.connect(ctx.destination);
      session.current = s;
      setState("recording");
      return null;
    } catch (e) {
      const name = (e as { name?: string }).name;
      const message =
        name === "NotAllowedError"
          ? "Libreri may not use the microphone. Allow it in your system's privacy settings."
          : name === "NotFoundError"
            ? "No microphone was found."
            : "The microphone could not be started.";
      setError(message);
      setState("idle");
      return message;
    }
  }, []);

  /** Stops. Returns the recording (voice notes), or what was said since
   * the last part (dictation; null if nothing was). */
  const stop = useCallback((): Int16Array | null => {
    const s = session.current;
    if (!s) return null;
    const dictating = !!opts.current.onPart;
    const pcm = dictating && !s.detector.heard ? null : toPcm16(concat(s.parts), s.ctx.sampleRate);
    cleanup();
    return pcm;
  }, [cleanup]);

  return { state, level, seconds, error, start, stop, cancel: cleanup };
}
