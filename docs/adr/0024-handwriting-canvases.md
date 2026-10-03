# 24. Handwriting canvases and ink to text

Status: accepted.

Libreri lets you write and draw by hand beside a book, clip figures from its pages, and turn handwriting into typed text.

## Decisions

- **Related records:** paper notes and maths as LaTeX are in ADR 0025, links to web pages and videos in ADR 0026.
- **Canvases are Excalidraw files.**
- **Ink to text:** the system's recogniser or Tesseract, chosen in Settings. Linux uses Tesseract.
- **Maths from pictures:**
  - Exact LaTeX stays the default: EPUB MathML, Markdown, and a rebuild from a PDF's text layer.
  - An optional maths model (pix2tex, MIT) can be downloaded in Settings. It runs with candle, a pure-Rust engine built into Libreri, so a missing or bad model only affects that feature (see ADR 0025).

## Canvases

- **The editor:** Excalidraw (MIT) is loaded only when a canvas opens, because it is large. Its fonts are served by Libreri (copied from the package at build time), never from a CDN. The 13 MB Chinese, Japanese and Korean handwriting font (Xiaolai) is left out of the app. It can be downloaded in Settings › Writing › Canvas fonts, and removed there too. It is taken from the same Excalidraw release on the npm registry, kept in app data `excalidraw/fonts/`, and served to canvases through `book://…/.extras/`, the second place Excalidraw looks for fonts.
- **Files:** each canvas is a standard `.excalidraw` file in `Notes/<profile>/Canvases/`, so it opens in excalidraw.com and other tools. Libreri's details sit in a `libreri` object that other tools ignore:
  - `book` is a `libreri://book/<id>` link, resolved through the book's aliases, so it keeps working after a book changes.
  - `paper` is plain, lined, squared or dotted.
- **Saving:** a canvas is saved as you draw, a moment after each change. Pictures are kept inside the file, up to 80 MB. Deleting a canvas moves it to the system trash.
- **Paper:** drawn behind a transparent canvas and moved and scaled with it, so the lines stay under the ink. Exports stay on white.
- **In the reader:** the pen button opens the canvas panel, in place of the notebook: this book's canvases, a new canvas and a wider view.
  - **Clip from page** (PDF, DjVu and comics): draw a box on the page. The part under it is copied as a picture (from the rendered page, or the page image fetched again), linked with `libreri://book/<id>#page=<n>&rect=x,y,w,h` and labelled with the book and page. The link opens the page.
  - Other links on a canvas open like links in notes.
- **Notes hub:** a *Canvases* tab lists every canvas and opens it.
- **Links:** `parseBookLink` now reads `#page=<n>` (with an optional `&rect=`), and a tab can open at a page.

## Ink to text

- **Using it:** select handwriting on a canvas, then choose *Ink to text*. The strokes are drawn onto white at twice their size and read. The text is added below them, and the ink stays.
- **System recognisers** are run by small scripts through the system's own tools, so nothing native is built into Libreri and failures stay inside this feature:
  - macOS: Apple's Vision (`VNRecognizeTextRequest`), accurate level, run as JavaScript for Automation with `osascript`. It reads joined-up handwriting.
  - Windows: `Windows.Media.Ocr`, run through Windows PowerShell. It is weaker on handwriting.
- **Tesseract** reads the image as one block of text (`--psm 6`) in the OCR languages chosen in Settings › Helper programs. It works best on neat printed letters.
- **The choice** is made in Settings › Writing › Handwriting, per computer. The default is the system's recogniser where there is one.

## Consequences

- **New npm package:** `@excalidraw/excalidraw` (MIT). Run `pnpm install` after pulling. It adds about 2 MB, loaded only with a canvas, plus 0.5 MB of fonts.
- **Clipping:** pages of ebooks (EPUB, Markdown) cannot be clipped as pictures. Quote them into the notebook instead.
- **Not tested here:** the macOS and Windows recognisers were not run in development (Linux). They report errors in a message, and Tesseract is always available as the other choice.
