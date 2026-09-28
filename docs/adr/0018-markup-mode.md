# 18. Markup mode and the measure tool

Status: accepted (2026-09-28)

Phase 6a lets you draw and write on fixed pages (PDF, DjVu, comics) and measure on them (boards 4b, and the measure part of 4d).

## Decisions (user, 2026-09-28)

- **Markup is kept in Libreri, like highlights.** Every mark is an annotation of the new kind `markup`, stored per profile. It goes into the sidecar backups, Libreri archives and backups like any note. The book file is never changed.
- *Export marked-up copy* makes a new PDF with the markup drawn in. *Save into the PDF* (standard PDF annotations, as a new version) comes with version history in 6b.

## Marks

The locator is `{"type":"markup","page":N,"layer":"…","item":{…}}`. Positions are fractions of the page as shown (top-left origin), so marks stay put at any zoom. Widths and text sizes are fractions of the page width.

| Tool | Item |
|---|---|
| Pen | points with pressure (stylus pointer events), drawn with perfect-freehand (MIT). Rough lines, boxes and circles snap to clean shapes (on by default, can be turned off). |
| Highlighter pen | flat, wide, see-through ink that darkens (multiply) rather than covers |
| Eraser | removes whole marks it touches |
| Rectangle, ellipse, line, arrow | solid, dashed or dotted; rectangles and ellipses can be filled; Shift keeps lines at 45° steps |
| Text box | typed in place; sans, serif or handwriting font; four sizes |
| Sticky note | an icon on the page; its text is the annotation's note (shown in the Notes hub) |
| Stamps | Approved, Reviewed, Draft, Confidential, Not approved, and your own |
| Signature | drawn (with pressure), typed in a handwriting font, or a picture; up to five kept in your profile's preferences |
| Picture | any picture file, made smaller (at most 1,600 px) and stored inside the mark (up to about 2 MB) |
| Measure | distance, perimeter, area and angle with a label |

- **Select** moves marks and resizes box-shaped ones; colour, width, line style and opacity changes apply to the selected mark. Undo and redo (Ctrl+Z, Ctrl+Shift+Z) cover every change in the session; Delete removes the selected mark.
- **Layers**: each mark belongs to a layer ("Markup" by default). The side panel's *Markup* tab lists every mark by page (click to go there and select it), shows or hides layers, and hides all markup. What is shown or hidden is remembered per book on this computer.
- In Read mode marks stay visible, and sticky notes open when clicked; everything else lets clicks and text selection through.
- Alt+M switches between Read and Markup.

## Measuring

- On PDFs, measurements use the page's true size: points converted to mm, cm, m, in, ft or pt. A page's own scale is used when the PDF has one (viewport `/Measure` dictionaries, common in CAD and map exports).
- **Calibrate**: draw over something of known length and type its real length. The scale is kept per book on this computer. On DjVu and comic pages, measurements are in pixels until calibrated.
- Each measurement keeps its scale and page size, so its label does not change if the calibration changes later.

## Export marked-up copy

- The interface turns marks into a drawing list: filled and stroked paths (SVG path data in page fractions) and pictures. Text boxes, stamps, sticky-note icons and measurement labels are sent as pictures, drawn exactly as on screen, so every font and script survives. Ink, shapes and lines stay vector.
- New crate `libreri-pdf-edit` draws the list on a copy of the PDF with lopdf. It adds content streams wrapped in `q … Q` so the page's own drawing state is untouched, and handles crop boxes and page rotation. Transparency and the highlighter's multiply blend become ExtGStates; pictures become image XObjects with soft masks. The page's text stays selectable and searchable.
- DjVu and comic books become a PDF of their page images first.
- The copy can also be added to the library, next to the book. Password-protected PDFs are refused.

## Also changed

- **PDF.js now loads its "legacy" build.** The standard build of pdfjs-dist 6 needs very new JavaScript (`Map.prototype.getOrInsertComputed`, `Promise.try`, `Math.sumPrecise`, `Uint8Array.toHex`), which the system web views on older macOS versions and WebKitGTK on Linux lack; PDFs would not open there. The legacy build carries polyfills.
- The Notes hub lists sticky notes and text boxes (drawings stay in the reader's Markup tab); Markdown notes include them.

## Consequences

- `AnnotationKind::Markup`, checked on save (valid locator, at most 3 MB).
- New commands: `export_marked_up`, `read_picture`, `measure_scales`. New preference group `markup` (signatures, own stamps, last colour and width, snap).
- Not yet: lasso selection, pixel eraser, markup on EPUB/Markdown (only highlights there), handwriting recognition (Phase 8).
