import { useMemo } from "react";
import { Loader2, Pause, Play, SkipBack, SkipForward, X } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useHelperDialog } from "@/features/helpers";
import { useProfilePrefs } from "@/features/profiles";
import { getEngine } from "./engine";
import { SPEEDS } from "./speeds";
import type { ReadAloud } from "./useReadAloud";

const isLinux = /Linux/.test(navigator.userAgent) && !/Android/.test(navigator.userAgent);

const selectClass =
  "h-7 rounded-md border border-input bg-background px-1.5 text-[12.5px] outline-none focus-visible:border-ring";

/** The read-aloud controls under the reader's toolbar. */
export function ReadAloudBar({ r, lang }: { r: ReadAloud; lang?: string }) {
  const listening = useProfilePrefs((s) => s.prefs.listening);
  const update = useProfilePrefs((s) => s.update);
  const openHelper = useHelperDialog((s) => s.open);

  // The book's language first, then the rest.
  const voices = useMemo(() => {
    const list = [...(r.engine?.voices ?? [])];
    const want = (lang || navigator.language || "en").slice(0, 2).toLowerCase();
    return list.sort(
      (a, b) =>
        Number(!a.lang.toLowerCase().startsWith(want)) -
          Number(!b.lang.toLowerCase().startsWith(want)) || a.name.localeCompare(b.name),
    );
  }, [r.engine, lang]);

  if (r.status === "off") return null;

  if (r.status === "noVoices")
    return (
      <div className="flex min-h-10 shrink-0 items-center gap-3 border-b bg-muted/40 px-3 py-1.5 text-[13px]">
        <span className="flex-1">
          {isLinux
            ? "This system has no voices to read aloud with. Libreri can use eSpeak NG instead."
            : "No voices were found. Add a voice in your system's speech settings, then try again."}
        </span>
        {isLinux && (
          <Button
            size="sm"
            onClick={() =>
              openHelper("espeak", () => {
                void getEngine(true).then(() => void r.start());
              })
            }
          >
            Install eSpeak NG…
          </Button>
        )}
        <Button variant="ghost" size="icon" aria-label="Close" onClick={r.stop}>
          <X />
        </Button>
      </div>
    );

  const playing = r.status === "playing";
  return (
    <div
      className="flex h-10 shrink-0 items-center gap-1.5 border-b bg-muted/40 px-2 text-[12.5px]"
      role="toolbar"
      aria-label="Read aloud"
    >
      <Button variant="ghost" size="icon" aria-label="Previous sentence" onClick={() => r.skip(-1)}>
        <SkipBack />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={playing ? "Pause" : "Read"}
        onClick={() => (playing ? r.pause() : r.resume())}
        disabled={r.status === "starting"}
      >
        {r.status === "starting" ? (
          <Loader2 className="animate-spin" />
        ) : playing ? (
          <Pause />
        ) : (
          <Play />
        )}
      </Button>
      <Button variant="ghost" size="icon" aria-label="Next sentence" onClick={() => r.skip(1)}>
        <SkipForward />
      </Button>
      <span className="min-w-0 flex-1 truncate px-1 text-muted-foreground" aria-live="off">
        {r.sentence}
      </span>
      <label className="flex items-center gap-1">
        <span className="sr-only">Speed</span>
        <select
          className={selectClass}
          value={listening.speechRate}
          onChange={(e) => {
            update({ listening: { speechRate: Number(e.target.value) } });
            setTimeout(r.restart, 0);
          }}
          aria-label="Speed"
        >
          {SPEEDS.map((s) => (
            <option key={s} value={s}>
              {s}×
            </option>
          ))}
        </select>
      </label>
      {voices.length > 0 && (
        <select
          className={`${selectClass} max-w-48`}
          value={listening.voice ?? ""}
          onChange={(e) => {
            update({ listening: { voice: e.target.value || null } });
            setTimeout(r.restart, 0);
          }}
          aria-label="Voice"
        >
          <option value="">Default voice</option>
          {voices.map((v) => (
            <option key={v.id} value={v.id}>
              {v.name} ({v.lang})
            </option>
          ))}
        </select>
      )}
      <label className="flex items-center gap-1.5 px-1" title="Turn pages to follow the reading">
        <input
          type="checkbox"
          checked={listening.follow}
          onChange={(e) => update({ listening: { follow: e.target.checked } })}
        />
        Follow
      </label>
      <Button variant="ghost" size="icon" aria-label="Stop reading aloud" onClick={r.stop}>
        <X />
      </Button>
    </div>
  );
}
