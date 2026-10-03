import type { ReactNode } from "react";
import {
  ArrowUpRight,
  Circle,
  Eraser,
  FileDown,
  Highlighter,
  ImagePlus,
  Layers,
  Minus,
  MousePointer2,
  Pen,
  PenLine,
  Redo2,
  Ruler,
  Square,
  Stamp,
  StickyNote,
  Trash2,
  Type,
  Undo2,
  Wand2,
} from "lucide-react";
import {
  DropdownMenu,
  menuContent,
  menuItem,
  menuLabel,
  menuSeparator,
} from "@/components/ui/menu";
import { useProfilePrefs } from "@/features/profiles";
import { cn } from "@/lib/utils";
import { markupModel, type MeasureKind, type ToolName, type Unit } from "@/readers";
import type { MarkupState } from "./useMarkup";

const { COLORS, HIGHLIGHTER_COLORS, WIDTHS, STAMPS, convertScale } = markupModel;

const TOOLS: { id: ToolName; label: string; Icon: typeof Pen }[] = [
  { id: "select", label: "Select and move", Icon: MousePointer2 },
  { id: "pen", label: "Pen", Icon: Pen },
  { id: "highlighter", label: "Highlighter pen", Icon: Highlighter },
  { id: "eraser", label: "Eraser (removes whole marks)", Icon: Eraser },
  { id: "rect", label: "Rectangle", Icon: Square },
  { id: "ellipse", label: "Ellipse", Icon: Circle },
  { id: "line", label: "Line", Icon: Minus },
  { id: "arrow", label: "Arrow", Icon: ArrowUpRight },
  { id: "text", label: "Text box", Icon: Type },
  { id: "note", label: "Sticky note", Icon: StickyNote },
];

const MEASURES: [MeasureKind, string][] = [
  ["distance", "Distance"],
  ["perimeter", "Perimeter (click points, double-click to finish)"],
  ["area", "Area (click corners, double-click to finish)"],
  ["angle", "Angle (three points)"],
];

const UNITS: Unit[] = ["mm", "cm", "m", "in", "ft", "pt"];

function ToolButton({
  active,
  label,
  onClick,
  children,
  disabled,
}: {
  active?: boolean;
  label: string;
  onClick?: () => void;
  children: ReactNode;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      aria-label={label}
      title={label}
      aria-pressed={active}
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex size-8 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground disabled:opacity-35 [&_svg]:size-4",
        active && "bg-muted text-foreground ring-1 ring-border",
      )}
    >
      {children}
    </button>
  );
}

const sep = <span className="mx-1 h-5 w-px shrink-0 bg-border" aria-hidden />;

