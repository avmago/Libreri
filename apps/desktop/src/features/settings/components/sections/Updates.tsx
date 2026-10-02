import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { commands } from "@/lib/ipc";
import { useUpdates } from "@/features/updates";
import { Group, Row, Switch } from "../parts";

/** Settings › General › Updates. */
export function UpdatesGroup() {
  const qc = useQueryClient();
  const status = useQuery({ queryKey: ["update-status"], queryFn: () => commands.updateStatus() });
  const u = useUpdates();
  const s = status.data;
  if (!s) return null;
  const pct =
    u.installing && u.installing.total
      ? Math.round((u.installing.done / u.installing.total) * 100)
      : null;
  return (
    <Group title="Updates" scope="computer">
      <Row
        label={`Libreri ${s.current}`}
        help={
          !s.available
            ? "This build cannot update itself (it was not made as a release)."
            : u.found
              ? `Version ${u.found.version} is ready to install. Your notes and places are saved first, then Libreri restarts.`
              : u.checked?.error
                ? `Could not look: ${u.checked.error}`
                : u.checked
                  ? "This is the newest version."
                  : "Updates come from Libreri's releases on GitHub and are checked against its signature before they are installed."
        }
      >
        {u.found ? (
          <Button size="sm" disabled={!!u.installing} onClick={() => void u.install()}>
            {u.installing ? (
              <>
                <Loader2 className="animate-spin" />
                {pct !== null ? `Downloading ${pct}%` : "Downloading…"}
              </>
            ) : (
              "Install and restart"
            )}
          </Button>
        ) : (
          <Button
            size="sm"
            variant="outline"
            disabled={!s.available || u.checking}
            onClick={() => void u.check()}
          >
            {u.checking && <Loader2 className="animate-spin" />}
            Check now
          </Button>
        )}
      </Row>
      <Row
        label="Look for updates when Libreri starts"
        help="Only asks GitHub whether there is a newer version; nothing about you or your library is sent."
      >
        <Switch
          label="Look for updates when Libreri starts"
          checked={s.checkOnStart}
          onChange={(v) =>
            void commands
              .setUpdateCheck(v)
              .then(() => qc.invalidateQueries({ queryKey: ["update-status"] }))
          }
        />
      </Row>
    </Group>
  );
}
