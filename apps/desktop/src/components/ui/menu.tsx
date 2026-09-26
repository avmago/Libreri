/**
 * Dropdown and context menus share one look. Use the Radix primitives with
 * these class names so every menu in the app matches.
 */
import * as DropdownMenu from "@radix-ui/react-dropdown-menu";
import * as ContextMenu from "@radix-ui/react-context-menu";
import { Check } from "lucide-react";
import type * as React from "react";
import { cn } from "@/lib/utils";

export const menuContent =
  "z-50 min-w-48 overflow-hidden rounded-lg border bg-popover p-1 text-popover-foreground shadow-lg";
export const menuItem =
  "flex h-8 cursor-default items-center gap-2.5 rounded-md px-2 text-[13px] outline-none select-none data-[disabled]:opacity-40 data-[highlighted]:bg-muted [&_svg]:size-4 [&_svg]:shrink-0 [&_svg]:text-muted-foreground";
export const menuSeparator = "-mx-1 my-1 h-px bg-border";
export const menuLabel =
  "px-2 pt-1.5 pb-1 text-[11px] font-semibold tracking-wide text-muted-foreground";

export { DropdownMenu, ContextMenu };

export function MenuShortcut({ children }: { children: React.ReactNode }) {
  return (
    <span className="ml-auto pl-4 font-mono text-[11px] text-muted-foreground">{children}</span>
  );
}

export function DropdownCheckItem({
  checked,
  onCheckedChange,
  children,
  className,
}: {
  checked: boolean;
  onCheckedChange: (v: boolean) => void;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <DropdownMenu.CheckboxItem
      checked={checked}
      onCheckedChange={onCheckedChange}
      onSelect={(e) => e.preventDefault()}
      className={cn(menuItem, className)}
    >
      <span className="flex size-4 items-center justify-center">
        {checked && <Check className="!text-foreground" />}
      </span>
      {children}
    </DropdownMenu.CheckboxItem>
  );
}
