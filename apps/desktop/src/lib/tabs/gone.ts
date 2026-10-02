/**
 * Books trashed in one window are closed in every window: each window has
 * its own tabs (and saves them in its own session), so the window that
 * trashed them tells the others.
 */
import { emit, listen } from "@tauri-apps/api/event";
import { useTabs } from "./index";

const EVENT = "libreri://books-gone";

/** Closes the books' tabs here and in every other window. */
export function closeGoneEverywhere(bookIds: string[]) {
  useTabs.getState().closeGone(bookIds);
  if (bookIds.length) void emit(EVENT, bookIds).catch(() => {});
}

/** Each window listens (see useLibraryEvents). */
export function listenForGone(): Promise<() => void> {
  return listen<string[]>(EVENT, ({ payload }) => {
    if (Array.isArray(payload)) useTabs.getState().closeGone(payload);
  });
}
