# 28. Podcasts: find, follow and listen beside a book

Status: accepted (2026-10-01). Part 1 built; part 2 (transcripts, chapters, notes at a moment) built 2026-10-02.

Libreri has a **Podcasts** section next to Feeds. Shows are found, followed and played inside Libreri, and keep playing while you read.

## Decisions (user, 2026-10-01)

- **A separate section**, not a kind of feed: shows, episodes and a player of their own.
- **Finding shows:** Apple's podcast search by default. It needs no key or account and works on macOS, Windows and Linux. Only the search words are sent.
- **Podcast Index is optional:** you enter your own free key and secret in Settings › Online details. It adds its search, popular shows and subjects. The key stays on this computer (`online-sources.json`) and is never exported. Libreri ships no key of its own.
- **Columns can be resized** (and are, everywhere in the app), so long show names can be read.

## How it works

- **Episodes come from each show's own RSS feed**, read by `libreri-feeds` like any feed, with the podcast parts added: the audio enclosure, `itunes:duration`, the show's artwork and author, and Podcasting 2.0 `podcast:transcript` and `podcast:chapters` addresses (kept for part 2).
- **Kept apart from feeds:** `Feeds/<profile>/.podcasts.json` (same format as `.feeds.json`, plus the Up next queue, each episode's position and played flag, and each show's speed). Every feed command takes a `space` (`feeds` or `podcasts`).
- **Artwork** is fetched after a refresh and kept as a 240 px thumbnail in the state file, so the list needs no network.
- **Playing:** one `<audio>` element for the whole window, so an episode keeps playing while you move between the library and books. It streams from the show (`media-src https:`), or plays the download through `book://`. A strip along the bottom shows it; in a book, the floating player has a **Podcast** mode instead (one voice at a time: reading aloud pauses the podcast).
- **Your place** is saved every 15 seconds, on pause and on leaving; an episode heard to the end is marked played and leaves Up next, and the next one starts.
- **Speed** is kept per show. Sleep timer: minutes, or the end of the episode. Media keys work.
- **Downloads** go to `Feeds/<profile>/Podcasts/<show>/`. A downloaded episode can be added to the library as an audiobook (series: the show).
- **Lists:** Up next (reorder, take out), New episodes, In progress, Downloaded, All episodes; per show: All, Not played, In progress, Downloaded, and Find in episodes.
- **Find podcasts:** Search (Apple, or Podcast Index with a key), Popular (Podcast Index), Address (an RSS address or a site), OPML file (from other podcast apps). Export to OPML too.
- **Pictures in search results** load from Apple's image servers (`img-src https://*.mzstatic.com`); other artwork shows the show's initials until it is followed.

## Part 2 (built 2026-10-02)

- **Transcript and chapters panel** (a button on the player, in the strip and in a book's floating player): the show's own transcript (WebVTT, SRT, JSON or HTML) follows along with the line being said highlighted; clicking a line or a chapter goes there. Chapters show which one is playing.
- **Note this moment:** the time, the line being said and an optional thought are written as Markdown into the notebook of the book being read, linked to its page, or into the note "Podcast moments" when no book is open. The moment is a `libreri://podcast/<episode>?t=<seconds>` link; clicking it in a notebook or the Notes hub plays the episode from there.
- **Written on this computer:** for shows without a transcript, *Write it down* (or *Download and write it down*) transcribes the downloaded episode with the speech model (whisper.cpp), five minutes at a time as a background job; lines appear as each part is done, and it can be stopped. Kept beside the download as `.<file>.transcript.json` (so it travels with downloads in backups), marked as written on this computer.
