import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { useSaveCollection } from "../api";
import { useLibraryDialogs } from "../dialogs";
import { buildQuery, navTitle, useLibraryView } from "../store";

/** Saves what the library shows now (search, filters, place) as a smart collection. */
export function SaveCollectionDialog() {
  const open = useLibraryDialogs((s) => s.saveCollection);
  const setOpen = useLibraryDialogs((s) => s.setSaveCollection);
  return (
    <Dialog
      open={open}
      onOpenChange={setOpen}
      title="Save as a smart collection"
      description="The collection keeps this search and these filters, and always shows the books that match now."
    >
      {open && <Form onDone={() => setOpen(false)} />}
    </Dialog>
  );
}

function Form({ onDone }: { onDone: () => void }) {
  const view = useLibraryView.getState();
  const [name, setName] = useState(() => view.search.trim() || navTitle(view.nav));
  const save = useSaveCollection();
  const setNav = useLibraryView((s) => s.setNav);
  return (
    <form
      className="flex flex-col gap-3"
      onSubmit={(e) => {
        e.preventDefault();
        const query = buildQuery(view);
        save.mutate(
          { name, query },
          {
            onSuccess: (c) => {
              toast.success(`Saved “${c.name}”`);
              view.clearFilters();
              setNav({ kind: "collection", id: c.id, name: c.name, query: c.query });
              onDone();
            },
          },
        );
      }}
    >
      <label className="flex flex-col gap-1.5">
        <span className="font-medium">Name</span>
        <Input value={name} onChange={(e) => setName(e.target.value)} autoFocus />
      </label>
      {save.error && <p className="text-destructive">{save.error.message}</p>}
      <div className="flex justify-end gap-2">
        <Button type="button" variant="ghost" onClick={onDone}>
          Cancel
        </Button>
        <Button type="submit" disabled={!name.trim() || save.isPending}>
          Save
        </Button>
      </div>
    </form>
  );
}
