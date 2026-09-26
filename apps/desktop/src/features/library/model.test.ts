import { describe, expect, it } from "vitest";
import { authorsText, coverTone, fileName, formatSize, toView } from "./model";
import type { BookDto } from "@/lib/ipc";

describe("model helpers", () => {
  it("formats sizes and names", () => {
    expect(formatSize(512)).toBe("512 B");
    expect(formatSize(1536)).toBe("1.5 KB");
    expect(formatSize(250 * 1024 * 1024)).toBe("250 MB");
    expect(fileName("Books/Sci/a b.pdf")).toBe("a b.pdf");
    expect(authorsText([])).toBe("Unknown author");
    expect(authorsText(["A", "B", "C"], 2)).toBe("A, B +1");
  });

  it("gives every book the same generated cover every time", () => {
    expect(coverTone("abc")).toEqual(coverTone("abc"));
  });

  it("fills in missing optional fields", () => {
    const dto = {
      id: "x",
      relPath: "Books/x.pdf",
      fileType: "pdf",
      fileSize: 1,
      hasCover: false,
      missing: false,
      addedAt: "",
      modifiedAt: "",
      folder: "",
      thumbnail: null,
      cover: null,
      metadata: { title: "T" },
      user: {},
    } as BookDto;
    const v = toView(dto);
    expect(v.metadata.authors).toEqual([]);
    expect(v.user.status).toBe("none");
  });
});
