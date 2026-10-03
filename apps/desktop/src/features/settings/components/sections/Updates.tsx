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

const REPO = "https://github.com/avmg0/Libreri";

/** Settings › General › About: the licence and where the source is. */
export function AboutGroup() {
  const open = (url: string) => void commands.openExternalUrl(url);
  return (
    <Group title="About Libreri" scope="computer">
      <Row
        label="Free software"
        help="Libreri is under the GNU General Public License, version 3 or later: you may use, study, change and share it. It comes with no warranty."
      >
        <Button size="sm" variant="outline" onClick={() => open(`${REPO}/blob/main/LICENSE`)}>
          Licence
        </Button>
      </Row>
      <Row
        label="Privacy"
        help="Your library never leaves your computer. Libreri goes online only for what you ask for, and sends only what that needs."
      >
        <Button size="sm" variant="outline" onClick={() => open(`${REPO}/blob/main/PRIVACY.md`)}>
          Privacy policy
        </Button>
      </Row>
      <Row
        label="Source code and credits"
        help="The code, and the libraries Libreri is built on with their licences."
      >
        <span className="flex gap-2">
          <Button size="sm" variant="outline" onClick={() => open(REPO)}>
            Source
          </Button>
          <Button
            size="sm"
            variant="outline"
            onClick={() => open(`${REPO}/blob/main/docs/third-party-licenses.md`)}
          >
            Credits
          </Button>
        </span>
      </Row>
    </Group>
  );
}
