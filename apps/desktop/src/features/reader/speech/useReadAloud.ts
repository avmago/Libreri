import { useCallback, useEffect, useRef, useState } from "react";
import { toast } from "sonner";
import { useProfilePrefs } from "@/features/profiles";
import type { Renderer, SpeechPiece, SpeechSource } from "@/readers";
import { chooseVoice } from "./choose";
import { getEngine, isNatural, primeSpeech, type Engine } from "./engine";
import { commands } from "@/lib/ipc";

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
    // Sentences that "finished" at once, one after another: the voice is
    // not speaking (rather than racing to the end of the book in silence).
    let quick = 0;
    while (id === run.current) {
      index.current++;
      if (index.current >= pieces.current.length) {
        let p: SpeechPiece | null;
        try {
          p = await src.next();
        } catch (err) {
          if (id !== run.current) return;
          src.clear();
          setStatus("off");
          setSentence("");
          toast.error("The text could not be read from this book", {
            description: err instanceof Error ? err.message : String(err),
          });
          return;
        }
        if (id !== run.current) return;
        if (!p) {
          src.clear();
          setStatus("off");
          setSentence("");
          if (pieces.current.length) toast("Read to the end of the book");
          else
            toast("There is no text to read from here", {
              description: "Scanned pages can be read aloud after Make searchable (OCR).",
            });
          return;
        }
        pieces.current.push(p);
      }
      const p = pieces.current[index.current]!;
      const { listening } = useProfilePrefs.getState().prefs;
      src.show(p, listening.follow);
      setSentence(p.text);
      setNumber(index.current + 1);
      const voice = chooseVoice(e.voices, listening, p.lang);
      const opts = { voice, rate: listening.speechRate, lang: p.lang };
      // A natural voice makes the next sentence while this one is said.
      const next = pieces.current[index.current + 1] ?? src.peek?.() ?? null;
      if (next && e.prepare) {
        e.prepare(next.text, { ...opts, voice: chooseVoice(e.voices, listening, next.lang) });
      }
      let finished: boolean;
      const began = Date.now();
      try {
        finished = await e.speak(p.text, opts);
      } catch (err) {
        if (id !== run.current) return;
        setStatus("paused");
        toast.error("This voice could not read aloud", {
          description: err instanceof Error ? err.message : String(err),
        });
        return;
      }
      if (!finished || id !== run.current) return;
      quick = Date.now() - began < 60 && p.text.length > 3 ? quick + 1 : 0;
      if (quick >= 4) {
        setStatus("paused");
        toast.error("The voice is not speaking", {
          description: "Try another voice in the player, or check the computer's sound.",
        });
        return;
      }
    }
  }, []);

  const start = useCallback(async () => {
    const r = renderer.current;
    if (!r?.readAloud) {
      toast("This book cannot be read aloud");
      return;
    }
    // Before anything is awaited: WebKit allows sound right after a click.
    primeSpeech();
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
    primeSpeech();
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
    // A natural voice gives its memory back.
    if (eng.current?.voices.some((v) => isNatural(v.id))) void commands.unloadVoices();
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
