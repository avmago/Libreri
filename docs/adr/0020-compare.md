# 20. Comparing documents

Status: accepted.

Libreri finds what changed between two documents.

## Decisions

- **What:** versions of a book (an earlier version with the current file), two books in the library, or a book and a PDF or DjVu file on this computer.
- **How:** text and looks. Words that were removed, added or changed are marked, and so are pages whose drawings or pictures look different. Pages are paired even when some were added or removed.
- **Views:** side by side, one page over the other (a slider, or only the differences), and a list of changes like tracked changes.
- **Result:** shown in the app, and it can be saved as a report PDF to send to someone.

## How it works

- **Words.** Each side's words and boxes come from the PDF (hayro, the same glyph reader as search) or from DjVu's text layer. For pages without text, saved OCR words are used when the file is a book's current file.
- **Pairing pages.** A first diff over all the words of both documents (Patience, with a time limit) shows which pages share unchanged words. The best chain of page pairs that keeps both documents in order wins. Pages left between two pairs are paired in order, and the rest were added or removed.
- **Word changes.** Each pair of pages is then compared on its own (Myers diff over words, ignoring case and outer punctuation), so text never matches across pages that do not belong together. Neighbouring words make line boxes. Changes of case or punctuation only are not listed.
- **Looks.** Both pages are drawn 480 pixels wide in grey and compared cell by cell (8 × 8), leaving out the boxes of words (those are compared as text). Neighbouring changed cells make one region, and single stray cells are ignored.
- **Report.** The report starts with a summary page listing every change. Then each changed page pair is shown side by side, with the changes marked in colour: red for removed, green for added, amber for changed and blue for looks. The text uses a standard PDF font, so letters it lacks show as "?". The pages themselves are pictures.
- Comparing runs as a background job with progress and can be cancelled. The interface shows pages through `book://…/.compare/<id>/<a|b>/<page>`, drawn on demand and kept in memory while the comparison is open. Nothing is saved in the library.

## Consequences

- Text moved from the bottom of one page to the top of the next shows as removed on one page and added on the other.
- Scans without a text layer or OCR text are compared by looks only.
- New dependency: `similar` (MIT/Apache-2.0) for diffing.
