# 23. Spell check and word suggestions

Status: accepted (2026-09-28)

Phase 7c checks spelling everywhere you write, suggests corrections, and completes words from your books and notes.

## Decisions (user, 2026-09-28)

- **Checker:** Libreri's own, not the web view's, so it behaves the same on every system and can know the words of your books.
- **Dictionaries:** English (US and UK) comes with the app. Other languages are downloaded from Settings.
- **Suggestions:** complete words while typing, offer spelling fixes, and never mark the book's names and terms (they are suggested first).
- **Where:** everywhere you write: notebooks, notes, comments, voice-note text, sticky notes, markup text boxes and text corrections in *Edit pages*.

## How it works

- **New crate `libreri-spell`** reads Hunspell dictionaries with spellbook (pure Rust, MPL-2.0).
  - `catalog`: the dictionary list, download and remove.
  - `text`: finds words, skipping Markdown code, maths, link targets, addresses, numbers, acronyms and CamelCase names. Places are counted in UTF-16 units, as the interface counts them.
  - `vocab`: words with how often each appears, found by their beginning.
  - `Checker`: marks misspelt words and suggests corrections.
- **A word is accepted when:**
  - a chosen dictionary knows it,
  - it is in the person's own dictionary,
  - or the book being written about uses it at least twice ("Harte's" counts when "Harte" does).
- **Dictionaries:**
  - The English ones are compiled into the app (SCOWL-based, MIT/BSD).
  - Others download into the app's data folder (`dictionaries/`, per computer) from wooorm/dictionaries, or from LibreOffice/dictionaries for Hindi and Bengali. GitHub is tried first, then the jsDelivr mirror; `LIBRERI_DICTIONARY_URL` can point elsewhere.
  - A download is checked by loading it before it is kept. Each dictionary keeps its own licence (GPL, LGPL, MPL or MIT), and none is shipped with Libreri.
- **Your dictionary:** `Notes/<profile>/Dictionary.txt`, one word per line. It is a plain file in the notes folder, so it travels in backups and archives and can be edited by hand.
- **Learning words:**
  - A book's words come from its text, with OCR text for scanned pages. The notes' words come from the profile's Markdown notes (up to 20 MB, relearned every five minutes).
  - Both are learned in the background and kept in memory for the last four books. Checking never waits for them: until they are ready, only the dictionaries are used.
- **Settings › Writing (the profile's):** spell check on or off, word completion on or off, and up to four languages checked together. It also downloads and removes dictionaries (this computer), and lists and removes words in your dictionary.

## In the interface

- **Marking words:** a text box cannot draw underlines, so a layer with the same text and layout (the text is invisible) sits over it and draws a red wavy line under each misspelt word. It follows scrolling and resizing. While typing, marks move with their words until the text is checked again, a third of a second later. The system's own spell check is turned off in that box.
- **Corrections:** right-click a marked word, or press Mod+; for the word at the cursor. The menu offers corrections (keeping capitals), *Add to your dictionary* and *Ignore* (until Libreri closes). A correction is typed with `insertText`, so undo works.
- **Completing words:** after three letters, a small list offers ways to finish the word.
  - Ranking: your own words first, then the open book's and your notes' (by how often they appear), then the dictionary's.
  - Tab takes the first; the arrow keys choose another; Esc closes the list.
- **Code:** `attachSpell(textarea)` works on any text box, including markup text boxes made outside React (`MarkupEvents.onEditorOpen`). `useSpell` and `SpellTextarea` are the React forms.

## Consequences

- **Hindi and Bengali** come from LibreOffice's dictionaries. Other Indian languages are not offered yet.
- **Dictionary words for completing** are the dictionary's word stems. Inflected forms come from books and notes.
- **The text box only:** Markdown previews and rendered notes are not checked.
- **New dependency:** spellbook (MPL-2.0).
