import { describe, expect, it } from "vitest";
import { defaultFileName, formatInfo, safeName, summariseImport } from "./model";

describe("export file names", () => {
  const day = new Date("2026-09-28T10:00:00Z");
  it("names whole-library, single-book and folder exports", () => {
    expect(defaultFileName(formatInfo("xlsx"), "Home", null, day)).toBe("Home 2026-09-28.xlsx");
    expect(defaultFileName(formatInfo("bibtex"), "Home", [{ title: "Optics: 2/e" }], day)).toBe(
      "Optics 2 e.bib",
    );
    expect(defaultFileName(formatInfo("csv"), "Home", [{ title: "a" }, { title: "b" }], day)).toBe(
      "Home – 2 books.csv",
    );
    expect(defaultFileName(formatInfo("obsidian"), "Home", null, day)).toBe(
      "Home 2026-09-28 notes",
    );
  });
  it("never makes an empty name", () => {
    expect(safeName(" ... ")).toBe("Libreri export");
  });
});

describe("archive import report", () => {
  it("says what happened to books and notes", () => {
    const s = summariseImport({
      jobId: "1",
      linked: 248,
      added: 0,
      otherFile: 3,
      missing: 1,
      filesRestored: 0,
      detailsUpdated: 2,
      notesAdded: 12,
      notesUpdated: 1,
      notesKept: 40,
      noteFilesAdded: 0,
      noteConflicts: [],
      profilesCreated: ["Sam"],
      missingBooks: ["x"],
      warnings: [],
    });
    expect(s.title).toBe("Archive imported");
    expect(s.description).toBe(
      "248 books already here · 3 matched another copy · 1 without a file\n12 notes added · 1 updated\nNew profiles: Sam",
    );
  });
});
