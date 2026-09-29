import { beforeEach, describe, expect, it } from "vitest";
import { useTabs } from "./index";

const tab = (bookId: string) => ({ bookId, title: bookId, fileType: "pdf" as const });

describe("closeGone", () => {
  beforeEach(() => useTabs.setState(useTabs.getInitialState(), true));

  it("closes the tabs of trashed books, without offering to reopen them", () => {
    const t = useTabs.getState();
    t.open(tab("a"));
    t.open(tab("b"));
    t.open(tab("c"));
    t.close("c");
    useTabs.getState().closeGone(["b", "c", "x"]);
    const s = useTabs.getState();
    expect(s.tabs.map((x) => x.bookId)).toEqual(["a"]);
    expect(s.active).toBe("a");
    expect(s.closed).toEqual([]);
  });
});
