import { useState } from "react";
import { FileSearch, type LucideIcon } from "lucide-react";
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
  onSearch,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  actions: PaletteAction[];
  /** "Search for …": opens the search screen with what was typed. */
  onSearch?: (query: string) => void;
}) {
  const [typed, setTyped] = useState("");
  // Opens empty each time: what was typed goes when it closes.
  const [wasOpen, setWasOpen] = useState(open);
  if (open !== wasOpen) {
    setWasOpen(open);
    if (!open) setTyped("");
  }
  const groups = [...new Set(actions.map((a) => a.group))];
  const run = (a: PaletteAction) => {
    onOpenChange(false);
    a.run();
  };

  return (
    <CommandDialog open={open} onOpenChange={onOpenChange} label="Command palette">
      <CommandInput
        placeholder={onSearch ? "Search or type a command…" : "Type a command…"}
        value={typed}
        onValueChange={setTyped}
      />
      <CommandList>
        {onSearch && typed.trim().length >= 2 && (
          <CommandGroup heading="Search" forceMount>
            <CommandItem
              forceMount
              value={`search ${typed}`}
              onSelect={() => {
                onOpenChange(false);
                onSearch(typed.trim());
                setTyped("");
              }}
            >
              <FileSearch />
              <span className="flex-1">Search books, text and notes for “{typed.trim()}”</span>
            </CommandItem>
          </CommandGroup>
        )}
        {!(onSearch && typed.trim().length >= 2) && (
          <CommandEmpty>No matching commands.</CommandEmpty>
        )}
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
