import { describe, expect, it } from "vitest";
import { addStreamIteration } from "./polyfills";

describe("ReadableStream async iteration", () => {
  it("reads every chunk with the polyfill", async () => {
    // Its own prototype, so the browser's iteration is not used.
    const s = new ReadableStream({
      start(c) {
        c.enqueue(1);
        c.enqueue(2);
        c.close();
      },
    });
    const own = Object.create(ReadableStream.prototype) as ReadableStream;
    addStreamIteration(own, true);
    Object.setPrototypeOf(s, own);
    const got: unknown[] = [];
    for await (const v of s as unknown as AsyncIterable<unknown>) got.push(v);
    expect(got).toEqual([1, 2]);
  });
});
