# 7. PDF covers are rendered by the interface

Status: accepted (2026-09-27)

Rust reads PDF details (title, authors, page count) with lopdf, but does not render pages: that would need a native PDF engine (PDFium or MuPDF) on every platform. The interface already ships PDF.js for the reader, so it renders the first page of PDFs without a cover in the background and sends the image to `save_cover`, which makes the cover and thumbnail like any other image. EPUB, FB2 and comic covers come from the files themselves in Rust.
