import { Button } from "@/components/ui/button";
import { NativeSelect } from "@/components/ui/input";
import { useProfilePrefs } from "@/features/profiles";
import type { SessionDto } from "@/lib/ipc";
import { Group, Row, Switch } from "../parts";

const LOCK_OPTIONS = [0, 1, 5, 15, 30, 60];

export function GeneralSettings({
  session,
  onSetPin,
}: {
  session: SessionDto;
  /** Opens Profiles & security, where a PIN is set. */
  onSetPin: () => void;
}) {
  const prefs = useProfilePrefs((s) => s.prefs);
  const update = useProfilePrefs((s) => s.update);
  return (
    <>
      <Group title="When you step away" scope="yours">
        <Row
          label="Lock after"
          htmlFor={session.profile.hasPin ? "autolock" : undefined}
          help={
            session.profile.hasPin
              ? "Libreri goes back to the profile picker after this long without use."
              : "Locking needs a PIN, so that only you can open your profile again. Set one first, then choose how long to wait."
          }
        >
          {session.profile.hasPin ? (
            <NativeSelect
              id="autolock"
              value={prefs.autoLockMinutes}
              onChange={(e) => update({ autoLockMinutes: Number(e.target.value) })}
              className="w-40"
            >
              {LOCK_OPTIONS.map((m) => (
                <option key={m} value={m}>
                  {m === 0 ? "Never" : m === 1 ? "1 minute" : `${m} minutes`}
                </option>
              ))}
            </NativeSelect>
          ) : (
            <Button variant="outline" size="sm" onClick={onSetPin}>
              Set a PIN…
            </Button>
          )}
        </Row>
      </Group>
      <Group title="Library" scope="yours">
        <Row
          label="Ask before moving books to the Trash"
          help="Books in the system Trash can be restored with their details."
        >
          <Switch
            label="Ask before moving books to the Trash"
            checked={prefs.library.confirmTrash}
            onChange={(v) => update({ library: { confirmTrash: v } })}
          />
        </Row>
        <Row label="Open books where you stopped" help="Otherwise books open at the start.">
          <Switch
            label="Open books where you stopped"
            checked={prefs.reader.resume}
            onChange={(v) => update({ reader: { resume: v } })}
          />
        </Row>
      </Group>
    </>
  );
}
