import { useEffect, useMemo, useState } from "react";
import { Check, ExternalLink, Loader2, Pause, Play, RotateCw, Trash2, X } from "lucide-react";
import { toast } from "sonner";
import { RowsSkeleton } from "@/components/Placeholders";
import { Button } from "@/components/ui/button";
import { NativeSelect } from "@/components/ui/input";
import { useHelperDialog } from "@/features/helpers";
import { useProfilePrefs } from "@/features/profiles";
import {
  baseLang,
  cancelVoiceDownload,
  piperSample,
  useDownloadVoice,
  useNaturalVoices,
  usePiperLanguages,
  usePiperVoices,
  useRemoveVoice,
  useSetVoiceOn,
} from "@/features/reader";
import { commands, events, type NaturalVoicesDto, type PiperVoiceInfo } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { Group, Row, Switch } from "../parts";

const KOKORO_SAMPLE = "/samples/kokoro-heart.mp3";

function size(mb: number): string {
  return mb >= 1000 ? `${(mb / 1000).toFixed(1)} GB` : `${mb} MB`;
}

type Progress = { id: string; done: number; total: number } | null;

/** Download progress of the voice being downloaded. */
function useVoiceProgress(): Progress {
  const [progress, setProgress] = useState<Progress>(null);
  useEffect(() => {
    const off = events.voiceDownload.listen(({ payload: p }) => {
      if (p.finished) setProgress(null);
      else setProgress({ id: p.id, done: p.done ?? 0, total: p.total ?? 0 });
    });
    return () => void off.then((f) => f());
  }, []);
  return progress;
}

/** One sample plays at a time. */
let sampleAudio: HTMLAudioElement | null = null;
function useSample() {
  const [playing, setPlaying] = useState<string | null>(null);
  useEffect(
    () => () => {
      sampleAudio?.pause();
      sampleAudio = null;
    },
    [],
  );
  const toggle = (url: string) => {
    if (sampleAudio) {
      sampleAudio.pause();
      sampleAudio = null;
    }
    if (playing === url) {
      setPlaying(null);
      return;
    }
    const a = new Audio(url);
    sampleAudio = a;
    a.onended = () => setPlaying(null);
    a.onerror = () => {
      setPlaying(null);
      toast.error("The sample could not be played", {
        description: "Check the internet connection.",
      });
    };
    void a.play().catch(() => setPlaying(null));
    setPlaying(url);
  };
  return { playing, toggle };
}

/**
 * Settings › Reader: natural voices for reading aloud (ADR 0030). Kokoro
 * and Piper voices are downloaded here, switched on or off and deleted;
 * the system's voices are always there.
 */
export function ReadAloudVoices() {
  const { data } = useNaturalVoices();
  const progress = useVoiceProgress();
  const sample = useSample();
  if (!data) return null;
  return (
    <>
      <KokoroGroup data={data} progress={progress} sample={sample} />
      <PiperGroup data={data} progress={progress} sample={sample} />
      <VoicePerLanguage data={data} />
    </>
  );
}

type Sample = ReturnType<typeof useSample>;

function useActions() {
  const download = useDownloadVoice();
  const remove = useRemoveVoice();
  const setOn = useSetVoiceOn();
  return {
    download: (id: string, name: string) =>
      download.mutate(id, {
        onSuccess: () =>
          toast.success(`${name} is ready`, {
            description: "Choose it in the read-aloud player, or below for each language.",
          }),
        onError: (e) =>
          e.message !== "cancelled" &&
          toast.error(`${name} could not be downloaded`, { description: e.message }),
      }),
    remove: (id: string, name: string) =>
      remove.mutate(id, {
        onSuccess: () => toast(`${name} was deleted`),
        onError: (e) => toast.error("Could not delete it", { description: e.message }),
      }),
    removing: (id: string) => remove.isPending && remove.variables === id,
    setOn: (id: string, on: boolean) =>
      setOn.mutate(
        { id, on },
        { onError: (e) => toast.error("Could not change it", { description: e.message }) },
      ),
  };
}

