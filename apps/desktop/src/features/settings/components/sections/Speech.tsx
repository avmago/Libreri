import { useEffect, useState } from "react";
import { Download, Loader2, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import {
  SYSTEM_DICTATION,
  cancelSpeechModelDownload,
  useDownloadSpeechModel,
  useRemoveSpeechModel,
  useSetSpeechSettings,
  useSpeechSettings,
} from "@/features/speech";
import { events, type ModelInfo } from "@/lib/ipc";
import { Group, Row, Switch } from "../parts";

/** Languages Whisper knows well; any other is found by "Detect". */
const LANGUAGES: [string, string][] = [
  ["en", "English"],
  ["de", "German"],
  ["fr", "French"],
  ["es", "Spanish"],
  ["it", "Italian"],
  ["pt", "Portuguese"],
  ["nl", "Dutch"],
  ["sv", "Swedish"],
  ["da", "Danish"],
  ["no", "Norwegian"],
  ["fi", "Finnish"],
  ["pl", "Polish"],
  ["cs", "Czech"],
  ["ru", "Russian"],
  ["uk", "Ukrainian"],
  ["el", "Greek"],
  ["tr", "Turkish"],
  ["ar", "Arabic"],
  ["he", "Hebrew"],
  ["hi", "Hindi"],
  ["ur", "Urdu"],
  ["bn", "Bengali"],
  ["zh", "Chinese"],
  ["ja", "Japanese"],
  ["ko", "Korean"],
  ["vi", "Vietnamese"],
  ["id", "Indonesian"],
];

function size(mb: number) {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`;
}

const selectClass =
  "h-8 rounded-md border border-input bg-background px-2 text-[13px] outline-none focus-visible:border-ring";

/** Settings › Speech: models for writing down speech, and how they are used. */
export function SpeechSettings() {
  const { data } = useSpeechSettings();
  const set = useSetSpeechSettings();
  const download = useDownloadSpeechModel();
  const remove = useRemoveSpeechModel();
  const [progress, setProgress] = useState<Record<string, number>>({});
  const [downloading, setDownloading] = useState<string[]>([]);
  const [more, setMore] = useState(false);

  useEffect(() => {
    const off = events.speechModelDownload.listen(({ payload: p }) => {
      if (p.total) setProgress((s) => ({ ...s, [p.id]: (p.done ?? 0) / p.total! }));
    });
    return () => void off.then((f) => f());
  }, []);

  if (!data) return null;
  const change = (c: Partial<typeof data>) =>
    set.mutate(
      {
        model: data.model,
        language: data.language,
        transcribeNotes: data.transcribeNotes,
        ...c,
      },
      { onError: (e) => toast.error(e.message) },
    );

  const get = (m: ModelInfo) => {
    setDownloading((d) => [...d, m.id]);
    download.mutate(m.id, {
      onSuccess: () => toast.success(`The ${m.name} model is ready`),
      onError: (e) => {
        if (e.message.toLowerCase() !== "cancelled")
          toast.error(`Could not download the ${m.name} model`, { description: e.message });
      },
      onSettled: () => {
        setDownloading((d) => d.filter((x) => x !== m.id));
        setProgress((s) => ({ ...s, [m.id]: 0 }));
      },
    });
  };

  const row = (m: ModelInfo) => (
    <Row
      key={m.id}
      label={`${m.name} · ${size(m.sizeMb)}`}
      help={m.description}
      htmlFor={m.downloaded ? `speech-model-${m.id}` : undefined}
    >
      {m.downloaded ? (
        <>
          <label className="flex items-center gap-1.5 text-[12.5px]">
            <input
              id={`speech-model-${m.id}`}
              type="radio"
              name="speech-model"
              checked={data.model === m.id}
              onChange={() => change({ model: m.id })}
            />
            Use this model
          </label>
          <Button
            variant="ghost"
            size="icon"
            aria-label={`Remove the ${m.name} model`}
            onClick={() =>
              remove.mutate(m.id, {
                onSuccess: () => toast(`The ${m.name} model was removed`),
                onError: (e) => toast.error(e.message),
              })
            }
          >
            <Trash2 />
          </Button>
        </>
      ) : downloading.includes(m.id) ? (
        <>
          <span className="flex items-center gap-1.5 text-[12.5px] text-muted-foreground tabular-nums">
            <Loader2 className="size-3.5 animate-spin" aria-hidden />
            {Math.round((progress[m.id] ?? 0) * 100)}%
          </span>
          <Button
            variant="ghost"
            size="icon"
            aria-label={`Stop downloading the ${m.name} model`}
            onClick={() => void cancelSpeechModelDownload(m.id)}
          >
            <X />
          </Button>
        </>
      ) : (
        <Button variant="outline" size="sm" onClick={() => get(m)}>
          <Download /> Download
        </Button>
      )}
    </Row>
  );

  const main = data.models.filter((m) => m.recommended);
  const others = data.models.filter((m) => !m.recommended);
  const anyDownloaded = data.models.some((m) => m.downloaded);

  return (
    <>
      <Group
        title="Speech models"
        scope="computer"
        description="Libreri writes down speech on this computer with Whisper; nothing you say leaves it. A model is needed for dictating with Libreri's microphone button, writing down voice notes and finding places in audiobooks. Download one, then choose which to use."
      >
        {main.map(row)}
        <div className="flex flex-col px-4 py-3">
          <button
            type="button"
            onClick={() => setMore((v) => !v)}
            aria-expanded={more || others.some((m) => m.downloaded)}
            className="self-start text-[12.5px] font-medium text-muted-foreground hover:text-foreground"
          >
            {more ? "Hide larger models" : "Larger, more accurate models…"}
          </button>
        </div>
        {(more || others.some((m) => m.downloaded)) && others.map(row)}
      </Group>

      <Group title="Writing down speech" scope="computer">
        <Row
          label="Language spoken"
          help="Choosing it makes short recordings more accurate. When a book says its language, that is used for its notes and audiobook."
          htmlFor="speech-language"
        >
          <select
            id="speech-language"
            className={selectClass}
            value={data.language ?? ""}
            onChange={(e) => change({ language: e.target.value || null })}
          >
            <option value="">Detect automatically</option>
            {LANGUAGES.map(([code, name]) => (
              <option key={code} value={code}>
                {name}
              </option>
            ))}
          </select>
        </Row>
        <Row
          label="Write down voice notes"
          help={
            anyDownloaded
              ? "The text of a voice note is kept with it, so it can be searched and read."
              : "Needs a speech model."
          }
        >
          <Switch
            label="Write down voice notes"
            checked={data.transcribeNotes}
            onChange={(v) => change({ transcribeNotes: v })}
          />
        </Row>
        <Row
          label="Dictation"
          help={
            SYSTEM_DICTATION
              ? `Your system's own dictation works in every text box in Libreri, for example ${SYSTEM_DICTATION}. The microphone button next to notes and comments uses Libreri's model instead.`
              : "The microphone button next to notes and comments writes down what you say with Libreri's model."
          }
        >
          <span />
        </Row>
      </Group>
    </>
  );
}
