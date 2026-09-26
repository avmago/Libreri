import type { LucideIcon } from "lucide-react";
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { Kbd } from "@/components/ui/kbd";

export interface PaletteAction {
  id: string;
  label: string;
  group: string;
  icon: LucideIcon;
  shortcut?: string;
  run: () => void;
}

/**
 * The ⌘K / Ctrl+K palette. Actions are passed in by the app shell so the
 * palette itself knows nothing about individual features.
 */
export function CommandPalette({
  open,
  onOpenChange,
  actions,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  actions: PaletteAction[];
}) {
  const groups = [...new Set(actions.map((a) => a.group))];
  const run = (a: PaletteAction) => {
    onOpenChange(false);
    a.run();
  };

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} label="Command palette">
      <CommandInput placeholder="Type a command…" />
      <CommandList>
        <CommandEmpty>No matching commands.</CommandEmpty>
        {groups.map((g) => (
          <CommandGroup key={g} heading={g}>
            {actions
              .filter((a) => a.group === g)
              .map((a) => (
                <CommandItem key={a.id} value={`${a.group} ${a.label}`} onSelect={() => run(a)}>
                  <a.icon />
                  <span className="flex-1">{a.label}</span>
                  {a.shortcut && <Kbd shortcut={a.shortcut} />}
                </CommandItem>
              ))}
          </CommandGroup>
        ))}
      </CommandList>
    </CommandDialog>
  );
}