function KokoroGroup({
  data,
  progress,
  sample,
}: {
  data: NaturalVoicesDto;
  progress: Progress;
  sample: Sample;
}) {
  const openHelper = useHelperDialog((s) => s.open);
  const act = useActions();
  const k = data.kokoro;
  const on = k.downloaded && !data.off.includes("kokoro");
  return (
    <Group
      title="Read aloud voices"
      scope="computer"
      description="Natural voices that sound like a person, made on this computer while you listen. Nothing is sent anywhere, and they work offline on macOS, Windows and Linux. Only voices free to use are offered. Download a voice, then switch it on to use it; delete it to get the space back. The system's own voices are always there."
    >
      <VoiceRow
        name="System voices"
        status={null}
        description="The voices that come with this computer (eSpeak NG on Linux when there are none). Always on."
        toggle={
          <Switch label="System voices" checked disabled title="Always on" onChange={() => {}} />
        }
      />
      <VoiceRow
        name="Kokoro"
        status={k.downloaded ? (on ? "Downloaded · On" : "Downloaded · Off") : null}
        description={`The most natural. ${k.voices} voices in English (US and UK), Spanish, French, Hindi, Italian, Portuguese, Japanese and Chinese.`}
        meta={
          <>
            <span>
              {size(k.sizeMb)} · {k.licence}
            </span>
            <button
              type="button"
              className="inline-flex items-center gap-1 underline-offset-2 hover:underline"
              onClick={() => void commands.openExternalUrl(k.homepage)}
            >
              About the model <ExternalLink className="size-3" />
            </button>
          </>
        }
        progress={
          data.downloading === "kokoro" ? (progress?.id === "kokoro" ? progress : null) : undefined
        }
        actions={
          <>
            <SampleButton url={KOKORO_SAMPLE} name="Kokoro" sample={sample} wide />
            <DownloadButton
              id="kokoro"
              name="Kokoro"
              sizeMb={k.sizeMb}
              downloaded={k.downloaded}
              data={data}
              act={act}
            />
          </>
        }
        toggle={
          <Switch
            label="Use Kokoro voices"
            checked={on}
            disabled={!k.downloaded}
            onChange={(v) => act.setOn("kokoro", v)}
          />
        }
      />
      {!data.espeak && (
        <div className="flex items-center gap-3 bg-muted/40 px-4 py-3 text-[12.5px]">
          <p className="flex-1 text-muted-foreground">
            Natural voices need eSpeak NG, a small helper program, to turn words into sounds.
          </p>
          <Button size="sm" variant="outline" onClick={() => openHelper("espeak")}>
            Install eSpeak NG…
          </Button>
        </div>
      )}
    </Group>
  );
}

function PiperGroup({
  data,
  progress,
  sample,
}: {
  data: NaturalVoicesDto;
  progress: Progress;
  sample: Sample;
}) {
  const [show, setShow] = useState(false);
  const languages = usePiperLanguages(show);
  const [picked, setPicked] = useState<string | null>(null);
  const [quality, setQuality] = useState("all");
  // This computer's language until one is picked.
  const code = useMemo(() => {
    if (picked) return picked;
    const list = languages.data ?? [];
    const want = navigator.language.replace("-", "_");
    return (
      list.find((l) => l.code.toLowerCase() === want.toLowerCase())?.code ??
      list.find((l) => baseLang(l.code) === baseLang(want))?.code ??
      list.find((l) => l.code === "en_US")?.code ??
      list[0]?.code ??
      null
    );
  }, [picked, languages.data]);
  const voices = usePiperVoices(show ? code : null);
  const act = useActions();
  const downloaded = data.voices.filter((v) => v.pack.startsWith("piper:"));
  const list = (voices.data?.voices ?? []).filter(
    (v) => quality === "all" || v.quality === quality,
  );

  return (
    <Group
      title="Piper voices"
      scope="computer"
      description="Lighter voices in 40+ more languages, one voice at a time. Each shows its licence; voices that are not free to use are left out."
    >
      {!show ? (
        <div className="flex items-center gap-3 px-4 py-3">
          <p className="flex-1 text-[12.5px] text-muted-foreground">
            {downloaded.length
              ? `${downloaded.length} downloaded: ${[...new Set(downloaded.map((v) => v.name))].join(", ")}.`
              : "The list of voices comes from the Piper project and needs the internet."}
          </p>
          <Button size="sm" variant="outline" onClick={() => setShow(true)}>
            Show voices
          </Button>
        </div>
      ) : languages.isError ? (
        <ErrorRow message={languages.error.message} onRetry={() => void languages.refetch()} />
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2 bg-muted/40 px-4 py-2.5">
            <NativeSelect
              aria-label="Language"
              value={code ?? ""}
              onChange={(e) => setPicked(e.target.value)}
              className="w-60"
              disabled={!languages.data}
            >
              {(languages.data ?? []).map((l) => (
                <option key={l.code} value={l.code}>
                  {l.name}
                </option>
              ))}
            </NativeSelect>
            <NativeSelect
              aria-label="Quality"
              value={quality}
              onChange={(e) => setQuality(e.target.value)}
              className="w-36"
            >
              <option value="all">All qualities</option>
              <option value="high">High</option>
              <option value="medium">Medium</option>
              <option value="low">Low</option>
              <option value="x_low">Very low</option>
            </NativeSelect>
            <span className="ml-auto text-[12px] text-muted-foreground">
              {voices.data
                ? `${list.length} ${list.length === 1 ? "voice" : "voices"}${voices.data.leftOut ? ` · ${voices.data.leftOut} left out (licence)` : ""}`
                : ""}
            </span>
          </div>
          {!voices.data && !voices.isError ? (
            <RowsSkeleton rows={3} label="Loading voices" />
          ) : voices.isError ? (
            <ErrorRow message={voices.error.message} onRetry={() => void voices.refetch()} />
          ) : list.length === 0 ? (
            <p className="px-4 py-3 text-[12.5px] text-muted-foreground">
              No voice of this language is free to use yet.
            </p>
          ) : (
            list.map((v) => (
              <PiperRow
                key={v.id}
                v={v}
                data={data}
                progress={progress}
                sample={sample}
                act={act}
              />
            ))
          )}
        </>
      )}
    </Group>
  );
}

