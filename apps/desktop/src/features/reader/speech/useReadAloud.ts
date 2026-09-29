import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { useProfilePrefs } from "@/features/profiles";
import type { Renderer, SpeechPiece, SpeechSource } from "@/readers";
import { getEngine, type Engine } from "./engine";

export type ReadAloudStatus = "off" | "starting" | "playing" | "paused" | "noVoices";

/**
 * Reads the open book aloud, a sentence at a time, marking the sentence
 * and turning pages to follow it.
 */
export function useReadAloud(renderer: React.RefObject<Renderer | null>) {
  const [status, setStatus] = useState<ReadAloudStatus>("off");
  const [sentence, setSentence] = useState<string>("");
  /** Which sentence is being read, counting from 1 where reading began. */
  const [number, setNumber] = useState(0);
  const [engine, setEngine] = useState<Engine | null>(null);
  const source = useRef<SpeechSource | null>(null);
  const pieces = useRef<SpeechPiece[]>([]);
  const index = useRef(-1);
  const run = useRef(0);
  const eng = useRef<Engine | null>(null);

  const loop = useCallback(async (id: number) => {
    const e = eng.current;
    const src = source.current;
    if (!e || !src) return;
    setStatus("playing");
    while (id === run.current) {
      index.current++;
      if (index.current >= pieces.current.length) {
        const p = await src.next();
        if (id !== run.current) return;
        if (!p) {
          src.clear();
          setStatus("off");
          setSentence("");
          toast("Read to the end of the book");
          return;
        }
        pieces.current.push(p);
      }
      const p = pieces.current[index.current]!;
      const { listening } = useProfilePrefs.getState().prefs;
      src.show(p, listening.follow);
      setSentence(p.text);
      setNumber(index.current + 1);
      const finished = await e.speak(p.text, {
        voice: listening.voice,
        rate: listening.speechRate,
        lang: p.lang,
      });
      if (!finished || id !== run.current) return;
    }
  }, []);

  const start = useCallback(async () => {
    const r = renderer.current;
    if (!r?.readAloud) {
      toast("This book cannot be read aloud");
      return;
    }
    setStatus("starting");
    const e = await getEngine();
    if (!e) {
      setStatus("noVoices");
      return;
    }
    eng.current = e;
    setEngine(e);
    const src = await r.readAloud().catch(() => null);
    if (!src) {
      setStatus("off");
      toast("There is no text to read here", {
        description: "Scanned pages can be read aloud after Make searchable (OCR).",
      });
      return;
    }
    source.current?.clear();
    source.current = src;
    pieces.current = [];
    index.current = -1;
    void loop(++run.current);
  }, [renderer, loop]);

  const pause = useCallback(() => {
    run.current++;
    eng.current?.stop();
    setStatus("paused");
  }, []);

  const resume = useCallback(() => {
    index.current = Math.max(-1, index.current - 1);
    void loop(++run.current);
  }, [loop]);

  const skip = useCallback(
    (dir: 1 | -1) => {
      if (!source.current) return;
      run.current++;
      eng.current?.stop();
      index.current = dir > 0 ? index.current : Math.max(-1, index.current - 2);
      void loop(++run.current);
    },
    [loop],
  );

  const stop = useCallback(() => {
    run.current++;
    eng.current?.stop();
    source.current?.clear();
    source.current = null;
    setStatus("off");
    setSentence("");
  }, []);

  /** Speed or voice changed: say the sentence again with it. */
  const restart = useCallback(() => {
    if (status !== "playing") return;
    run.current++;
    eng.current?.stop();
    index.current = Math.max(-1, index.current - 1);
    void loop(++run.current);
  }, [status, loop]);

  useEffect(
    () => () => {
      run.current++;
      eng.current?.stop();
      source.current?.clear();
    },
    [],
  );

  return { status, sentence, number, engine, start, pause, resume, skip, stop, restart };
}

export type ReadAloud = ReturnType<typeof useReadAloud>;
