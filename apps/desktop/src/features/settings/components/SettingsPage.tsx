import { useEffect } from "react";
import {
  ArrowLeft,
  BookOpen,
  HardDrive,
  Keyboard,
  NotebookPen,
  Palette,
  Settings2,
  ShieldCheck,
  type LucideIcon,
} from "lucide-react";
import { Button } from "@/components/ui/button";
import type { LibrarySummary, SessionDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { AppearanceSettings } from "./sections/Appearance";
import { GeneralSettings } from "./sections/General";
import { LibrarySettings } from "./sections/LibraryStorage";
import { NotesSettings } from "./sections/Notes";
import { ProfileSettings } from "./sections/ProfilesSecurity";
import { ReaderSettings } from "./sections/Reader";
import { ShortcutSettings } from "./sections/Shortcuts";

export type SettingsSection =
  "general" | "library" | "profiles" | "appearance" | "reader" | "notes" | "shortcuts";

const SECTIONS: { id: SettingsSection; label: string; Icon: LucideIcon }[] = [
  { id: "general", label: "General", Icon: Settings2 },
  { id: "library", label: "Library & storage", Icon: HardDrive },
  { id: "profiles", label: "Profiles & security", Icon: ShieldCheck },
  { id: "appearance", label: "Appearance", Icon: Palette },
  { id: "reader", label: "Reader", Icon: BookOpen },
  { id: "notes", label: "Notes", Icon: NotebookPen },
  { id: "shortcuts", label: "Shortcuts", Icon: Keyboard },
];

/** Boards 16–23: Settings, one page per section. */
export function SettingsPage({
  library,
  session,
  section,
  onSection,
  onClose,
}: {
  library: LibrarySummary;
  session: SessionDto;
  section: SettingsSection;
  onSection: (s: SettingsSection) => void;
  onClose: () => void;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape" || e.defaultPrevented) return;
      const t = e.target as HTMLElement | null;
      if (t?.closest("[role=dialog], [data-recording]")) return;
      onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const current = SECTIONS.find((s) => s.id === section) ?? SECTIONS[0]!;

  return (
    <div className="flex h-full">
      <nav
        aria-label="Settings"
        className="flex w-60 shrink-0 flex-col gap-1 border-r bg-sidebar p-2.5"
      >
        <Button variant="ghost" size="sm" className="mb-2 justify-start" onClick={onClose}>
          <ArrowLeft /> Back
        </Button>
        <h1 className="px-2.5 pb-2 text-[20px] font-semibold tracking-tight">Settings</h1>
        {SECTIONS.map(({ id, label, Icon }) => (
          <button
            key={id}
            type="button"
            aria-current={id === section ? "page" : undefined}
            onClick={() => onSection(id)}
            className={cn(
              "flex h-8 items-center gap-2.5 rounded-md px-2.5 text-left",
              id === section ? "bg-muted font-medium" : "hover:bg-muted/60",
            )}
          >
            <Icon className="size-4 text-muted-foreground" aria-hidden />
            {label}
          </button>
        ))}
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto" aria-label={current.label}>
        <div className="mx-auto flex max-w-3xl flex-col gap-8 px-8 pt-8 pb-16">
          <h1 className="text-[20px] font-semibold tracking-tight">{current.label}</h1>
          {section === "general" && <GeneralSettings session={session} />}
          {section === "library" && <LibrarySettings library={library} session={session} />}
          {section === "profiles" && <ProfileSettings session={session} />}
          {section === "appearance" && <AppearanceSettings />}
          {section === "reader" && <ReaderSettings />}
          {section === "notes" && <NotesSettings session={session} library={library} />}
          {section === "shortcuts" && <ShortcutSettings />}
        </div>
      </main>
    </div>
  );
}
