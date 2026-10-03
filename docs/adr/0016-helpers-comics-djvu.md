# 16. Helper programs, comics and DjVu

Status: accepted.

Libreri opens every page-image format in the reader: DjVu and comics (CBZ, CBR, CB7, CBT, CBA).

## Helper programs

Some formats need open-source programs Libreri runs as separate processes, never links:

| Helper | Programs | Used for | Licence |
|---|---|---|---|
| DjVuLibre | `ddjvu`, `djvutxt`, `djvused` | DjVu pages, text layer, contents, details | GPL-2.0 |
| Tesseract | `tesseract` | OCR | Apache-2.0 |
| unar | `unar`, `lsar` | ACE comics (.cba), optional | LGPL-2.1 |

- `libreri-helpers` finds them on `PATH` and in the usual install folders (Homebrew, `/usr/local`, Program Files), reports version and missing programs, and runs them without a console window on Windows.
- **Install…** (Settings › Helper programs, and the reader's error panel when a book needs one) shows the exact command and runs it with the computer's package manager: Homebrew on macOS, winget on Windows, apt/dnf/pacman/zypper through `pkexec` on Linux. Output streams into the dialog. Without a package manager the dialog says where to get the program.
- The Flatpak cannot use a package manager, so DjVuLibre, Tesseract and eSpeak NG are built into it; unar is left out.
- Helpers are per computer; nothing about them goes into the library or backups.

## Comics

- CBZ pages are read straight from the zip. RAR (`unrar` crate), 7z (`sevenz-rust2`) and tar are solid or sequential, so the first open unpacks the pages once into the page cache; ACE goes through `unar`.
- Page order is natural sort (`p2` before `p10`). `ComicInfo.xml` gives title, series, number, writer and `Manga=YesAndRightToLeft`.
- Layouts: one page, two pages side by side (the cover alone), continuous (webtoon), and right to left, remembered per book on this computer. Clicking the left or right third turns the page; arrows swap for right to left.

## DjVu

- Pages are rendered by `ddjvu` to PNM, turned into JPEG at a few width steps (480 … 3200 px) and cached.
- The hidden text layer (`djvutxt --detail=word`) sits over each page, so text can be selected, highlighted, noted and found. Highlights use the PDF locators (page plus rectangles as fractions of the page), so notes, exports and the notebook treat DjVu like PDF.
- The outline becomes the table of contents; metadata title and author fill details.

## Page cache

- Rendered and unpacked pages live in the app cache on each computer (`…/pages/<bookId>/`), capped at 2 GB, oldest books pruned at start. Settings › Library & storage shows its size with a *Clear* button. It is rebuilt when needed and is not part of backups.

## Consequences

- The reader has a third renderer, `PageRenderer`, served by the `book://…/.pages/<bookId>/<page>?w=` route.
- CI installs `djvulibre-bin` so the DjVu tests run; they skip where DjVuLibre is absent.
