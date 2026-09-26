import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import { Check, Contrast, Monitor, Moon, Sun } from "lucide-react";
import type { Theme } from "@/lib/ipc";
import { Button } from "@/components/ui/button";
import { useSetTheme, useSettings } from "../api";

const OPTIONS: { value: Theme; label: string; Icon: typeof Sun }[] = [
  { value: "system", label: "System", Icon: Monitor },
  { value: "light", label: "Light", Icon: Sun },
  { value: "dark", label: "Dark", Icon: Moon },
  { value: "highContrast", label: "High contrast", Icon: Contrast },
];

export function ThemeMenu() {
  const { data } = useSettings();
  const setTheme = useSetTheme();
  const current = data?.theme ?? "system";
  const CurrentIcon = OPTIONS.find((o) => o.value === current)?.Icon ?? Monitor;

  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger asChild>
        <Button variant="ghost" size="icon" aria-label="Theme">
          <CurrentIcon />
        </Button>
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content
          align="end"
          sideOffset={6}
          className="z-50 min-w-44 rounded-lg border bg-popover p-1 text-popover-foreground shadow-lg"
        >
          {OPTIONS.map(({ value, label, Icon }) => (
            <DropdownMenu.Item
              key={value}
              onSelect={() => setTheme.mutate(value)}
              className="flex h-8 cursor-default items-center gap-2.5 rounded-md px-2 outline-none data-[highlighted]:bg-muted [&_svg]:size-4"
            >
              <Icon className="text-muted-foreground" />
              <span className="flex-1">{label}</span>
              {value === current && <Check />}
            </DropdownMenu.Item>
          ))}
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
