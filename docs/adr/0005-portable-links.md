# 5. Every link survives export, import and a new computer

Status: accepted.

All notes, highlights, canvases, voice notes and links point to books with `libreri://book/<id>#<anchor>`, store a precise locator plus a text-quote fallback, and use only library-relative paths. Import re-links by hash, then by ISBN/DOI/title, then asks the user to locate missing files. Round-trip tests enforce this. See docs/data-portability.md.