function PiperRow({
  v,
  data,
  progress,
  sample,
  act,
}: {
  v: PiperVoiceInfo;
  data: NaturalVoicesDto;
  progress: Progress;
  sample: Sample;
  act: ReturnType<typeof useActions>;
}) {
  const on = v.downloaded && !data.off.includes(v.id);
  const url = piperSample(v.id);
  return (
    <VoiceRow
      name={v.name}
      detail={`${v.language} · ${v.quality === "x_low" ? "very low" : v.quality}`}
      status={null}
      meta={
        <span>
          {size(v.sizeMb)} · {v.licence}
        </span>
      }
      progress={data.downloading === v.id ? (progress?.id === v.id ? progress : null) : undefined}
      actions={
        <>
          {url && <SampleButton url={url} name={v.name} sample={sample} />}
          <DownloadButton
            id={v.id}
            name={v.name}
            sizeMb={v.sizeMb}
            downloaded={v.downloaded}
            data={data}
            act={act}
          />
        </>
      }
      toggle={
        <Switch
          label={`Use ${v.name}`}
          checked={on}
          disabled={!v.downloaded}
          onChange={(x) => act.setOn(v.id, x)}
        />
      }
    />
  );
}

/** Settings › Reader: the voice for books in each language. */
function VoicePerLanguage({ data }: { data: NaturalVoicesDto }) {
  const listening = useProfilePrefs((s) => s.prefs.listening);
  const update = useProfilePrefs((s) => s.update);
  const on = data.voices.filter((v) => v.on);
  const langs = useMemo(() => {
    const m = new Map<string, string>();
    for (const v of on) {
      const b = baseLang(v.lang);
      if (!m.has(b)) m.set(b, v.language.replace(/ \(.*\)$/, ""));
    }
    return [...m.entries()].sort((a, b) => a[1].localeCompare(b[1]));
  }, [on]);
  if (!langs.length) return null;
  return (
    <Group
      title="Voice for each language"
      scope="yours"
      description="Used when a book is in that language; only voices that are on are listed. Change it any time in the read-aloud player."
    >
      {langs.map(([b, name]) => (
        <Row key={b} label={name} htmlFor={`voice-${b}`}>
          <NativeSelect
            id={`voice-${b}`}
            value={listening.voiceFor[b] ?? ""}
            onChange={(e) => {
              const voiceFor = { ...listening.voiceFor };
              if (e.target.value) voiceFor[b] = e.target.value;
              else delete voiceFor[b];
              update({ listening: { voiceFor } });
            }}
            className="w-64"
          >
            <option value="">Automatic</option>
            {on
              .filter((v) => baseLang(v.lang) === b)
              .map((v) => (
                <option key={v.id} value={v.id}>
                  {v.pack === "kokoro" ? "Kokoro" : "Piper"} · {v.name}
                  {v.lang.includes("-") ? ` (${v.lang.split("-")[1]})` : ""}
                </option>
              ))}
          </NativeSelect>
        </Row>
      ))}
    </Group>
  );
}

