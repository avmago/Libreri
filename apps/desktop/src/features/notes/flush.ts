/** Saves of open notes still waiting, run early by `flushNotes`. */
const flushers = new Set<() => Promise<void>>();

/** Registers an open note's save; returns the unregister function. */
export function registerNoteFlush(flush: () => Promise<void>): () => void {
  flushers.add(flush);
  return () => void flushers.delete(flush);
}

/** Writes every note typed and not saved yet (before locking or closing). */
export async function flushNotes(): Promise<void> {
  await Promise.allSettled([...flushers].map((f) => f()));
}
