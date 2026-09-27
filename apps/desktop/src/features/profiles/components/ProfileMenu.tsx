import { Lock, LogOut, Settings, Users } from "lucide-react";
import {
  DropdownMenu,
  MenuShortcut,
  menuContent,
  menuItem,
  menuSeparator,
} from "@/components/ui/menu";
import type { SessionDto } from "@/lib/ipc";
import { displayKeys, platform } from "@/lib/shortcuts";
import { KIND_LABELS } from "../model";
import { ProfileAvatar } from "./ProfileAvatar";

/** The signed-in person, in the corner of the tab strip. */
export function ProfileMenu({
  session,
  onSwitch,
  onOpenSettings,
  lockShortcut,
}: {
  session: SessionDto;
  /** Flush unsaved state, then sign out. */
  onSwitch: () => void;
  onOpenSettings: () => void;
  lockShortcut?: string | null;
}) {
  const p = session.profile;
  const guest = p.kind === "guest";
  return (
    <DropdownMenu.Root>
      <DropdownMenu.Trigger
        aria-label={`Signed in as ${p.name}`}
        className="flex items-center rounded-full outline-none focus-visible:ring-2 focus-visible:ring-ring"
      >
        <ProfileAvatar name={p.name} colour={p.colour} guest={guest} size={26} />
      </DropdownMenu.Trigger>
      <DropdownMenu.Portal>
        <DropdownMenu.Content align="end" sideOffset={6} className={menuContent}>
          <div className="flex items-center gap-2.5 px-2 py-2">
            <ProfileAvatar name={p.name} colour={p.colour} guest={guest} size={32} />
            <div className="flex min-w-0 flex-col">
              <span className="truncate font-medium">{p.name}</span>
              <span className="text-xs text-muted-foreground">
                {guest ? "Nothing you do is kept" : KIND_LABELS[p.kind]}
              </span>
            </div>
          </div>
          <div className={menuSeparator} />
          <DropdownMenu.Item className={menuItem} onSelect={onOpenSettings}>
            <Settings /> Settings
            <MenuShortcut>{displayKeys("Mod+,", platform).join("")}</MenuShortcut>
          </DropdownMenu.Item>
          <DropdownMenu.Item className={menuItem} onSelect={onSwitch}>
            {guest ? <LogOut /> : p.hasPin ? <Lock /> : <Users />}
            {guest ? "Leave and forget this visit" : p.hasPin ? "Lock" : "Switch profile"}
            {lockShortcut && (
              <MenuShortcut>{displayKeys(lockShortcut, platform).join("")}</MenuShortcut>
            )}
          </DropdownMenu.Item>
        </DropdownMenu.Content>
      </DropdownMenu.Portal>
    </DropdownMenu.Root>
  );
}
