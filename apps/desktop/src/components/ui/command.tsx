import * as React from "react";
import * as DialogPrimitive from "@radix-ui/react-dialog";
import { Command as CommandPrimitive } from "cmdk";
import { Search } from "lucide-react";
import { cn } from "@/lib/utils";

export function CommandDialog({
  open,
  onOpenChange,
  children,
  label,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  children: React.ReactNode;
  label: string;
}) {
  return (
    <DialogPrimitive.Root open={open} onOpenChange={onOpenChange}>
      <DialogPrimitive.Portal>
        <DialogPrimitive.Overlay className="fixed inset-0 z-50 bg-black/40" />
        <DialogPrimitive.Content
          className="fixed top-[18%] left-1/2 z-50 w-[560px] max-w-[calc(100vw-32px)] -translate-x-1/2 overflow-hidden rounded-lg border bg-popover text-popover-foreground shadow-2xl"
          aria-describedby={undefined}
        >
          <DialogPrimitive.Title className="sr-only">{label}</DialogPrimitive.Title>
          <CommandPrimitive label={label} className="flex flex-col">
            {children}
          </CommandPrimitive>
        </DialogPrimitive.Content>
      </DialogPrimitive.Portal>
    </DialogPrimitive.Root>
  );
}

export function CommandInput(props: React.ComponentProps<typeof CommandPrimitive.Input>) {
  return (
    <div className="flex items-center gap-2 border-b px-3">
      <Search className="size-4 text-muted-foreground" aria-hidden />
      <CommandPrimitive.Input
        {...props}
        className="h-11 w-full bg-transparent text-sm outline-none placeholder:text-muted-foreground"
      />
    </div>
  );
}

export function CommandList(props: React.ComponentProps<typeof CommandPrimitive.List>) {
  return <CommandPrimitive.List {...props} className="max-h-80 overflow-y-auto p-1.5" />;
}

export function CommandEmpty(props: React.ComponentProps<typeof CommandPrimitive.Empty>) {
  return <CommandPrimitive.Empty {...props} className="py-6 text-center text-muted-foreground" />;
}

export function CommandGroup({
  className,
  ...props
}: React.ComponentProps<typeof CommandPrimitive.Group>) {
  return (
    <CommandPrimitive.Group
      {...props}
      className={cn(
        "[&_[cmdk-group-heading]]:px-2 [&_[cmdk-group-heading]]:py-1.5 [&_[cmdk-group-heading]]:text-[11px] [&_[cmdk-group-heading]]:font-semibold [&_[cmdk-group-heading]]:tracking-wide [&_[cmdk-group-heading]]:text-muted-foreground",
        className,
      )}
    />
  );
}

export function CommandItem({
  className,
  ...props
}: React.ComponentProps<typeof CommandPrimitive.Item>) {
  return (
    <CommandPrimitive.Item
      {...props}
      className={cn(
        "flex h-9 cursor-default items-center gap-2.5 rounded-md px-2 data-[selected=true]:bg-muted [&_svg]:size-4 [&_svg]:text-muted-foreground",
        className,
      )}
    />
  );
}
