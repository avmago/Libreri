import { useDetailsDialog } from "@/features/details";
import { useHelperDialog } from "@/features/helpers";
import { useLibraryDialogs, useLibraryView } from "@/features/library";
import { usePortability } from "@/features/portability";
import { useOcrDialog } from "@/features/search";
import { useStudy, useTimer } from "@/features/study";
import type { StoreApi } from "zustand";
import { useTabs } from "@/lib/tabs";
import { useUi } from "./ui-store";

/**
 * Puts every store that shows one profile's things back as it was at start:
 * open tabs, where the library is, the selection, open dialogs. Called before
 * a profile's app first renders, so nothing of the previous profile shows
 * (not even for one frame); its tabs are then restored from its session.
 */
export function resetProfileState() {
  reset(useTabs);
  reset(useLibraryView);
  reset(useLibraryDialogs);
  reset(usePortability);
  reset(useDetailsDialog);
  reset(useOcrDialog);
  reset(useHelperDialog);
  reset(useTimer);
  reset(useStudy);
  const ui = useUi.getState();
  ui.closeSettings();
  ui.setPaletteOpen(false);
  ui.setShortcutsOpen(false);
}

/** Back to the state the store was created with (replacing, not merging). */
function reset<T>(store: StoreApi<T>) {
  store.setState(store.getInitialState(), true);
}
