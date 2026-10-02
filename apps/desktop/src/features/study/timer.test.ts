import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("@/lib/ipc", () => ({
  commands: {
    studyRead: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    studyWrite: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
  },
  unwrap: async (p: Promise<{ status: string; data?: unknown; error?: unknown }>) => {
    const r = await p;
    if (r.status === "ok") return r.data;
    throw r.error;
  },
}));

const { useStudy, useReading } = await import("./store");
const { useTimer } = await import("./timer");

describe("study timer", () => {
  beforeEach(async () => {
    vi.useFakeTimers();
    vi.setSystemTime(new Date("2026-10-02T09:00:00"));
    useTimer.setState(useTimer.getInitialState(), true);
    useStudy.setState(useStudy.getInitialState(), true);
    await useStudy.getState().load();
    useReading
      .getState()
      .set({ bookId: "b", title: "The Lighthouse", page: 84, pages: 300, progress: 0.28 });
  });
  afterEach(() => vi.useRealTimers());

  it("runs a focus round, records it, and waits on the break card", async () => {
    const t = useTimer.getState();
    t.start();
    expect(useTimer.getState()).toMatchObject({ status: "running", phase: "work", round: 1 });
    vi.advanceTimersByTime(25 * 60_000);
    useReading
      .getState()
      .set({ bookId: "b", title: "The Lighthouse", page: 97, pages: 300, progress: 0.32 });
    useTimer.getState().tick();
    await vi.runAllTimersAsync();
    const s = useTimer.getState();
    expect(s.card).toMatchObject({ minutes: 25, pageFrom: 84, pageTo: 97, next: "break" });
    expect(s).toMatchObject({ status: "paused", phase: "break" });
    expect(useStudy.getState().data.sessions).toHaveLength(1);
    expect(useStudy.getState().data.sessions[0]).toMatchObject({ bookId: "b", minutes: 25 });
    s.startBreak();
    expect(useTimer.getState()).toMatchObject({ status: "running", phase: "break", card: null });
    vi.advanceTimersByTime(5 * 60_000);
    useTimer.getState().tick();
    expect(useTimer.getState()).toMatchObject({ status: "paused", phase: "work", round: 2 });
  });

  it("pauses and resumes a countdown where it was", () => {
    useTimer.getState().setMode("timer");
    useTimer.getState().start();
    vi.advanceTimersByTime(4 * 60_000);
    useTimer.getState().pause();
    expect(useTimer.getState().heldMs).toBe(6 * 60_000);
    vi.advanceTimersByTime(60 * 60_000);
    useTimer.getState().resume();
    vi.advanceTimersByTime(6 * 60_000);
    useTimer.getState().tick();
    expect(useTimer.getState().status).toBe("idle");
  });

  it("saves a stopwatch as reading when stopped", async () => {
    useTimer.getState().setMode("stopwatch");
    useTimer.getState().start();
    vi.advanceTimersByTime(12 * 60_000);
    useTimer.getState().stop();
    await vi.runAllTimersAsync();
    expect(useStudy.getState().data.sessions[0]).toMatchObject({ kind: "stopwatch", minutes: 12 });
  });

  it("skipping a focus stretch keeps what was read and starts the break", async () => {
    useTimer.getState().start();
    vi.advanceTimersByTime(10 * 60_000);
    useTimer.getState().skip();
    await vi.runAllTimersAsync();
    expect(useTimer.getState()).toMatchObject({ status: "running", phase: "break" });
    expect(useStudy.getState().data.sessions[0]?.minutes).toBe(10);
  });
});
