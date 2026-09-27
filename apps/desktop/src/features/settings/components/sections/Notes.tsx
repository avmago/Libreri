import { FolderOpen } from "lucide-react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { useProfilePrefs } from "@/features/profiles";
import { commands, type HighlightColor, type LibrarySummary, type SessionDto } from "@/lib/ipc";
import { cn } from "@/lib/utils";
import { HIGHLIGHT_COLORS } from "@/readers";
import { Group, Row, Switch } from "../parts";

const SWATCH: Record<HighlightColor, string> = {
  yellow: "#facc15",
  green: "#4ade80",
  blue: "#60a5fa",
  pink: "#f472b6",
};

export function NotesSettings({
  session,
  library,
}: {
  session: SessionDto;
  library: LibrarySummary;
}) {
  const prefs = useProfilePrefs((s) => s.prefs.notes);
  const update = useProfilePrefs((s) => s.update);
  return (
    <>
      <Group title="Highlights" scope="yours">
        <Row
          label="Colour for new highlights"
          help="Keys 1–4 in the highlight menu pick another colour."
        >
          <div className="flex gap-2" role="radiogroup" aria-label="Colour for new highlights">
            {HIGHLIGHT_COLORS.map((c) => (
              <button
                key={c}
                type="button"
                role="radio"
                aria-checked={prefs.defaultColor === c}
                aria-label={c}
                title={c}
                onClick={() => update({ notes: { defaultColor: c } })}
                className={cn(
                  "size-6 rounded-full border border-black/10",
                  prefs.defaultColor === c &&
                    "ring-2 ring-ring ring-offset-2 ring-offset-background",
                )}
                style={{ background: SWATCH[c] }}
              />
            ))}
          </div>
        </Row>
        <Row
          label="Link quotes back to the page"
          help="Quotes added to a notebook end with a link that opens the book there."
        >
          <Switch
            label="Link quotes back to the page"
            checked={prefs.linkQuotes}
            onChange={(v) => update({ notes: { linkQuotes: v } })}
          />
        </Row>
      </Group>
      <Group
        title="Notebooks"
        scope="yours"
        description="Notebooks are ordinary Markdown files, so any editor (Obsidian, VS Code…) can open them."
      >
        <Row
          label="Your notes folder"
          help={
            <span className="font-mono text-[11.5px]">
              {library.path}/Notes/{session.profile.name}/
            </span>
          }
        >
          <Button
            variant="outline"
            size="sm"
            disabled={!session.keepsData}
            onClick={() =>
              void commands.revealNotesFolder().then((r) => {
                if (r.status === "error") toast.error(r.error.message);
              })
            }
          >
            <FolderOpen /> Open folder
          </Button>
        </Row>
      </Group>
    </>
  );
}
