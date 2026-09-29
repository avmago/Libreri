import { Check, Minus, Plus, Type } from "lucide-react";
import { Button } from "@/components/ui/button";
import { DropdownMenu, menuContent, menuLabel, menuSeparator } from "@/components/ui/menu";
import { PAGE_THEMES, type PageLayout, type PdfDarkMode, type PageThemeId } from "@/readers";
import { cn } from "@/lib/utils";
import { useProfilePrefs } from "@/features/profiles";
import { useReaderPrefs } from "../prefs";
import { adhdTools, useAdhd, useAdhdPause } from "../adhd/state";

const LAYOUTS: [PageLayout["mode"], string][] = [
  ["single", "One page"],
  ["spread", "Two pages side by side"],
  ["scroll", "Continuous (webtoon)"],
];

const itemClass =
  "flex h-8 cursor-default items-center gap-2.5 rounded-md px-2 outline-none data-[highlighted]:bg-muted";

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
  pageLayout,
  onPageLayout,
  bionicHere,
}: {
  isPdf: boolean;
  /** Bionic reading can change this book's text. */
  bionicHere: boolean;
  zoomLabel: string;
  onZoom: (step: 1 | -1 | 0) => void;
  /** Comics only. */
  pageLayout?: PageLayout | null;
  onPageLayout?: (change: Partial<PageLayout>) => void;
}) {
  const prefs = useReaderPrefs();
  const adhd = useAdhd();
  const setPaused = useAdhdPause((s) => s.setPaused);
  const updatePrefs = useProfilePrefs((s) => s.update);
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
          {pageLayout && onPageLayout && (
            <>
              <DropdownMenu.Separator className={menuSeparator} />
              <DropdownMenu.Label className={menuLabel}>COMIC PAGES</DropdownMenu.Label>
              {LAYOUTS.map(([mode, label]) => (
                <DropdownMenu.CheckboxItem
                  key={mode}
                  checked={pageLayout.mode === mode}
                  onCheckedChange={() => onPageLayout({ mode })}
                  onSelect={(e) => e.preventDefault()}
                  className={itemClass}
                >
                  <span className="flex size-4 items-center justify-center">
                    {pageLayout.mode === mode && <Check className="size-4" />}
                  </span>
                  {label}
                </DropdownMenu.CheckboxItem>
              ))}
              <DropdownMenu.CheckboxItem
                checked={pageLayout.rightToLeft}
                disabled={pageLayout.mode === "scroll"}
                onCheckedChange={(v) => onPageLayout({ rightToLeft: v })}
                onSelect={(e) => e.preventDefault()}
                className={cn(itemClass, "data-[disabled]:opacity-50")}
              >
                <span className="flex size-4 items-center justify-center">
                  {pageLayout.rightToLeft && <Check className="size-4" />}
                </span>
                Right to left (manga)
              </DropdownMenu.CheckboxItem>
            </>
          )}
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
          {adhd.enabled && (
            <>
              <DropdownMenu.Separator className={menuSeparator} />
              <DropdownMenu.Label className={menuLabel}>ADHD READING</DropdownMenu.Label>
              <DropdownMenu.CheckboxItem
                checked={!adhd.paused}
                onCheckedChange={(v) => setPaused(!v)}
                onSelect={(e) => e.preventDefault()}
                className={cn(itemClass, "h-auto min-h-8 py-1")}
              >
                <span className="flex size-4 shrink-0 items-center justify-center">
                  {!adhd.paused && <Check className="size-4" />}
                </span>
                <span className="flex flex-col">
                  ADHD reading
                  <span className="text-[11.5px] text-muted-foreground">
                    {adhdTools({ ...adhd, bionic: adhd.bionic && bionicHere })}
                    {adhd.bionic && !bionicHere && (
                      <>
                        {adhd.line || adhd.mask ? " · " : ""}Bionic reading is not available for
                        this book
                      </>
                    )}
                  </span>
                </span>
              </DropdownMenu.CheckboxItem>
              {adhd.mask && (
                <label className="flex items-center gap-2.5 px-2 py-1.5">
                  <span className="shrink-0 text-muted-foreground">Mask opening</span>
                  <input
                    type="range"
                    min={40}
                    max={360}
                    step={10}
                    value={adhd.maskHeight}
                    disabled={adhd.paused}
                    aria-label="Height of the strip the reading mask leaves clear"
                    onChange={(e) => updatePrefs({ adhd: { maskHeight: Number(e.target.value) } })}
                    // The menu would take the arrow keys to move between items.
                    onKeyDown={(e) => e.stopPropagation()}
                    className="min-w-0 flex-1 disabled:opacity-50"
                  />
                </label>
              )}
            </>
          )}
          {isPdf && (
            <>
              <DropdownMenu.Separator className={menuSeparator} />
              <DropdownMenu.Label className={menuLabel}>DARK PAGES</DropdownMenu.Label>
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
