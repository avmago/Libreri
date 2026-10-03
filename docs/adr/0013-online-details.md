# 13. Book details from online sources, and barcode scanning

Status: accepted.

## Online details

A new crate, `libreri-metadata`, asks free sources about a book and returns **candidates**: what one source knows (as `BookMetadata`), where it came from, a link to the source's page, a cover address and a match score. Nothing is saved by the crate; the reader chooses.

- **Which sources are asked** depends on what is known. An ISBN goes to Open Library, Google Books and ISBNdb; a DOI to Crossref, OpenAlex and Semantic Scholar; an arXiv id to arXiv and Semantic Scholar. With no identifier (or none that finds anything) the title and first author go to the book sources, and to Crossref and OpenAlex for papers; comics go to ComicVine. All sources are asked at once, each on its own thread, with a 12 s limit.
- **Identifiers from the pages.** When a book has no ISBN, DOI or arXiv id in its details, Libreri reads the text of a PDF's first four and last two pages to find one (`libreri_formats::find_identifiers`).
- **Scores.** A candidate found by identifier scores 1. Others are scored by title words in common (either way round, subtitle included) and whether the author's name appears; they never reach 1. 0.85 and above counts as a *sure match*.
- **Tidying.** Subject headings become at most 12 tags: "Physics -- Textbooks" gives "Physics"; library housekeeping ("Accessible book", "Protected DAISY", "In library", NYT lists, dates) is dropped; duplicates are merged. Google's "Science / Physics / General" becomes the category `Science/Physics`; OpenAlex topics become `Field/Subfield`; arXiv classes become subjects ("cs.LG" → Computer Science, tag "Machine learning"). Dates become years, "eng" becomes "en", "Smith, John" becomes "John Smith", ISBN-10 and -13 fill each other.
- **Privacy.** Requests carry only the identifier or the title and author, and a User-Agent naming the app and its repository; no e-mail address, no library or profile data. Nothing is looked up unless the reader asks, or turns on *Fill in missing details when importing* (off by default).
- **Keys.** ComicVine and ISBNdb need the reader's own key. Keys and the list of sources in use live in `online-sources.json` in the computer's app-config folder (readable only by the user on macOS and Linux), never in the library, so they are never exported or synced. The interface only ever sees the last four characters.
- **Only profiles that can change the library** (owner and standard) can look up, save details or change these settings.

### Picking what to keep

*Find details online* (Mod+Shift+D, the globe button in the details panel, the book menu) opens a search form filled with the book's identifiers, title and author, searches at once, and lists the matches best first. Choosing a match shows a table: each field of the book next to the match's value, with a tick to use it. By default only empty fields, new tags and categories, and a more specific type than "Book" are ticked; nothing the reader typed is replaced unless they tick it. Fields can be taken from different matches (the row says whose value is used). Tags and categories are **added**, never replaced. The cover is shown side by side and ticked when the book has none.

*Fill in missing details online* (book menu for several books, the details panel, the command palette) does the same for many books in the background, but only with sure matches and only for empty fields and missing covers. The report says how many were filled, and offers to select the books that had no sure match so they can be looked up one by one.

### Covers

Covers are downloaded by Rust, only from the hosts the sources use (`covers.openlibrary.org`, `archive.org`, `books.google.com`, `books.googleusercontent.com`, `comicvine.gamespot.com`, `images.isbndb.com`), only over HTTPS, only if the answer really is an image (checked by its first bytes) and not a tiny placeholder. Previews reach the interface as `data:` URLs, so the webview never loads anything from the internet and its content security policy stays closed.

### Testing without the network

Parsers are tested against saved answers in `crates/libreri-metadata/tests/fixtures`. Debug builds started with `LIBRERI_HTTP_FIXTURES=<folder>` answer every request from files listed in `<folder>/routes.json`, for trying the interface offline. Release builds always use the network.

## Barcode scanning

A second crate, `libreri-scan`, reads EAN-13, EAN-8, UPC-A and UPC-E barcodes with `rxing` (the picture is scaled to 1600 px and also tried turned sideways). A code starting 978/979 (or an ISBN-10) becomes the ISBN in the search form, which searches at once. Three ways to take the picture:

- **Camera.** The webview's camera (`getUserMedia`) shows a live view; about three frames a second are sent to Rust until one reads. Pictures never leave the computer. macOS asks once (`NSCameraUsageDescription` in `Info.plist`; signed builds carry the camera entitlement). Where the webview offers no camera, the dialog says so and points to the other two ways.
- **Picture file.** Any photo or screenshot.
- **Phone.** Libreri starts a small web server on the computer's local network address and a random port, and shows its address as a QR code. The address contains a 128-bit random token; anything without it gets "Not found". The page is self-contained (no outside scripts, fonts or images; strict content security policy) and uses the phone's own camera app through a file input, so it works over plain HTTP. The phone scales the photo down and sends it; the computer reads the barcode and the dialog continues as with the camera. The server accepts only pictures up to 10 MB and codes up to 64 bytes, and stops after 10 minutes, when the dialog closes, or when the library closes. The first time, macOS may ask whether Libreri may accept incoming connections.

## Consequences

- Two new crates and three new dependencies of note: `ureq` (HTTPS with rustls), `rxing` (barcodes) and `tiny_http` (the phone page).
- Live lookups cannot be tested in CI; the parsers follow each source's documented answer format and are checked against fixtures. When a source changes its format, a fixture and parser change together.
