# 31. Reading calendar and study timer

Status: accepted (2026-10-02). Built.

## Decisions (user, 2026-10-02)

- **A reading calendar, not a general calendar:** days read, timer sessions, reading goals and due dates. Each profile has its own.
- **Timer:** focus sessions (25 minutes of reading, a 5-minute break, a 15-minute break every 4 rounds; all can be changed), a plain countdown and a stopwatch.
- **Timer sessions count as reading** the open book: minutes and pages per book, a streak and monthly totals. This starts the reading statistics planned for version 2.
- **Where:**
  - **Calendar** is a section in the sidebar (Mod+Shift+8).
  - **The timer** is a clock in the reader's toolbar.
  - **While a timer runs, it also shows outside the window:**
    - **macOS:** in the menu bar, plus progress on the Dock icon.
    - **Windows:** progress on Libreri's taskbar button, and a tray icon.
    - **Linux:** a tray icon, with the time where the desktop shows text.
    - In each place there is a menu with Pause, Skip, Stop and Open Libreri.

## How it works

- **Storage:**
  - Each profile's calendar is one JSON document in `.library-data/profiles/<profile>.study.json`, so it is in backups and survives a rebuild of the index.
  - The app owns its shape (sessions, goals, focus lengths, options). The library only checks it is a JSON object under 8 MB.
  - Guests keep none.
- **Sessions** record:
  - their start, minutes and kind;
  - the book;
  - the page at the start and at the end, when the same book is still open.
- **Goals:**
  - **Finish:** a book up to a page by a date. The reader moves the goal on as the book is read. Books without pages, such as EPUB, use how far through they are. Each goal gets a pace: pages a day to finish on time, a behind warning, and overdue.
  - **Due:** something to have read by a day, with an optional book.
- **Timer state:**
  - The timer lives in the window (zustand) and keeps going across books and the library.
  - At the end of a focus stretch it:
    - chimes;
    - saves the session;
    - pauses read aloud and any audio, when that option is on;
    - shows a break card with the minutes and pages read. The card can save a takeaway to the book's notebook, then the break starts when asked.
- **System clock:** the command `timer_tray` updates the menu bar, taskbar or tray once a second, using Tauri's `tray-icon` feature and `set_progress_bar`. Menu choices come back as the `TimerTrayAction` event.
- **Export:** an .ics file, with goals as all-day events and sessions as timed events, for Apple Calendar, Google Calendar or Outlook. There is no two-way sync.

## Not yet

- Archives (`.libreri`) do not carry the calendar; backups do.
- The timer belongs to the main window; book windows do not show it.
