# 9. One renderer interface, three engines

Status: accepted (2026-09-27)

The reader talks to books only through `readers/types.ts` (`Renderer`). Three engines implement it:

- **PDF:** PDF.js with its viewer (text layer, links, find). Pages stream through HTTP range requests on `book://`. Highlights are rectangles in fractions of the page plus a text quote.
- **EPUB, MOBI, AZW3, FB2, CBZ:** foliate-js (MIT). Highlights are EPUB CFIs plus a text quote. Scripts inside books never run (the content security policy blocks them).
- **Markdown and TXT:** rendered as one document with markdown-it, KaTeX, highlight.js and Mermaid; raw HTML is not rendered. Highlights are character offsets plus a text quote.

Dark PDF pages are recoloured with an SVG colour-matrix filter on the page canvases (paper → theme background, ink → theme text), because PDF.js's own recolouring relies on canvas filters that WebKit does not support. "Invert" and "Dim" are CSS filters.

Annotations are stored per profile in SQLite and backed up to `.library-data/annotations/<profile>/<book>.json`; notebooks are Markdown files in `Notes/<profile>/` whose front matter links to the book, so both survive a database rebuild. This logic lives in `libreri-library` (`reading.rs`); a separate `libreri-annotations` crate will be split out when ink and voice notes arrive.
