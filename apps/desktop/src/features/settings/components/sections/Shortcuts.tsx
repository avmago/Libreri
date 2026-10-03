import { useEffect, useMemo, useState } from "react";
import { RotateCcw, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import { useProfilePrefs } from "@/features/profiles";
import {
  ACTION_IDS,
  ACTIONS,
  DEFAULT_SHORTCUTS,
  GROUP_ORDER,
  findConflicts,
  platform,
  shortcutFromEvent,
  type ActionId,
} from "@/lib/shortcuts";
import { cn } from "@/lib/utils";
import { Group, Row, Switch } from "../parts";

/** Every action, its keys, and changing them. */
export function ShortcutSettings() {
  const prefs = useProfilePrefs((s) => s.prefs);
  const update = useProfilePrefs((s) => s.update);
  const [filter, setFilter] = useState("");
  const [recording, setRecording] = useState<ActionId | null>(null);
  const [clash, setClash] = useState<{ id: ActionId; keys: string; with: ActionId[] } | null>(null);
  const overrides = prefs.shortcuts;
  const keysOf = (id: ActionId): string | null =>
    id in overrides ? (overrides[id] ?? null) : DEFAULT_SHORTCUTS[id];
  const changed = Object.keys(overrides).length;

  const assign = (id: ActionId, keys: string | null, clear: ActionId[] = []) => {
    const next: Record<string, string | null> = { ...overrides, [id]: keys };
    if (keys === DEFAULT_SHORTCUTS[id]) delete next[id];
    for (const other of clear) next[other] = null;
    update({ shortcuts: next });
  };

  // Record the next key press for the chosen action.
  useEffect(() => {
    if (!recording) return;
    const onKey = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopImmediatePropagation();
      if (e.key === "Escape") return setRecording(null);
      const keys = shortcutFromEvent(e, platform);
      if (!keys) return;
      setRecording(null);
      const others = findConflicts(recording, keys, keysOf, platform);
      if (others.length) setClash({ id: recording, keys, with: others });
      else assign(recording, keys);
    };
    window.addEventListener("keydown", onKey, { capture: true });
    return () => window.removeEventListener("keydown", onKey, { capture: true });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [recording]);

  const q = filter.trim().toLowerCase();
  const groups = useMemo(
    () =>
      GROUP_ORDER.map((g) => ({
        group: g,
        ids: ACTION_IDS.filter(
          (id) =>
            ACTIONS[id].group === g &&
            (!q ||
              ACTIONS[id].label.toLowerCase().includes(q) ||
              (keysOf(id) ?? "").toLowerCase().includes(q)),
        ),
      })).filter((g) => g.ids.length),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [q, overrides],
  );

  return (
    <>
      <Group title="Keyboard" scope="yours">
        <Row
          label="Vim-style keys"
          help="Adds j/k to move, h/l for pages, gg and G for start and end, / to search, n/N for the next and previous match."
        >
          <Switch
            label="Vim-style keys"
            checked={prefs.vimKeys}
            onChange={(v) => update({ vimKeys: v })}
          />
        </Row>
      </Group>

      <section className="flex flex-col gap-3" aria-label="All shortcuts">
        <div className="flex items-center gap-3">
          <div className="relative flex-1">
            <Search
              className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
              aria-hidden
            />
            <Input
              value={filter}
              onChange={(e) => setFilter(e.target.value)}
              placeholder={`Search ${ACTION_IDS.length} actions…`}
              aria-label="Search shortcuts"
              className="pl-8"
            />
          </div>
          <Button
            variant="outline"
            size="sm"
            disabled={!changed}
            onClick={() => update({ shortcuts: {} })}
          >
            <RotateCcw /> Reset all{changed ? ` (${changed})` : ""}
          </Button>
        </div>
        <p className="text-[12.5px] text-muted-foreground">
          Click a shortcut, then press the new keys. Esc cancels. The same keys can do different
          things in the library and the reader.
        </p>

        {clash && (
          <div
            role="alert"
            className="flex items-center gap-3 rounded-lg border border-amber-500/50 bg-amber-500/10 px-3 py-2.5"
          >
            <span className="flex-1">
              <Kbd shortcut={clash.keys} className="mr-1.5 align-middle" /> already does{" "}
              {clash.with.map((w) => `“${ACTIONS[w].label}”`).join(", ")}. Use it for “
              {ACTIONS[clash.id].label}” instead?
            </span>
            <Button
              size="sm"
              onClick={() => {
                assign(clash.id, clash.keys, clash.with);
                setClash(null);
              }}
            >
              Use here
            </Button>
            <Button size="sm" variant="ghost" onClick={() => setClash(null)}>
              Cancel
            </Button>
          </div>
        )}

        {groups.map(({ group, ids }) => (
          <div key={group} className="flex flex-col gap-1">
            <h2 className="pt-3 pb-1 text-[13px] font-semibold text-muted-foreground">{group}</h2>
            <div className="flex flex-col divide-y rounded-lg border bg-background">
              {ids.map((id) => {
                const keys = keysOf(id);
                const isChanged = id in overrides;
                return (
                  <div key={id} className="flex h-10 items-center gap-3 px-4">
                    <span className="flex-1 truncate">{ACTIONS[id].label}</span>
                    {isChanged && (
                      <button
                        type="button"
                        className="text-[12px] text-muted-foreground underline-offset-2 hover:underline"
                        onClick={() => assign(id, DEFAULT_SHORTCUTS[id])}
                        title={
                          DEFAULT_SHORTCUTS[id]
                            ? `Default: ${DEFAULT_SHORTCUTS[id]}`
                            : "Default: none"
                        }
                      >
                        Reset
                      </button>
                    )}
                    <button
                      type="button"
                      data-recording={recording === id ? "" : undefined}
                      onClick={() => setRecording(recording === id ? null : id)}
                      aria-label={`${ACTIONS[id].label}: ${keys ?? "no shortcut"}. Change`}
                      className={cn(
                        "flex h-7 min-w-28 items-center justify-end rounded-md border border-transparent px-2 hover:border-border",
                        recording === id && "border-primary ring-2 ring-primary/30",
                      )}
                    >
                      {recording === id ? (
                        <span className="text-[12px] text-muted-foreground">Press keys…</span>
                      ) : keys ? (
                        <Kbd shortcut={keys} />
                      ) : (
                        <span className="text-[12px] text-muted-foreground">None</span>
                      )}
                    </button>
                    {keys && (
                      <button
                        type="button"
                        className="text-[12px] text-muted-foreground hover:text-foreground"
                        aria-label={`Remove the shortcut for ${ACTIONS[id].label}`}
                        title="Remove"
                        onClick={() => assign(id, null)}
                      >
                        ×
                      </button>
                    )}
                  </div>
                );
              })}
            </div>
          </div>
        ))}
      </section>
    </>
  );
}
