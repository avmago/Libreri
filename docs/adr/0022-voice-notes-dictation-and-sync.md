# 22. Voice notes, dictation and automatic audiobook sync

Status: accepted.

Libreri adds speech recognition: voice notes, dictation and finding an audiobook's places in its book by listening. All of it runs on the computer.

## Decisions

- **Whisper is built into the app** (whisper.cpp through whisper-rs), not a helper program.
- **Models:** Tiny and Small are offered first; Medium and Large (turbo) can be downloaded from Settings too. Only a downloaded model can be chosen as the one to use.
- **Dictation:** the system's own dictation first (macOS Dictation, Windows voice typing), plus a microphone button that uses Libreri's model.
- **Voice notes:** both on a place in a book (the selected text, or the page being read) and inside notebooks and notes.

## Speech recognition

- New crate `libreri-speech`:
  - `models`: the model list, download with progress and cancel, check the file (ggml magic, full length), and remove.
  - `audio`: decode MP3, AAC/M4B, FLAC, Ogg Vorbis and WAV to 16 kHz mono with symphonia, and write FLAC with flacenc.
  - `transcribe`: whisper with greedy sampling. The language comes from Settings, or the book's language, or is detected. Noise markers like "[BLANK_AUDIO]" are dropped.
  - `sync`: match heard words to the book's text.
- **Where models are kept:** in the app's data folder (`whisper/`), per computer. They are never part of a library, backup or archive. The first model downloaded becomes the one used. Removing the one in use picks another downloaded one.
- **Loading:** the model is loaded on first use and kept in memory. Changing or removing it frees it. On macOS whisper uses Metal.
- **Settings › Speech (this computer):** models (download, stop, remove, use this model), the language spoken, and whether voice notes are written down.

## Voice notes

- **Recording:** the interface records the microphone (a script processor, which works in every web view Libreri runs in) and turns it into 16 kHz 16-bit mono. It is sent to Rust as base64 and saved as FLAC in `Notes/<profile>/Voice notes/<date time>.flac`. Recordings are ordinary files in the person's notes folder.
- **In a book:** an annotation of the new kind `voice`. Its locator is the reader locator (the selection, or the page or place being read) plus `"audio": "Notes/…"` and `"duration"`. The quote is the selected text, if any. `note` holds the transcript, which can be edited.
  - Voice notes on selected text are drawn with a dotted underline.
  - They are listed under *Voice notes* in the Marks panel, with a player.
  - The Notes hub has a *Voice notes* filter.
  - Deleting the voice note keeps the recording in the notes folder, so the Notes hub's *Undo* brings it back whole.
- **In notebooks and notes:** a microphone button records and inserts `**Voice note** [0:42](<Voice notes/….flac>)` with the transcript as a quote. The relative link also works in other Markdown editors. The preview shows a player in place of the link.
- **Validation:** a voice annotation must name an audio file under `Notes/`, with no `..` parts. The `book://` protocol serves only the signed-in profile's own notes folder.

## Dictation

- **The button** (next to comments, voice-note transcripts, the notebook and notes while editing) records and sends each stretch of speech at a pause. Pauses are found by comparing loudness with the room's level. Each stretch is written down, and the text is typed at the cursor with `insertText`, so undo works.
- **The system's dictation** works in every text box without Libreri doing anything. The button's tooltip and Settings say how to start it.

## Finding sync points by listening

- **Listening:** a background job listens to about one 24-second stretch every five minutes of the audiobook (at least 6, at most 40).
- **Matching:**
  - Each heard stretch is looked up in the linked book's words. The words come from `book_text`, with saved OCR text for pages that have none.
  - Every three-word run votes for an alignment, so a few misheard words don't matter. Very common runs are ignored.
  - Places that go backwards are dropped by keeping the longest run that goes forward.
- **Saving:** each place becomes a sync point with `auto: true`. For page-based books it is placed by page ("p. 12"). For flowing books it is placed by length through the text.
- **Points set by hand win:** found points near one, or on the wrong side of one, are left out. Running it again replaces only the found points.

## Consequences

- **Building** needs cmake and clang (whisper.cpp is compiled with the app), so CI installs both on Linux. On the Mac, cmake comes from Homebrew.
- **Model sizes:** 75 MB to 1.6 GB, downloaded from the whisper.cpp model repository on Hugging Face (`LIBRERI_WHISPER_URL` can point elsewhere).
- **Speed:** transcribing is quick with Tiny and Small on recent computers. Large models are slow without a GPU.
- **Microphone permission:** macOS asks the first time the microphone is used. `NSMicrophoneUsageDescription` and the audio-input entitlement are added.
- **New dependencies:**
  - whisper-rs and whisper.cpp (Unlicense / MIT)
  - symphonia (MPL-2.0)
  - flacenc (Apache-2.0)