/** The markup tools, shown under the reader toolbar in markup mode. */
export function MarkupToolbar({
  m,
  onPickImage,
  onNewSignature,
  onNewStamp,
  onExport,
  onLayers,
  layersOpen,
}: {
  m: MarkupState;
  onPickImage: () => void;
  onNewSignature: () => void;
  onNewStamp: () => void;
  onExport: () => void;
  onLayers: () => void;
  layersOpen: boolean;
}) {
  const { signatures, stamps } = useProfilePrefs((s) => s.prefs.markup);
  const s = m.style;
  const t = m.tool;
  const inkish = ["pen", "rect", "ellipse", "line", "arrow", "text", "select"].includes(t);
  const lines = ["pen", "rect", "ellipse", "line", "arrow"].includes(t);
  const selectedHasStroke = m.selected && "width" in m.selected.item;

  return (
    <div
      role="toolbar"
      aria-label="Markup tools"
      className="flex h-11 shrink-0 items-center gap-0.5 overflow-x-auto border-b bg-background px-3"
    >
      {TOOLS.map(({ id, label, Icon }) => (
        <ToolButton key={id} label={label} active={t === id} onClick={() => m.setTool(id)}>
          <Icon />
        </ToolButton>
      ))}

      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button
            type="button"
            aria-label="Stamps"
            title="Stamps"
            className={cn(
              "flex size-8 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground [&_svg]:size-4",
              t === "stamp" && "bg-muted text-foreground ring-1 ring-border",
            )}
          >
            <Stamp />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content sideOffset={6} className={menuContent}>
            <DropdownMenu.Label className={menuLabel}>
              STAMP, THEN CLICK THE PAGE
            </DropdownMenu.Label>
            {[...STAMPS, ...stamps].map((text) => (
              <DropdownMenu.Item
                key={text}
                className={menuItem}
                onSelect={() => {
                  m.setStyle({ stamp: text });
                  m.setTool("stamp");
                }}
              >
                <span className="font-semibold tracking-wide">{text}</span>
              </DropdownMenu.Item>
            ))}
            <DropdownMenu.Separator className={menuSeparator} />
            <DropdownMenu.Item className={menuItem} onSelect={onNewStamp}>
              New stamp…
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button
            type="button"
            aria-label="Signature"
            title="Signature"
            className={cn(
              "flex size-8 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground [&_svg]:size-4",
              t === "image" && s.image?.signature && "bg-muted text-foreground ring-1 ring-border",
            )}
          >
            <PenLine />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content sideOffset={6} className={cn(menuContent, "w-64")}>
            <DropdownMenu.Label className={menuLabel}>SIGN, THEN CLICK THE PAGE</DropdownMenu.Label>
            {signatures.map((sig) => (
              <DropdownMenu.Item
                key={sig.id}
                className={cn(menuItem, "h-12")}
                onSelect={() => {
                  m.setStyle({ image: { src: sig.src, aspect: sig.aspect, signature: true } });
                  m.setTool("image");
                }}
              >
                <img
                  src={sig.src}
                  alt="Signature"
                  className="h-9 max-w-full object-contain dark:invert"
                />
              </DropdownMenu.Item>
            ))}
            {signatures.length > 0 && <DropdownMenu.Separator className={menuSeparator} />}
            <DropdownMenu.Item className={menuItem} onSelect={onNewSignature}>
              New signature…
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      <ToolButton
        label="Picture"
        active={t === "image" && !s.image?.signature}
        onClick={onPickImage}
      >
        <ImagePlus />
      </ToolButton>

      <DropdownMenu.Root>
        <DropdownMenu.Trigger asChild>
          <button
            type="button"
            aria-label="Measure"
            title="Measure"
            className={cn(
              "flex size-8 items-center justify-center rounded-md text-muted-foreground hover:bg-muted hover:text-foreground [&_svg]:size-4",
              t === "measure" && "bg-muted text-foreground ring-1 ring-border",
            )}
          >
            <Ruler />
          </button>
        </DropdownMenu.Trigger>
        <DropdownMenu.Portal>
          <DropdownMenu.Content sideOffset={6} className={cn(menuContent, "w-72")}>
            <DropdownMenu.Label className={menuLabel}>MEASURE</DropdownMenu.Label>
            {MEASURES.map(([kind, label]) => (
              <DropdownMenu.Item
                key={kind}
                className={menuItem}
                onSelect={() => {
                  m.setStyle({ measure: kind });
                  m.setTool("measure");
                }}
              >
                {label}
              </DropdownMenu.Item>
            ))}
            <DropdownMenu.Separator className={menuSeparator} />
            <DropdownMenu.Item className={menuItem} onSelect={m.startCalibrate}>
              <Wand2 /> Calibrate: draw over a known length…
            </DropdownMenu.Item>
            <DropdownMenu.Label className={menuLabel}>
              {s.scale ? `SCALE: CALIBRATED (${s.scale.unit})` : "SCALE: TRUE SIZE OF THE PAGE"}
            </DropdownMenu.Label>
            <div className="flex flex-wrap gap-1 px-2 pb-1.5">
              {UNITS.map((u) => (
                <button
                  key={u}
                  type="button"
                  onClick={() =>
                    m.setStyle({
                      scale: s.scale
                        ? convertScale(s.scale, u)
                        : { perUnit: markupModel.defaultScale(true, u).perUnit, unit: u },
                    })
                  }
                  className={cn(
                    "rounded border px-1.5 py-0.5 text-[12px]",
                    (s.scale?.unit ?? "mm") === u && "border-foreground font-medium",
                  )}
                >
                  {u}
                </button>
              ))}
            </div>
            {s.scale && (
              <DropdownMenu.Item className={menuItem} onSelect={() => m.setStyle({ scale: null })}>
                Forget the calibration
              </DropdownMenu.Item>
            )}
          </DropdownMenu.Content>
        </DropdownMenu.Portal>
      </DropdownMenu.Root>

      {sep}

      {t === "highlighter"
        ? HIGHLIGHTER_COLORS.map((c) => (
            <button
              key={c}
              type="button"
              aria-label={`Highlighter colour ${c}`}
              aria-pressed={s.highlighter === c}
              onClick={() => m.setStyle({ highlighter: c })}
              className={cn(
                "size-5 shrink-0 rounded-full border",
                s.highlighter === c &&
                  "ring-2 ring-foreground ring-offset-1 ring-offset-background",
              )}
              style={{ background: c }}
            />
          ))
        : (inkish || selectedHasStroke) &&
          COLORS.map((c) => (
            <button
              key={c}
              type="button"
              aria-label={`Colour ${c}`}
              aria-pressed={s.color === c}
              onClick={() => m.setStyle({ color: c })}
              className={cn(
                "mx-0.5 size-5 shrink-0 rounded-full border",
                s.color === c && "ring-2 ring-foreground ring-offset-1 ring-offset-background",
              )}
              style={{ background: c }}
            />
          ))}

      {(lines || t === "highlighter" || selectedHasStroke) && (
        <>
          {sep}
          {WIDTHS.map((w, i) => (
            <ToolButton
              key={w}
              label={`Width ${i + 1}`}
              active={s.width === w}
              onClick={() => m.setStyle({ width: w })}
            >
              <span
                className="rounded-full bg-foreground"
                style={{ width: 4 + i * 3, height: 4 + i * 3 }}
              />
            </ToolButton>
          ))}
        </>
      )}

      {(["rect", "ellipse", "line", "arrow"].includes(t) || (t === "pen" && s.snap)) && (
        <select
          aria-label="Line style"
          value={s.dash}
          onChange={(e) => m.setStyle({ dash: e.target.value as typeof s.dash })}
          className="ml-1 h-7 shrink-0 rounded-md border bg-background px-1 text-[12.5px]"
        >
          <option value="solid">Solid</option>
          <option value="dashed">Dashed</option>
          <option value="dotted">Dotted</option>
        </select>
      )}
      {(t === "rect" || t === "ellipse") && (
        <label className="ml-2 flex shrink-0 items-center gap-1 text-[12.5px] whitespace-nowrap">
          <input
            type="checkbox"
            checked={s.fill}
            onChange={(e) => m.setStyle({ fill: e.target.checked })}
          />
          Fill
        </label>
      )}
      {t === "pen" && (
        <label
          className="ml-2 flex shrink-0 items-center gap-1 text-[12.5px] whitespace-nowrap"
          title="Rough circles, boxes and lines become clean shapes"
        >
          <input
            type="checkbox"
            checked={s.snap}
            onChange={(e) => m.setStyle({ snap: e.target.checked })}
          />
          Snap to shapes
        </label>
      )}
      {t === "text" && (
        <>
          <select
            aria-label="Font"
            value={s.font}
            onChange={(e) => m.setStyle({ font: e.target.value as typeof s.font })}
            className="ml-1 h-7 shrink-0 rounded-md border bg-background px-1 text-[12.5px]"
          >
            <option value="sans">Sans</option>
            <option value="serif">Serif</option>
            <option value="hand">Handwriting</option>
          </select>
          <select
            aria-label="Text size"
            value={s.textSize}
            onChange={(e) => m.setStyle({ textSize: Number(e.target.value) })}
            className="ml-1 h-7 shrink-0 rounded-md border bg-background px-1 text-[12.5px]"
          >
            <option value={0.013}>Small</option>
            <option value={0.018}>Normal</option>
            <option value={0.026}>Large</option>
            <option value={0.04}>Huge</option>
          </select>
        </>
      )}

      <div className="flex-1" />
      <ToolButton label="Undo (Ctrl+Z)" onClick={m.undo} disabled={!m.canUndo}>
        <Undo2 />
      </ToolButton>
      <ToolButton label="Redo (Ctrl+Shift+Z)" onClick={m.redo} disabled={!m.canRedo}>
        <Redo2 />
      </ToolButton>
      <ToolButton
        label="Delete the selected mark (Delete)"
        onClick={m.deleteSelected}
        disabled={!m.selected}
      >
        <Trash2 />
      </ToolButton>
      {sep}
      <ToolButton label="Layers and marks" active={layersOpen} onClick={onLayers}>
        <Layers />
      </ToolButton>
      <button
        type="button"
        onClick={onExport}
        className="ml-1 flex h-8 shrink-0 items-center gap-1.5 rounded-md border px-2.5 text-[12.5px] font-medium hover:bg-muted [&_svg]:size-4"
      >
        <FileDown /> Export marked-up copy…
      </button>
    </div>
  );
}