function VoiceRow({
  name,
  detail,
  status,
  description,
  meta,
  progress,
  actions,
  toggle,
}: {
  name: string;
  detail?: string;
  status: string | null;
  description?: string;
  meta?: React.ReactNode;
  /** undefined: not downloading; null: starting. */
  progress?: { done: number; total: number } | null;
  actions?: React.ReactNode;
  toggle: React.ReactNode;
}) {
  const pct =
    progress && progress.total ? Math.round((progress.done / progress.total) * 100) : null;
  return (
    <div className="flex items-start gap-3 px-4 py-3">
      <div className="flex min-w-0 flex-1 flex-col gap-0.5">
        <span className="flex flex-wrap items-baseline gap-x-1.5">
          <span className="font-medium">{name}</span>
          {detail && <span className="text-[12px] text-muted-foreground">· {detail}</span>}
          {status && (
            <span className="inline-flex items-center gap-0.5 text-[12px] font-medium text-emerald-600 dark:text-emerald-400">
              <Check className="size-3.5" /> {status}
            </span>
          )}
        </span>
        {description && (
          <p className="text-[12.5px] leading-snug text-muted-foreground">{description}</p>
        )}
        {meta && (
          <p className="flex flex-wrap items-center gap-x-2 text-[12px] text-muted-foreground">
            {meta}
          </p>
        )}
        {progress !== undefined && (
          <span className="flex items-center gap-2 pt-1 text-[12px] text-muted-foreground">
            <span className="h-1.5 max-w-64 flex-1 overflow-hidden rounded-full bg-muted">
              <span
                className="block h-full bg-foreground/70 transition-[width]"
                style={{ width: `${pct ?? 0}%` }}
              />
            </span>
            <span className="tabular-nums">{pct !== null ? `${pct}%` : "Starting…"}</span>
          </span>
        )}
      </div>
      {actions && <div className="flex shrink-0 items-center gap-1.5">{actions}</div>}
      <div className="flex shrink-0 items-center self-center">{toggle}</div>
    </div>
  );
}

function SampleButton({
  url,
  name,
  sample,
  wide,
}: {
  url: string;
  name: string;
  sample: Sample;
  wide?: boolean;
}) {
  const playing = sample.playing === url;
  return (
    <Button
      variant="ghost"
      size={wide ? "sm" : "icon"}
      aria-label={playing ? `Stop the sample of ${name}` : `Hear a sample of ${name}`}
      title={playing ? "Stop" : "Hear a sample"}
      onClick={() => sample.toggle(url)}
    >
      {playing ? <Pause /> : <Play />}
      {wide && (playing ? "Stop" : "Sample")}
    </Button>
  );
}

function DownloadButton({
  id,
  name,
  sizeMb,
  downloaded,
  data,
  act,
}: {
  id: string;
  name: string;
  sizeMb: number;
  downloaded: boolean;
  data: NaturalVoicesDto;
  act: ReturnType<typeof useActions>;
}) {
  if (data.downloading === id)
    return (
      <Button variant="outline" size="sm" onClick={cancelVoiceDownload}>
        <X /> Stop
      </Button>
    );
  if (downloaded)
    return (
      <Button
        variant="outline"
        size="sm"
        aria-label={`Delete ${name}`}
        title={`Delete ${name} from this computer (${size(sizeMb)})`}
        disabled={act.removing(id)}
        onClick={() => act.remove(id, name)}
      >
        {act.removing(id) ? <Loader2 className="animate-spin" /> : <Trash2 />} Delete
      </Button>
    );
  return (
    <Button
      size="sm"
      disabled={data.downloading !== null}
      title={`Download ${name} (${size(sizeMb)})`}
      onClick={() => act.download(id, name)}
    >
      Download
    </Button>
  );
}

function ErrorRow({ message, onRetry }: { message: string; onRetry: () => void }) {
  return (
    <div className={cn("flex items-center gap-3 px-4 py-3 text-[12.5px]")}>
      <p className="flex-1 text-muted-foreground">The voices could not be listed: {message}</p>
      <Button size="sm" variant="outline" onClick={onRetry}>
        <RotateCw /> Try again
      </Button>
    </div>
  );
}
