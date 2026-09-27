import { useMemo, useState } from "react";
import { Printer, Search } from "lucide-react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Kbd } from "@/components/ui/kbd";
import {
  ACTION_IDS,
  ACTIONS,
  GROUP_ORDER,
  VIM_KEYS,
  useShortcutBindings,
  type ActionId,
} from "@/lib/shortcuts";

/** The Ctrl+/ cheat sheet: every shortcut in use, printable. */
export function ShortcutsSheet({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { overrides, vim } = useShortcutBindings();
  const [filter, setFilter] = useState("");
  const keysOf = (id: ActionId) => (id in overrides ? (overrides[id] ?? null) : ACTIONS[id].keys);
  const q = filter.trim().toLowerCase();
  const groups = useMemo(
    () =>
      GROUP_ORDER.map((g) => ({
        group: g,
        ids: ACTION_IDS.filter(
          (id) =>
            ACTIONS[id].group === g &&
            keysOf(id) &&
            (!q || ACTIONS[id].label.toLowerCase().includes(q)),
        ),
      })).filter((g) => g.ids.length),
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [q, overrides],
  );

  return (
    <Dialog
      open={open}
      onOpenChange={onOpenChange}
      title="Keyboard shortcuts"
      description="Change them in Settings › Shortcuts."
      className="lb-print-sheet w-[880px]"
    >
      <div className="flex items-center gap-2 print:hidden">
        <div className="relative flex-1">
          <Search
            className="pointer-events-none absolute top-1/2 left-2.5 size-3.5 -translate-y-1/2 text-muted-foreground"
            aria-hidden
          />
          <Input
            value={filter}
            onChange={(e) => setFilter(e.target.value)}
            placeholder="Search"
            aria-label="Search shortcuts"
            className="pl-8"
            autoFocus
          />
        </div>
        <Button variant="outline" size="sm" onClick={() => window.print()}>
          <Printer /> Print
        </Button>
      </div>
      <div className="columns-2 gap-8 print:columns-3">
        {groups.map(({ group, ids }) => (
          <section key={group} className="mb-5 break-inside-avoid" aria-label={group}>
            <h3 className="mb-1.5 text-[12px] font-semibold tracking-wide text-muted-foreground uppercase">
              {group}
            </h3>
            <dl className="flex flex-col">
              {ids.map((id) => (
                <div key={id} className="flex items-center justify-between gap-3 py-1">
                  <dt className="truncate text-[12.5px]">{ACTIONS[id].label}</dt>
                  <dd>
                    <Kbd shortcut={keysOf(id)} />
                  </dd>
                </div>
              ))}
            </dl>
          </section>
        ))}
        {vim && (
          <section className="mb-5 break-inside-avoid" aria-label="Vim keys">
            <h3 className="mb-1.5 text-[12px] font-semibold tracking-wide text-muted-foreground uppercase">
              Vim keys
            </h3>
            <dl className="flex flex-col">
              {VIM_KEYS.map(([id, keys]) => (
                <div key={`${id}-${keys}`} className="flex items-center justify-between gap-3 py-1">
                  <dt className="truncate text-[12.5px]">{ACTIONS[id].label}</dt>
                  <dd>
                    <Kbd shortcut={keys} />
                  </dd>
                </div>
              ))}
            </dl>
          </section>
        )}
      </div>
    </Dialog>
  );
}
