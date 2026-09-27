import { Check, Contrast, Monitor, Moon, Sun } from "lucide-react";
import type { Theme } from "@/lib/ipc";
import { ACCENTS } from "@/lib/theme";
import { cn } from "@/lib/utils";
import { useSetAccent, useSetTheme, useSettings } from "../../api";
import { Group, Row } from "../parts";

const THEMES: { value: Theme; label: string; Icon: typeof Sun }[] = [
  { value: "system", label: "System", Icon: Monitor },
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
  { value: "highContrast", label: "High contrast", Icon: Contrast },
];

export function AppearanceSettings() {
  const { data: settings } = useSettings();
  const setTheme = useSetTheme();
  const setAccent = useSetAccent();
  const theme = settings?.theme ?? "system";
  const accent = settings?.accent ?? null;
  return (
    <Group title="Theme" scope="computer" description="Page colours for books are in Reader.">
      <div className="grid grid-cols-4 gap-3 p-4" role="radiogroup" aria-label="Theme">
        {THEMES.map(({ value, label, Icon }) => (
          <button
            key={value}
            type="button"
            role="radio"
            aria-checked={theme === value}
            onClick={() => setTheme.mutate(value)}
            className={cn(
              "flex flex-col items-center gap-2 rounded-lg border p-4",
              theme === value ? "border-primary ring-2 ring-primary" : "hover:bg-muted/60",
            )}
          >
            <Icon className="size-5" aria-hidden />
            <span className="text-[13px] font-medium">{label}</span>
          </button>
        ))}
      </div>
      <Row label="Accent colour" help="Buttons, selection and focus rings. Black is the default.">
        <div className="flex gap-1.5" role="radiogroup" aria-label="Accent colour">
          {ACCENTS.map((a) => (
            <button
              key={a.label}
              type="button"
              role="radio"
              aria-checked={accent === a.value}
              aria-label={a.label}
              title={a.label}
              onClick={() => setAccent.mutate(a.value)}
              className="flex size-6 items-center justify-center rounded-full border border-black/10 text-white"
              style={{ background: a.value ?? "#18181b" }}
            >
              {accent === a.value && <Check className="size-3.5" />}
            </button>
          ))}
        </div>
      </Row>
    </Group>
  );
}
