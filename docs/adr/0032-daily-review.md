# 32. Daily review

Status: accepted (2026-10-02). Built.

## Decisions (user, 2026-10-02)

- **Scheduling:** FSRS (the algorithm modern Anki uses), through `ts-fsrs`.
- **Which highlights:** every highlight becomes a card by itself, with a switch per book (and one for the whole feature) to leave books out.
- **Three kinds of card:**
  - **Passage:** the start of the highlight; recall how it goes on.
  - **Question and answer:** the highlight's comment is the question, the passage the answer. Only when there is a comment.
  - **Cloze:** chosen words of the passage are hidden.
- **Export:** an Anki deck (`.apkg`).

## How it works

- **Where:**
  - **Daily review** is a section in the sidebar (Mod+Shift+7) with how many cards are left today.
  - The overview shows today's cards, the next 7 days, a streak, the day's limits (new cards, reviews at most, how likely to remember) and the books taking part.
  - A session shows one card at a time: Show answer (Space), then Again, Hard, Good or Easy (1–4), each with when the card comes back. Cards due again within minutes come back at the end of the session. *Open in the book* jumps to the highlight.
  - A highlight's menu in the reader has **Review card**: Passage, Q&A and Cloze (picking the words to hide), or Leave out.
  - The calendar's day shows *Review: N cards due*.
- **Storage:** each profile's review is one JSON document in `.library-data/profiles/<profile>.review.json` (settings, books left out, per-highlight choices, the schedule by card, and a log of answers), next to the calendar. It is in backups and survives a rebuild. Guests keep none.
- **Cards are not stored:** they are worked out from the highlights each time, so editing a highlight or its comment changes its cards, and deleting it removes them. Only the schedule and choices are kept, by highlight id.
- **Anki:** `libreri-export::anki` writes the legacy `collection.anki2` schema that every Anki version imports, with two note types (*Libreri*: Front, Back, Source; *Libreri Cloze*: Text, Back Extra, Source). Note ids are stable, so importing again updates notes instead of adding them twice. Tags: `libreri` and the book.

## Not yet

- Archives (`.libreri`) do not carry the review; backups do.
- Schedules do not come back from Anki.
