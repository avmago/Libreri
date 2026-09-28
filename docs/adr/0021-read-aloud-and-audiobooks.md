# 21. Read aloud and audiobooks

Status: accepted (2026-09-28)

Phase 7a lets you listen to any book: text read aloud by the system's voices, and audiobooks in a player that can keep the book they read in step.

## Decisions (user, 2026-09-28)

- **Phase 7 comes in three parts:** 7a is read aloud and audiobooks, 7b is voice notes and dictation, and 7c is spell check and word suggestions.
- **Voices:** read aloud uses the system's voices; the optional offline neural voices (Piper) come later.
- **Audiobooks and text:** an audiobook can be linked to the book it reads, with sync points set by hand. Automatic sync by listening, using a locally run whisper.cpp, comes in 7b together with dictation, because both need the same speech-recognition model.
- **Voice notes** will live as audio files in `Notes/<profile>/` (7b).

## Read aloud

- Each renderer gives the text from the place shown, one sentence at a time (`Renderer.readAloud`, `readers/speech/`):
  - **Markdown, text and EPUB:** blocks of the HTML in reading order are split into sentences (`Intl.Segmenter`, with abbreviations joined back). Each sentence is marked with the CSS Custom Highlight API. The Markdown reader falls back to boxes over the text where the web view lacks that API.
  - **Maths:** formulas are read from their MathML (KaTeX adds MathML to Markdown formulas; EPUBs carry MathML), for example "a over b plus x squared". TeX is read the same way when there is no MathML.
  - **PDF, DjVu and scans:** the words of each page with their boxes come from PDF.js text, the DjVu text layer, or saved OCR words. Line boxes mark the sentence, and a change of size or a gap between lines starts a new sentence (so headings are read on their own).
- **Voices:** read aloud uses the web view's speech (Web Speech: the system voices on macOS and Windows, and on Linux where WebKitGTK has them). Where there are none, it uses eSpeak NG, a new optional helper program that Libreri can install, run by Libreri one sentence at a time and stopped at once when asked.
- **Controls:** a bar under the toolbar has previous and next sentence, pause, speed from 0.5× to 3×, the voice (the book's language first), and "Follow", which turns pages to what is being read. The Mod+Shift+U shortcut starts and stops reading. Speed, voice and Follow are kept in the profile's preferences.

## Audiobooks

- MP3, M4B, M4A, AAC, OGG, Opus and FLAC files open in a player instead of the reader. They play with the web view's own audio, streamed with range requests from `book://`.
- **Details:** titles, authors and covers come from their tags (lofty, MIT/Apache-2.0).
- **Chapters:** read from ID3v2 `CHAP` frames, QuickTime chapter tracks, Nero `chpl` atoms and `CHAPTERnnn` comments.
- **Player:** chapters with marks on the seek bar, back and forward buttons (15 s and 30 s), speed from 0.5× to 3×, a sleep timer (after 5 to 60 minutes, or at the end of the chapter, fading out), volume, and bookmarks (bookmark annotations with an `{type:"audio", t}` locator). The place is saved as the reading position. Media keys and the system's now-playing controls work through the Media Session API.
- **Link and sync:** `.library-data/audio-links/<audiobook id>.json` holds the linked book and its sync points. Each point has a time, a reader locator, 0–1 through the text, and a label. Links belong to the library, not a profile. They follow id changes and travel in backups and archives.
  - Between sync points both sides move in a straight line, and from the start and to the end where there are no points.
  - While the audiobook plays, the open book follows it (`goToFraction`).
  - In the book, *Listen from here* opens the audiobook beside it at the matching moment.
  - A sync point is added from the player: "now is where the open book is".

## Consequences

- Which audio formats play depends on the system's web view: WebKitGTK needs GStreamer plugins, and some systems lack AAC. The player says when a file cannot be played.
- The web view's speech gives no word timings everywhere, so sentences, not words, are marked.
- New dependency: lofty (MIT/Apache-2.0).
