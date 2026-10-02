import { describe, expect, it } from "vitest";
import {
  addDays,
  byDay,
  clock,
  duration,
  monthGrid,
  monthTotals,
  noteReached,
  pace,
  parseStudy,
  streak,
  toIcs,
  weekOf,
  type Goal,
  type StudySession,
} from "./model";

const S = (start: string, minutes: number, from?: number, to?: number): StudySession => ({
  id: start,
  start: new Date(start).toISOString(),
  minutes,
  kind: "focus",
  title: "The Lighthouse",
  pageFrom: from ?? null,
  pageTo: to ?? null,
});

describe("study model", () => {
  it("reads old or broken documents with defaults", () => {
    expect(parseStudy(null).focus.work).toBe(25);
    expect(parseStudy("not json").sessions).toEqual([]);
    const d = parseStudy('{"focus":{"work":50},"options":{"askTakeaway":true}}');
    expect(d.focus).toEqual({ work: 50, brk: 5, long: 15, rounds: 4 });
    expect(d.options.askTakeaway).toBe(true);
    expect(d.options.countReading).toBe(true);
  });

  it("sums days, months and the streak", () => {
    const days = byDay([
      S("2026-10-01T09:00:00", 25, 10, 20),
      S("2026-10-01T10:00:00", 25, 20, 32),
      S("2026-10-02T09:00:00", 40),
      S("2026-09-30T21:00:00", 15),
    ]);
    expect(days.get("2026-10-01")).toMatchObject({ minutes: 50, pages: 22 });
    expect(monthTotals(days, "2026-10")).toEqual({ minutes: 90, pages: 22 });
    expect(streak(days, "2026-10-02")).toBe(3);
    // Nothing yet today: the streak up to yesterday still counts.
    expect(streak(days, "2026-10-03")).toBe(3);
    expect(streak(days, "2026-10-05")).toBe(0);
  });

  it("lays out months and weeks from Monday", () => {
    const g = monthGrid(2026, 9); // October 2026 starts on a Thursday
    expect(g[0]![0]).toBe("2026-09-28");
    expect(g.every((w) => w.length === 7)).toBe(true);
    expect(g.flat()).toContain("2026-10-31");
    expect(weekOf("2026-10-04")[0]).toBe("2026-09-28");
    expect(addDays("2026-12-31", 1)).toBe("2027-01-01");
  });

  it("works out a goal's pace", () => {
    const g: Goal = {
      id: "g",
      kind: "finish",
      title: "The Lighthouse",
      bookId: "b",
      due: "2026-10-11",
      pages: 300,
      startPage: 100,
      reached: 150,
      created: new Date("2026-10-01T08:00:00").toISOString(),
    };
    const p = pace(g, "2026-10-02");
    expect(p.left).toBe(150);
    expect(p.perDay).toBe(15); // 10 days including today
    expect(p.progress).toBeCloseTo(0.25);
    expect(p.behind).toBe(false);
    expect(pace({ ...g, reached: 100 }, "2026-10-08").behind).toBe(true);
    expect(pace(g, "2026-10-12").overdue).toBe(true);
    const due: Goal = { ...g, kind: "due", pages: null };
    expect(pace(due, "2026-10-11").daysLeft).toBe(0);
  });

  it("moves goals on as the book is read", () => {
    const g: Goal = {
      id: "g",
      kind: "finish",
      title: "T",
      bookId: "b",
      due: "2026-10-11",
      pages: 200,
      reached: 50,
      created: "",
    };
    expect(noteReached([g], "b", 40)).toBeNull();
    expect(noteReached([g], "other", 90)).toBeNull();
    expect(noteReached([g], "b", 90)![0]!.reached).toBe(90);
    expect(noteReached([g], "b", 200)![0]!.done).toBe(true);
    // A book without pages: by how far through it is.
    expect(noteReached([g], "b", null, 0.5)![0]!.reached).toBe(100);
  });

  it("exports an .ics calendar", () => {
    const data = parseStudy(null);
    data.goals.push({
      id: "g1",
      kind: "due",
      title: "Chapter 4; seminar",
      due: "2026-10-06",
      created: "",
    });
    data.sessions.push(S("2026-10-01T09:00:00", 25, 10, 20));
    const ics = toIcs(data, new Date("2026-10-02T00:00:00Z"));
    expect(ics.startsWith("BEGIN:VCALENDAR\r\n")).toBe(true);
    expect(ics).toContain("DTSTART;VALUE=DATE:20261006");
    expect(ics).toContain("DTEND;VALUE=DATE:20261007");
    expect(ics).toContain("SUMMARY:Due: Chapter 4\\; seminar");
    expect(ics).toContain("DESCRIPTION:10 pages");
    expect(ics.trimEnd().endsWith("END:VCALENDAR")).toBe(true);
  });

  it("formats times", () => {
    expect(clock(932_000)).toBe("15:32");
    expect(clock(3_723_000)).toBe("1:02:03");
    expect(duration(50)).toBe("50 min");
    expect(duration(580)).toBe("9 h 40");
    expect(duration(120)).toBe("2 h");
  });
});
