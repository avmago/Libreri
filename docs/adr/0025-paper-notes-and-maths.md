# 25. Paper notes and maths as LaTeX

Status: accepted (2026-09-28)

Phase 8b lets you photograph paper notes and keep them beside the page they belong to. It also lets you copy maths as LaTeX.

## Decisions (user, 2026-09-28)

- **Capture from:** the computer's camera, the phone (the one-time phone page from Phase 4a) and picture files.
- **Saved as:** a PDF in the profile's notes folder, linked to the page.
- **Maths:**
  - Exact LaTeX is the default.
  - A downloadable model can be turned on in Settings to read maths from pictures. The user asked that it must not be able to break the app.

## Paper notes

- **Cleaning up a photo** (`libreri-scan/src/paper.rs`, pure Rust):
  - The page is found as the largest bright shape (Otsu threshold), and its corners are the extremes along the diagonals. The corners can be dragged in the dialog.
  - The page is straightened with a perspective transform.
  - The lighting is evened out, in colour, grey or black and white, or not at all.
  - The page can be turned and the pages reordered.
  - HEIC photos are converted with `sips` on macOS.
- **Saving** (`libreri-pdf-edit::pictures_pdf`):
  - The pages become one PDF in `Notes/<profile>/Captures/<title>.pdf`.
  - When *Read the text* is ticked, Tesseract adds an invisible text layer, so the PDF can be searched.
  - The text read is also kept as the note's text.
- **The link:**
  - An annotation of kind `capture` is made where you are reading.
  - Its locator is the reader's own locator plus `capture` (the PDF's path, which must be under `Notes/` and end in `.pdf`), `pages` and `title`.
  - It is shown in Contents (*Paper notes*), in Marks, and in the Notes hub (*Paper notes* filter), with a viewer and *Open in another app*.
- **Deleting and Undo:** deleting the note keeps the PDF, as it does for voice notes, so Undo brings the note back complete. The file stays in the notes folder, where it can still be opened or deleted by hand.

## Maths as LaTeX

- **Exact, always on:**
  - Clicking a formula in a Markdown book gives the TeX that KaTeX keeps.
  - In an EPUB, it gives the MathML's TeX annotation, or the MathML turned into LaTeX (`readers/math/latex.ts`).
  - Selected text from a PDF can be copied as LaTeX. Symbols, superscripts and subscripts are rebuilt, and the popover says to check fractions and roots.
- **The popover:** a live KaTeX preview, the LaTeX to correct, *Copy LaTeX*, *Copy as $…$* and *Add to notebook* (a `$$` block).
- **From pictures, optional** (`libreri-maths`):
  - The model is pix2tex (Lukas Blecher, MIT): a ResNet and vision-transformer encoder with a transformer decoder.
  - It is run with candle, a pure-Rust engine compiled into Libreri. There is no separate runtime, no Python and no native library to load, so an absent or bad model cannot crash the app.
  - The weights (102 MB) are downloaded in Settings › Writing › Maths from the project's own GitHub release. They are checked against a known size and SHA-256, and kept per computer in app data `maths/`. *Remove* deletes them and turns the feature off.
  - **Reading a picture:**
    - The writing is cropped out and turned dark on light.
    - The picture is read at up to four sizes, set from the typical height of its marks. The reading the model is surest of is kept (mean log-probability), stopping early when it is nearly certain.
    - In tests on KaTeX renders at three sizes, 17 of 18 formulas were read exactly. The one miss was a very small render.
  - **In the reader:** with the model on, the Σ button in PDF, DjVu and comic readers lets you draw a box around a formula. The clip is read on this computer and opens in the same popover, marked as read from a picture.
  - The model is loaded when first used and kept while the app runs.
- **Not done:** pix2tex's separate image-resizer network. The size search above gave the same results without a second download.
