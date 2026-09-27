import { Check, Minus, Plus, Type } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DropdownMenu, menuContent, menuLabel, menuSeparator } from "@/components/ui/menu";
import { PAGE_THEMES, type PdfDarkMode, type PageThemeId } from "@/readers";
import { cn } from "@/lib/utils";
import { useReaderPrefs } from "../prefs";

const PDF_MODES: [PdfDarkMode, string][] = [
  ["recolour", "Recolour text, keep images"],
  ["invert", "Invert everything (scans)"],
  ["dim", "Dim only"],
  ["off", "Off"],
];

/** Page theme, "follow app theme", dark PDF mode and text size / zoom. */
export function AppearanceMenu({
  isPdf,
  zoomLabel,
  onZoom,
}: {
  isPdf: boolean;
  zoomLabel: string;
  onZoom: (step: 1 | -1 | 0) => void;
}) {
  const prefs = useReaderPrefs();
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button variant="ghost" size="icon" aria-label="Page appearance" title="Page appearance">
          <Type />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" sideOffset={6} className={cn(menuContent, "w-72 p-2")}>
          <DropdownMenu.Label className={menuLabel}>
            {isPdf ? "ZOOM" : "TEXT SIZE"}
          </DropdownMenu.Label>
          <div className="flex items-center gap-2 px-2 pb-2">
            <Button variant="outline" size="icon" aria-label="Smaller" onClick={() => onZoom(-1)}>
              <Minus />
            </Button>
            <button
              type="button"
              onClick={() => onZoom(0)}
              className="flex-1 rounded-md py-1 text-center tabular-nums hover:bg-muted"
              title={isPdf ? "Automatic size" : "Reset"}
            >
              {zoomLabel}
            </button>
            <Button variant="outline" size="icon" aria-label="Larger" onClick={() => onZoom(1)}>
              <Plus />
            </Button>
          </div>
          <DropdownMenu.Separator className={menuSeparator} />
          <DropdownMenu.Label className={menuLabel}>PAGE</DropdownMenu.Label>
          <div className="grid grid-cols-3 gap-1.5 px-1 pb-2">
            {PAGE_THEMES.map((t) => (
              <button
                key={t.id}
                type="button"
                onClick={() => prefs.set({ theme: t.id as PageThemeId })}
                aria-pressed={prefs.theme === t.id}
                className={cn(
                  "flex h-12 flex-col items-center justify-center gap-0.5 rounded-md border text-[11px]",
                  prefs.theme === t.id && "ring-2 ring-ring",
                )}
                style={{
                  background: t.bg,
                  color: t.fg,
                  borderColor: t.dark ? "#3f3f46" : "#e4e4e7",
                }}
              >
                <span className="font-serif text-[15px] leading-none">Aa</span>
                {t.name}
              </button>
            ))}
          </div>
          <DropdownMenu.CheckboxItem
            checked={prefs.followApp}
            onCheckedChange={(v) => prefs.set({ followApp: v })}
            onSelect={(e) => e.preventDefault()}
            className="flex h-8 cursor-default items-center gap-2.5 rounded-md px-2 outline-none data-[highlighted]:bg-muted"
          >
            <span className="flex size-4 items-center justify-center">
              {prefs.followApp && <Check className="size-4" />}
            </span>
            Dark pages when the app is dark
          </DropdownMenu.CheckboxItem>
          {isPdf && (
            <>
              <DropdownMenu.Separator className={menuSeparator} />
              <DropdownMenu.Label className={menuLabel}>DARK PDF PAGES</DropdownMenu.Label>
              {PDF_MODES.map(([mode, label]) => (
                <DropdownMenu.CheckboxItem
                  key={mode}
                  checked={prefs.pdfMode === mode}
                  onCheckedChange={() => prefs.set({ pdfMode: mode })}
                  onSelect={(e) => e.preventDefault()}
                  className="flex h-8 cursor-default items-center gap-2.5 rounded-md px-2 outline-none data-[highlighted]:bg-muted"
                >
                  <span className="flex size-4 items-center justify-center">
                    {prefs.pdfMode === mode && <Check className="size-4" />}
                  </span>
                  {label}
                </DropdownMenu.CheckboxItem>
              ))}
            </>
          )}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
