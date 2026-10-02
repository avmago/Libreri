/**
 * What older WebKit (the macOS web view) lacks and libraries expect.
 *
 * - Async iteration of a ReadableStream (`for await (const x of stream)`),
 *   which PDF.js uses to get a page's text (read aloud, find): without it
 *   "undefined is not a function".
 */
type Iterable = ReadableStream & {
  [Symbol.asyncIterator]?: () => AsyncIterableIterator<unknown>;
  values?: (o?: { preventCancel?: boolean }) => AsyncIterableIterator<unknown>;
};

/** Adds async iteration to a ReadableStream prototype that lacks it. */
export function addStreamIteration(proto: Iterable, force = false) {
  if (!proto[Symbol.asyncIterator] || force) {
    const values = function (
      this: ReadableStream,
      { preventCancel = false }: { preventCancel?: boolean } = {},
    ): AsyncIterableIterator<unknown> {
      const reader = this.getReader();
      return {
        async next() {
          const r = await reader.read();
          if (r.done) reader.releaseLock();
          return r.done ? { done: true, value: undefined } : { done: false, value: r.value };
        },
        async return(value?: unknown) {
          if (!preventCancel) await reader.cancel(value).catch(() => {});
          reader.releaseLock();
          return { done: true, value };
        },
        [Symbol.asyncIterator]() {
          return this;
        },
      };
    };
    if (force || !proto.values) proto.values = values;
    proto[Symbol.asyncIterator] = values;
  }
}

if (typeof ReadableStream !== "undefined") addStreamIteration(ReadableStream.prototype);
