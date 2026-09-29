import { NativeSelect } from "@/components/ui/input";
import { useProfilePrefs } from "@/features/profiles";
import { useReaderPrefs } from "@/features/reader";
import { cn } from "@/lib/utils";
import { PAGE_THEMES, type PdfDarkMode } from "@/readers";
import { Group, Row, Slider, Switch } from "../parts";

const PDF_MODES: { value: PdfDarkMode; label: string }[] = [
  { value: "recolour", label: "Recolour text, keep pictures" },
  { value: "invert", label: "Invert (good for scans)" },
  { value: "dim", label: "Dim the page" },
  { value: "off", label: "Leave pages as they are" },
];

export function ReaderSettings() {
  const reader = useReaderPrefs();
  const prefs = useProfilePrefs((s) => s.prefs.reader);
  const adhd = useProfilePrefs((s) => s.prefs.adhd);
  const update = useProfilePrefs((s) => s.update);
  return (
    <>
      <Group title="Pages" scope="yours" description="Also in the reader's Aa menu while reading.">
        <div className="grid grid-cols-3 gap-2.5 p-4" role="radiogroup" aria-label="Page theme">
          {PAGE_THEMES.map((t) => (
            <button
              key={t.id}
              type="button"
              role="radio"
              aria-checked={reader.theme === t.id}
              onClick={() => reader.set({ theme: t.id })}
              className={cn(
                "flex h-16 flex-col justify-center rounded-lg border px-3 text-left",
                reader.theme === t.id && "ring-2 ring-primary",
              )}
              style={{ background: t.bg, color: t.fg }}
            >
              <span className="font-serif text-[15px]">Aa</span>
              <span className="font-sans text-[12px] opacity-80">{t.name}</span>
            </button>
          ))}
        </div>
        <Row label="Dark pages when the app is dark" help="Uses Night while the app theme is dark.">
          <Switch
            label="Dark pages when the app is dark"
            checked={reader.followApp}
            onChange={(v) => reader.set({ followApp: v })}
          />
        </Row>
        <Row label="Dark PDF pages" htmlFor="pdfmode" help="How PDFs look on a dark page theme.">
          <NativeSelect
            id="pdfmode"
            value={reader.pdfMode}
            onChange={(e) => reader.set({ pdfMode: e.target.value as PdfDarkMode })}
            className="w-64"
          >
            {PDF_MODES.map((m) => (
              <option key={m.value} value={m.value}>
                {m.label}
              </option>
            ))}
          </NativeSelect>
        </Row>
        <Row label="Brightness" help="For all books. Lower it for reading at night.">
          <Slider
            label="Brightness"
            min={50}
            max={130}
            value={prefs.brightness}
            onChange={(v) => update({ reader: { brightness: v } })}
            format={(v) => `${v}%`}
          />
        </Row>
        <Row label="Contrast" help="Raise it for faint scans.">
          <Slider
            label="Contrast"
            min={70}
            max={160}
            value={prefs.contrast}
            onChange={(v) => update({ reader: { contrast: v } })}
            format={(v) => `${v}%`}
          />
        </Row>
      </Group>
      <Group
        title="Text"
        scope="yours"
        description="For EPUB, Markdown, FB2 and other books whose text can reflow."
      >
        <Row
          label="Text size"
          help="The starting size; change it while reading with Ctrl and + or −."
        >
          <Slider
            label="Text size"
            min={70}
            max={200}
            step={5}
            value={prefs.fontScale}
            onChange={(v) => update({ reader: { fontScale: v } })}
            format={(v) => `${v}%`}
          />
        </Row>
        <Row label="Line spacing">
          <Slider
            label="Line spacing"
            min={1.2}
            max={2.2}
            step={0.05}
            value={prefs.lineHeight}
            onChange={(v) => update({ reader: { lineHeight: v } })}
            format={(v) => v.toFixed(2)}
          />
        </Row>
      </Group>
      <Group
        title="ADHD reading"
        scope="yours"
        description="Help to keep your place while reading. When one is on, the reader's Aa menu can pause them for a while."
      >
        <Row
          label="Bionic reading"
          help="The start of each word is bold, so the eye jumps from word to word. For EPUB, MOBI, FB2, Markdown and text books, not PDFs or scans."
        >
          <Switch
            label="Bionic reading"
            checked={adhd.bionic}
            onChange={(v) => update({ adhd: { bionic: v } })}
          />
        </Row>
        {adhd.bionic && (
          <>
            <Row label="Bold part of each word" help="Short words always have one bold letter.">
              <Slider
                label="Bold part of each word"
                min={0.3}
                max={0.7}
                step={0.05}
                value={adhd.fixation}
                onChange={(v) => update({ adhd: { fixation: v } })}
                format={(v) => `${Math.round(v * 100)}%`}
              />
            </Row>
            <Row label="Rest of the word" help="Lighter makes the bold starts stand out more.">
              <Slider
                label="Rest of the word"
                min={0.4}
                max={1}
                step={0.05}
                value={adhd.fade}
                onChange={(v) => update({ adhd: { fade: v } })}
                format={(v) => `${Math.round(v * 100)}%`}
              />
            </Row>
          </>
        )}
        <Row
          label="Line highlight"
          help="Highlights the line of text under the pointer, in every book with text."
        >
          <Switch
            label="Line highlight"
            checked={adhd.line}
            onChange={(v) => update({ adhd: { line: v } })}
          />
        </Row>
        <Row
          label="Reading mask"
          help="Dims the page except a strip around the pointer. Works in every book."
        >
          <Switch
            label="Reading mask"
            checked={adhd.mask}
            onChange={(v) => update({ adhd: { mask: v } })}
          />
        </Row>
        {adhd.mask && (
          <Row label="Mask opening" help="Also in the reader's Aa menu.">
            <Slider
              label="Mask opening"
              min={40}
              max={360}
              step={10}
              value={adhd.maskHeight}
              onChange={(v) => update({ adhd: { maskHeight: v } })}
              format={(v) => `${v} px`}
            />
          </Row>
        )}
      </Group>
    </>
  );
}
