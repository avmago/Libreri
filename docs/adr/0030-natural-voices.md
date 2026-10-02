# 30. Natural voices for reading aloud

Status: accepted (2026-10-02). Kokoro and Piper built.

Read aloud used the system's voices (ADR 0021). On macOS and Windows they are good. On Linux they are often missing or robotic (eSpeak NG). Natural voices that run on the computer fix that everywhere.

## Decisions (user, 2026-10-02)

- **Offered on macOS, Windows and Linux** as optional downloads in **Settings › Reader › Read aloud voices**. Each download has **Download**, **Delete**, and a switch to turn its voices **on or off**. System voices are always there.
- **Only voices free to use**, including for work.
- **Kokoro 82M** (hexgrad, Apache 2.0): 54 voices in English (US and UK), Spanish, French, Hindi, Italian, Portuguese (Brazil), Japanese and Chinese. One download of about 350 MB, from the kokoro-onnx project's releases.
- **Piper** voices (one small model each, 40+ languages), from the `rhasspy/piper-voices` collection. Each voice's MODEL_CARD names its dataset licence. A voice is offered only when every licence it names is public domain, CC0, CC BY or CC BY-SA, MIT, Apache or BSD. Non-commercial, no-derivatives, unknown and "see this page" licences are left out, and the list says how many were left out. So the popular *Lessac* voice (Blizzard dataset licence) is not offered.

## How it works

- **New crate `libreri-voices`** handles downloads (resumable, with progress and cancel, and a `.complete` note per download), the Piper list (kept for a week) and its licences (read once per voice and kept), removing, and speaking.
- **ONNX Runtime 1.23.2** (MIT, Microsoft) runs both kinds of voice. It is downloaded from Microsoft's GitHub releases with the first voice and loaded at run time (`ort` with `load-dynamic`), so Libreri stays small for people who never use natural voices. 1.23 is the newest release with builds for Intel Macs. The runtime is deleted with the last voice.
- **Phonemes come from eSpeak NG**, run as a separate helper program as before. Libreri itself therefore links no GPL code; sherpa-onnx 1.x would have linked eSpeak NG in. eSpeak NG can be installed from Settings with Homebrew, winget (`eSpeak-NG.eSpeak-NG`, new) or the Linux package managers.
  - Text is cut at punctuation, each piece is turned into IPA, and the marks are put back. This is what `phonemizer` does for kokoro-onnx, and the tests check the phonemes and token numbers against kokoro-onnx and Piper's own.
- **Kokoro:**
  - Its tokens come from its vocabulary.
  - The style row is the phoneme count minus one.
  - Text longer than 510 phonemes is cut into batches.
  - Output is 24 kHz.
- **Piper:**
  - Ids are made by its `phoneme_id_map` with the pad after each phoneme (NFD).
  - Each sentence is spoken on its own, with a short rest between sentences.
  - Its own noise, length and noise-w settings are used.
- **Playback:**
  - Each sentence becomes a WAV file, sent as base64 and played from a blob URL.
  - The next sentence is made while one plays, using a new `peek()` on read-aloud sources.
  - Speeds from 0.5× to 2× are the model's own; faster than that is played faster with the pitch kept.
  - Stopping read aloud frees the model's memory.
- **Choosing a voice:**
  - The player's voice menu groups voices as *Natural · Kokoro*, *Natural · Piper* and *This computer*, and ends with *Get more voices…*.
  - The voice picked is remembered for the book's language (`listening.voiceFor`, per profile). Settings › Reader › *Voice for each language* sets it too.
  - With nothing chosen, the most natural voice for the language is used.
- **Samples:**
  - Kokoro's sample is a short recording made with Libreri itself (`public/samples/kokoro-heart.mp3`).
  - Piper's samples stream from the Piper project's sample pages.
- **Storage:** files live in the app data folder, in `voices/` (`runtime/`, `kokoro/`, `piper/<voice>/`). They belong to this computer; backups and exports leave them out. Settings keeps `voices_off`.

## Not yet

- Word-by-word highlighting with natural voices (Kokoro gives durations; they are not used yet).
- Multi-speaker Piper voices use their first speaker.
- Japanese and Chinese Kokoro voices use eSpeak's phonemes, which are weaker than the misaki rules Kokoro was trained with.
