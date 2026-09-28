import { useEffect, useState } from "react";
import { Download, Loader2, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  cancelCanvasFontDownload,
  useCanvasFonts,
  useDownloadCanvasFont,
  useInkSettings,
  useRemoveCanvasFont,
  useSetInkEngine,
} from "@/features/canvas";
import { useProfilePrefs } from "@/features/profiles";
import {
  cancelMathsDownload,
  useDownloadMathsModel,
  useMathsSettings,
  useRemoveMathsModel,
  useSetMathsFromPictures,
} from "@/features/reader";
import {
  cancelDictionaryDownload,
  useDictionaries,
  useDownloadDictionary,
  useOwnWords,
  useRemoveDictionary,
  useRemoveOwnWord,
} from "@/features/spell";
import { events, type DictionaryInfo } from "@/lib/ipc";
import { Group, Row, Switch } from "../parts";

const isMac = typeof navigator !== "undefined" && /Mac/.test(navigator.platform);

function size(kb: number) {
  return kb >= 1000 ? `${(kb / 1000).toFixed(1)} MB` : `${kb} KB`;
}

/** Settings › Writing: spell check, dictionaries, word completion. */
export function WritingSettings() {
  const writing = useProfilePrefs((s) => s.prefs.writing);
  const update = useProfilePrefs((s) => s.update);
  const { data: dicts = [] } = useDictionaries();
  const download = useDownloadDictionary();
  const remove = useRemoveDictionary();
  const { data: own = [] } = useOwnWords();
  const removeWord = useRemoveOwnWord();
  const [progress, setProgress] = useState<Record<string, number>>({});
  const [busy, setBusy] = useState<string[]>([]);
  const [pick, setPick] = useState("");
  const { data: ink } = useInkSettings();
  const setInk = useSetInkEngine();
  const { data: fonts = [] } = useCanvasFonts();
  const getFont = useDownloadCanvasFont();
  const removeFont = useRemoveCanvasFont();
  const [fontProgress, setFontProgress] = useState<Record<string, number>>({});
  const { data: maths } = useMathsSettings();
  const setMaths = useSetMathsFromPictures();
  const getMaths = useDownloadMathsModel();
  const removeMaths = useRemoveMathsModel();
  const [mathsProgress, setMathsProgress] = useState(0);
  useEffect(() => {
    const off = events.mathsDownload.listen(({ payload: p }) => {
      if (p.total) setMathsProgress(Math.min(1, (p.done ?? 0) / p.total));
    });
    return () => void off.then((f) => f());
  }, []);
  const turnMaths = (on: boolean) => {
    if (!on || maths?.downloaded) return setMaths.mutate(on);
    setMathsProgress(0);
    getMaths.mutate(undefined, {
      onSuccess: () =>
        setMaths.mutate(true, {
          onSuccess: () => toast.success("Maths can now be read from pictures"),
        }),
      onError: (e) => {
        if (e.message.toLowerCase() !== "cancelled")
          toast.error("Could not download the maths model", { description: e.message });
      },
    });
  };
  useEffect(() => {
    const off = events.canvasFontDownload.listen(({ payload: p }) => {
      if (p.total)
        setFontProgress((s) => ({ ...s, [p.id]: Math.min(1, (p.done ?? 0) / p.total!) }));
    });
    return () => void off.then((f) => f());
  }, []);

  useEffect(() => {
    const off = events.dictionaryDownload.listen(({ payload: p }) => {
      if (p.total) setProgress((s) => ({ ...s, [p.code]: Math.min(1, (p.done ?? 0) / p.total!) }));
    });
    return () => void off.then((f) => f());
  }, []);

  const ready = dicts.filter((d) => d.downloaded);
  const others = dicts.filter((d) => !d.downloaded);
  const toggle = (code: string, on: boolean) => {
    const next = on
      ? [...writing.languages.filter((c) => c !== code), code]
      : writing.languages.filter((c) => c !== code);
    if (!next.length) return toast("Keep at least one language for spell check");
    if (next.length > 4) return toast("Up to four languages can be checked together");
    update({ writing: { languages: next } });
  };
  const get = (d: DictionaryInfo) => {
    setBusy((b) => [...b, d.code]);
    download.mutate(d.code, {
      onSuccess: () => {
        toast.success(`${d.name} can now be checked`);
        if (writing.languages.length < 4)
          update({ writing: { languages: [...writing.languages, d.code] } });
      },
      onError: (e) => {
        if (e.message.toLowerCase() !== "cancelled")
          toast.error(`Could not download ${d.name}`, { description: e.message });
      },
      onSettled: () => {
        setBusy((b) => b.filter((x) => x !== d.code));
        setProgress((s) => ({ ...s, [d.code]: 0 }));
      },
    });
  };

  return (
    <>
      <Group
        title="Spell check"
        scope="yours"
        description={`Libreri checks what you write in notebooks, notes, comments, voice-note text and text boxes, and never marks the names and terms of the book you are writing about. For corrections, right-click a word with a wavy line or press ${isMac ? "⌘;" : "Ctrl+;"}. Dictionaries you download are kept on this computer.`}
      >
        <Row label="Check spelling" help="Uses the languages ticked below.">
          <Switch
            label="Check spelling"
            checked={writing.spellCheck}
            onChange={(v) => update({ writing: { spellCheck: v } })}
          />
        </Row>
        <Row
          label="Complete words as you type"
          help="Offers ways to finish a word, from your notes and the open book first. Tab takes the first one."
        >
          <Switch
            label="Complete words as you type"
            checked={writing.complete}
            onChange={(v) => update({ writing: { complete: v } })}
          />
        </Row>
        {ready.map((d) => (
          <Row
            key={d.code}
            label={d.name}
            help={d.builtIn ? "Comes with Libreri" : `Downloaded · ${size(d.sizeKb)}`}
          >
            <label className="flex items-center gap-1.5 text-[12.5px]">
              <input
                type="checkbox"
                checked={writing.languages.includes(d.code)}
                onChange={(e) => toggle(d.code, e.target.checked)}
              />
              Check in this language
            </label>
            {!d.builtIn && (
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Remove the ${d.name} dictionary`}
                onClick={() => {
                  if (writing.languages.includes(d.code) && writing.languages.length > 1)
                    update({
                      writing: { languages: writing.languages.filter((c) => c !== d.code) },
                    });
                  remove.mutate(d.code, { onError: (e) => toast.error(e.message) });
                }}
              >
                <Trash2 />
              </Button>
            )}
          </Row>
        ))}
        {dicts
          .filter((d) => busy.includes(d.code))
          .map((d) => (
            <Row key={d.code} label={d.name} help="Downloading…">
              <span className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground tabular-nums">
                <Loader2 className="size-3.5 animate-spin" aria-hidden />
                {Math.round((progress[d.code] ?? 0) * 100)}%
              </span>
              <Button
                variant="ghost"
                size="icon"
                aria-label={`Stop downloading ${d.name}`}
                onClick={() => void cancelDictionaryDownload(d.code)}
              >
                <X />
              </Button>
            </Row>
          ))}
        <Row
          label="Add a language"
          help="Downloaded to this computer. Remove it here any time with the bin button."
          htmlFor="lb-add-dictionary"
        >
          <select
            id="lb-add-dictionary"
            className="h-8 max-w-56 rounded-md border border-input bg-background px-2 text-[13px] outline-none focus-visible:border-ring"
            value={pick}
            onChange={(e) => setPick(e.target.value)}
          >
            <option value="">Choose a language…</option>
            {others
              .filter((d) => !busy.includes(d.code))
              .map((d) => (
                <option key={d.code} value={d.code}>
                  {d.name} ({size(d.sizeKb)})
                </option>
              ))}
          </select>
          <Button
            variant="outline"
            size="sm"
            disabled={!pick}
            onClick={() => {
              const d = others.find((x) => x.code === pick);
              if (d) get(d);
              setPick("");
            }}
          >
            <Download /> Download
          </Button>
        </Row>
      </Group>

      <Group
        title="Your dictionary"
        scope="yours"
        description="Words you added with “Add to your dictionary”. They are kept in Dictionary.txt in your notes folder, so they travel with your notes and can be edited there."
      >
        {own.length ? (
          <div className="flex flex-wrap gap-1.5 px-4 py-3">
            {own.map((w) => (
              <span
                key={w}
                className="flex items-center gap-1 rounded-full border bg-muted/50 py-0.5 pr-1 pl-2.5 text-[12.5px]"
              >
                {w}
                <button
                  type="button"
                  aria-label={`Remove ${w}`}
                  className="rounded-full p-0.5 text-muted-foreground hover:bg-background hover:text-foreground"
                  onClick={() => removeWord.mutate(w, { onError: (e) => toast.error(e.message) })}
                >
                  <X className="size-3" />
                </button>
              </span>
            ))}
          </div>
        ) : (
          <p className="px-4 py-3 text-[12.5px] text-muted-foreground">No words added yet.</p>
        )}
      </Group>
      {ink && (
        <Group
          title="Handwriting"
          scope="computer"
          description="Canvases can turn handwriting into typed text (select the ink, then Ink to text). Choose what reads it."
        >
          {ink.system && (
            <Row
              label={ink.system}
              help="Built into the system: reads joined-up handwriting well. Nothing to install."
              htmlFor="ink-system"
            >
              <input
                id="ink-system"
                type="radio"
                name="ink-engine"
                checked={ink.engine === "system"}
                onChange={() => setInk.mutate("system", { onError: (e) => toast.error(e.message) })}
              />
            </Row>
          )}
          <Row
            label="Tesseract"
            help={
              ink.tesseract
                ? "The OCR helper program, with the languages chosen in Settings › Helper programs. Best for neat printed letters."
                : "Install Tesseract in Settings › Helper programs to use it."
            }
            htmlFor="ink-tesseract"
          >
            <input
              id="ink-tesseract"
              type="radio"
              name="ink-engine"
              checked={ink.engine === "tesseract"}
              disabled={!ink.tesseract && ink.engine !== "tesseract"}
              onChange={() =>
                setInk.mutate("tesseract", { onError: (e) => toast.error(e.message) })
              }
            />
          </Row>
        </Group>
      )}
      {maths && (
        <Group
          title="Maths"
          scope="computer"
          description="Libreri copies maths as LaTeX exactly where a book has it (Markdown formulas, MathML in EPUBs). It can also read maths from pictures, such as scanned PDFs and clips, with a model that runs on this computer."
        >
          <Row
            label="Read maths from pictures"
            help={`Turning this on downloads the pix2tex model (MIT licence, about ${maths.sizeMb} MB) once. What it reads is a best attempt to check.`}
          >
            <Switch
              label="Read maths from pictures"
              checked={maths.on && maths.downloaded}
              onChange={(v) => {
                if (!getMaths.isPending && !maths.downloading) turnMaths(v);
              }}
            />
          </Row>
          {(getMaths.isPending || maths.downloading) && (
            <Row label="Downloading the maths model">
              <span className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground tabular-nums">
                <Loader2 className="size-3.5 animate-spin" aria-hidden />
                {Math.round(mathsProgress * 100)}%
              </span>
              <Button
                variant="ghost"
                size="icon"
                aria-label="Stop downloading the maths model"
                onClick={() => void cancelMathsDownload()}
              >
                <X />
              </Button>
            </Row>
          )}
          {maths.downloaded && (
            <Row label={`Maths model · ${maths.sizeMb} MB`} help="Kept on this computer.">
              <Button
                variant="ghost"
                size="icon"
                aria-label="Remove the maths model"
                onClick={() =>
                  removeMaths.mutate(undefined, {
                    onSuccess: () => toast("The maths model was removed"),
                    onError: (e) => toast.error(e.message),
                  })
                }
              >
                <Trash2 />
              </Button>
            </Row>
          )}
        </Group>
      )}
      {fonts.length > 0 && (
        <Group
          title="Canvas fonts"
          scope="computer"
          description="Fonts canvases can use for text. They are downloaded to this computer and can be removed here."
        >
          {fonts.map((f) => (
            <Row key={f.id} label={`${f.name} · ${f.sizeMb} MB`} help={f.description}>
              {f.downloaded ? (
                <Button
                  variant="ghost"
                  size="icon"
                  aria-label={`Remove ${f.name}`}
                  onClick={() =>
                    removeFont.mutate(f.id, {
                      onSuccess: () => toast(`${f.name} was removed`),
                      onError: (e) => toast.error(e.message),
                    })
                  }
                >
                  <Trash2 />
                </Button>
              ) : getFont.isPending && getFont.variables === f.id ? (
                <>
                  <span className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground tabular-nums">
                    <Loader2 className="size-3.5 animate-spin" aria-hidden />
                    {Math.round((fontProgress[f.id] ?? 0) * 100)}%
                  </span>
                  <Button
                    variant="ghost"
                    size="icon"
                    aria-label={`Stop downloading ${f.name}`}
                    onClick={() => void cancelCanvasFontDownload(f.id)}
                  >
                    <X />
                  </Button>
                </>
              ) : (
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() =>
                    getFont.mutate(f.id, {
                      onSuccess: () => toast.success(`${f.name} is ready for canvases`),
                      onError: (e) => {
                        if (e.message.toLowerCase() !== "cancelled")
                          toast.error(`Could not download ${f.name}`, { description: e.message });
                      },
                    })
                  }
                >
                  <Download /> Download
                </Button>
              )}
            </Row>
          ))}
        </Group>
      )}
    </>
  );
}
