# Privacy policy

*Last updated: 3 October 2026*

Libreri is a desktop app that keeps your library on your own computer. This page explains, plainly, what it does with your information. The short version: **Libreri has no servers, no accounts, no analytics and no advertising. We never receive your books, notes, highlights or reading habits.**

## What stays on your computer

Everything you put into Libreri stays on your computer, in your library folder or in Libreri's settings folder:

- your books, audiobooks, downloads and the catalogue of them;
- notes, highlights, comments, notebooks, canvases, voice notes and paper notes;
- reading positions, ratings, reading status, collections, flashcards and their schedule, the reading calendar and study sessions;
- profiles and PINs (PINs are stored only as a one-way hash);
- your settings, and any keys you enter for online services.

Speech recognition, natural voices, text recognition (OCR) and maths recognition run on your computer. Nothing you say, hear or scan is sent anywhere.

If you keep your library in a synced folder (iCloud Drive, Dropbox, OneDrive and so on), that service stores a copy under its own privacy policy. Libreri does not choose or control that.

## When Libreri goes online

Libreri connects to the internet only when you use a feature that needs it. Each request contains only what that feature needs; none contains your name, email, library contents or an identifier for you or your computer. Requests identify the app as `Libreri` and give the project's web address.

| When you… | Libreri contacts | It sends |
|---|---|---|
| Fill in a book's details or cover | Open Library, Google Books, Crossref, arXiv, OpenAlex, Semantic Scholar; ISBNdb and ComicVine only if you enter your own key | The ISBN, DOI, arXiv id, or title and author |
| Look up a word or name | Wiktionary and Wikipedia | The selected words and the book's language |
| Follow feeds | Each feed's own website | A request for the feed |
| Find or follow podcasts | Apple Podcasts search; Podcast Index only with your own key; each show's website | Your search words, or a request for the show's feed and episodes |
| Download a paper, article or episode | The website it comes from | A request for that file |
| Save a link to a web page or video | That website (and YouTube or Vimeo for video details) | A request for the page or its details |
| Download a voice, model, dictionary, OCR language or font | Hugging Face, GitHub, the npm registry, or the project that publishes it | A request for that file |
| Install a helper program | Your system's package manager (Homebrew, winget, apt, dnf, pacman, zypper) | What that package manager normally sends |
| Check for updates | GitHub (Libreri's releases) | A request for the latest version |
| Use the phone scanner | Nothing on the internet: your phone talks to your computer over your own network | — |

These services receive your computer's internet address, as with any web request, and handle it under their own privacy policies. Libreri does not add tracking to these requests and does not send cookies.

You can turn off checking for updates in Settings › General › Updates. Everything else in the table happens only when you ask for it.

## Keys for online services

If you enter a key for ISBNdb, ComicVine or Podcast Index, it is stored on your computer only (readable only by your user account on macOS and Linux), sent only to that service, and never included in exports or backups.

## Children

Libreri has Kids profiles that limit what a child can open. Libreri collects nothing from anyone, children included.

## Exports, backups and sharing

Exports and backups are files you create and keep. They contain what you chose to include and go only where you save or send them. Libreri never uploads them.

## Changes

If this policy changes, the new version will be published with the app's source code, and the date above will change. Libreri will never start collecting personal information without saying so here first and asking you in the app.

## Contact

Questions about privacy: open an issue at [github.com/avmg0/Libreri](https://github.com/avmg0/Libreri/issues).
