import { describe, expect, it } from "vitest";
import { PauseDetector, concat, rms, toPcm16 } from "./pcm";

describe("toPcm16", () => {
  it("turns 48 kHz into 16 kHz 16-bit samples", () => {
    const input = new Float32Array(48_000).fill(0.5);
    const out = toPcm16(input, 48_000);
    expect(out.length).toBe(16_000);
    expect(out[100]).toBe(Math.round(0.5 * 32767));
  });

  it("keeps a 440 Hz tone", () => {
    const input = Float32Array.from({ length: 44_100 }, (_, i) =>
      Math.sin((i / 44_100) * 440 * 2 * Math.PI),
    );
    const out = toPcm16(input, 44_100);
    let crossings = 0;
    for (let i = 1; i < out.length; i++) if (out[i - 1]! < 0 !== out[i]! < 0) crossings++;
    expect(crossings).toBeGreaterThan(860);
    expect(crossings).toBeLessThan(900);
  });

  it("joins and measures buffers", () => {
    const j = concat([new Float32Array([1, 1]), new Float32Array([-1])]);
    expect(Array.from(j)).toEqual([1, 1, -1]);
    expect(rms(j)).toBe(1);
  });
});

describe("PauseDetector", () => {
  const step = 0.1;
  it("sends after speech and a pause", () => {
    const d = new PauseDetector();
    const said: (string | null)[] = [];
    for (let i = 0; i < 5; i++) said.push(d.push(0.002, step)); // the room
    for (let i = 0; i < 15; i++) said.push(d.push(0.1, step)); // speech
    for (let i = 0; i < 8; i++) said.push(d.push(0.002, step)); // pause
    expect(said.filter(Boolean)).toEqual(["send"]);
    expect(said.indexOf("send")).toBe(26);
  });

  it("drops long silence and cuts long speech", () => {
    const d = new PauseDetector();
    const quiet = Array.from({ length: 31 }, () => d.push(0.001, step));
    expect(quiet.filter(Boolean)).toEqual(["drop"]);
    const talk = Array.from({ length: 260 }, () => d.push(0.2, step));
    expect(talk.filter(Boolean)).toEqual(["send"]);
  });
});
