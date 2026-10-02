# 27. Feeds: papers, articles and newsletters from RSS and Atom

Status: accepted (2026-09-29)

Libreri follows RSS and Atom feeds, so new papers, preprints, articles and newsletters come in by themselves. Each can be downloaded or deleted, and what is kept can be added to the library.

## Decisions (user, 2026-09-29)

- **Whose:** each profile has its own subscriptions and downloads. Guests have none.
- **When:** new items are looked for while Libreri is open: soon after the library opens, then every hour by default (30 minutes to 12 hours, or only on Refresh). Nothing runs while the app is closed.
- **Articles without a PDF** (news, newsletters, blog posts) are downloaded as a readable copy.
- **Adding feeds:** by a feed's or a site's address, from arXiv's categories (and arXiv searches), from a short list of suggested sources, or from an OPML file.
- **Where downloads go:** a folder of their own, `Feeds/<profile>/`, apart from `Books/` and `Notes/`. From there an item can be added to any folder of the library, where it becomes a book.

## Folders, feeds and items

- **Folders** sort feeds into categories and subcategories (any depth). A folder, or a feed, can be set to *Download new items automatically*; a feed follows the setting of any folder it is in.
- **Topics** are the feed's own categories for each item (arXiv: `cs.AI`, `math.PR`, shown with arXiv's names). The list can be filtered by topic, by New / Downloaded / In library, and by words in the title, authors or abstract.
- **Each item** has *Download* (the PDF, or the article) and *Delete*. Once downloaded: *Read* (in a tab, with the library's own reader), *Add to library*, *Show in folder*, *Delete the download only*. Added items show *In library · Open*.
- **Deleted items do not come back**, even while the feed still lists them (they are remembered for 400 days). Items not downloaded are removed after 30 days by default (7 days to a year).

## How it is kept

- **`Feeds/<profile>/.feeds.json`** holds the folders, feeds (address, title, folder, auto-download, the site's ETag and Last-Modified for "not modified" answers, the last error), the items with their details, and the settings. It is written aside and renamed, so a crash never leaves half a file.
- **Downloads** are `Feeds/<profile>/<folder>/<subfolder>/<feed>/<title>.pdf|md`, mirroring the folders.
- **Articles are Markdown**, so they are books Libreri can read and other apps can open: the readable part of the page (picked out as for links' offline copies, ADR 0026), converted with `htmd`, pictures inside as data. Front matter carries the title, authors, date, publisher (the feed), address, DOI, tags (the topics) and abstract, which the library reads on import.
- **Profiles:** the folder follows a renamed profile and goes to the system trash with a deleted one. `book://` serves `Feeds/` files only to their own profile.
- **Backups and archives (2026-10-02):** Backups and whole-library exports (with notes) carry each profile's `Feeds/<profile>/` subscriptions (`.feeds.json`, `.podcasts.json`), and its downloads (papers, articles, episodes) when book files are included. Importing joins them with the profile's own: folders by name, feeds by address, items by feed and key, keeping what was read, heard and downloaded (`State::absorb`).

## Reading feeds (`libreri-feeds`)

- **`feed-rs`** reads RSS 0.9x, 1.0 and 2.0, Atom and JSON Feed. Libreri keeps each item's key stable (the feed's id, or its link, or title and date), because feed-rs makes random ids for items without one.
- **Details:** authors (arXiv's single `dc:creator` is split into names), the abstract as plain text (arXiv's "Announce Type" prefix removed and kept as New, Cross-list or Updated), topics, date, a PDF (a `application/pdf` link or enclosure, a `.pdf` link, or arXiv's `/pdf/<id>`), the arXiv id and a DOI (in the id, links or text; Nature's from its article address).
- **Finding a site's feed:** the address is read; if it is a page, its `<link rel="alternate">` feeds are tried, then `/feed`, `/rss`, `/atom.xml`, `/feed.xml`, `/rss.xml`, `/index.xml`. `feed:` addresses become https.
- **Refreshing** reads six feeds at a time, with If-None-Match and If-Modified-Since. A check already running is not started twice; a profile switch during a check stops it from writing.
- **Limits:** feeds 16 MB, pages 8 MB, PDFs 300 MB (checked to start with `%PDF`).
- **arXiv:** categories are arXiv's taxonomy (built in), each followed at `https://rss.arxiv.org/rss/<code>` in *arXiv › <group>*. Searches use arXiv's API (`export.arxiv.org/api/query`, newest 50, sorted by date) in *arXiv › Searches*; plain words must all appear, and arXiv's own syntax (`au:`, `ti:`, `cat:`, AND/OR) is kept.
- **Suggested sources** were checked to answer as feeds on 2026-09-29: Nature, PLOS ONE, PLOS Biology, eLife, JOSS, Quanta Magazine, IEEE Spectrum, MIT Technology Review and Hacker News. bioRxiv's and medRxiv's addresses could not be confirmed, and Science and PNAS refuse automated readers, so they are left out; they can still be added by address.

## Adding to the library

The download is imported into the chosen folder of `Books/` (moved, as imports do). The book then gets the feed's details where the feed knows better than the file: title, authors, abstract, year, publisher (arXiv, or the feed's title), DOI, arXiv id, address, topics as tags (arXiv codes as names), and a content type (Preprint for arXiv and preprint servers, Research paper when there is a DOI, Article for articles). A file already in the library is not added twice: that book is linked instead. Profiles that cannot edit the library can read and download, not add.

## In the app

- **Feeds** in the sidebar (with the number of new items), the command palette, and Mod+Shift+0.
- **Feeds screen:** folders and feeds on the left, with their menus (auto-download, check now, mark all seen, rename, move, remove / stop following); items on the right. *Follow feeds* has four tabs: Address, arXiv, Suggested, OPML file. The ⋯ menu has New folder, Import and Export OPML, Show the Feeds folder, how often to check, and how long to keep items.
- **Reading a download:** PDFs in Libreri's PDF reader, articles in its Markdown reader, inside a window; or *Open in another app*.

## Changes (2026-10-01)

- **Read opens a tab** in the library's own reader (PDF or article), instead of a separate window: page appearance, ADHD reading, read aloud, find and reading maths all work, and the place is kept on this computer. Highlights, bookmarks and notes come once it is added to the library; *Add to library* in the tab replaces it with the book.
- **Opened items stay in New** until you choose another list or feed, so they do not vanish while being read (they are marked seen at once).
