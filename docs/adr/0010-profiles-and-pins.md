# 10. Profiles, PINs and who is signed in

Status: accepted (2026-09-27)

Several people share one library. Each profile keeps its own reading status, ratings, favourites, positions, highlights, bookmarks, notebooks, smart collections and interface preferences; book files, folders and book details are shared.

## Kinds of profile

| Kind | Reads and makes notes | Changes the library | Manages profiles | Data kept |
|---|---|---|---|---|
| Owner (one per library) | yes | yes | yes | yes |
| Standard | yes | yes | no | yes |
| Kids | only in folders the owner chooses | no | no | yes |
| Guest | yes | no | no | no, forgotten when the guest leaves |

## Who is signed in lives in the library session

The signed-in profile is part of the open `Library` (`Library::sign_in`, `Library::sign_out`), not something the interface passes to each command. The database functions still take an explicit `ProfileId` (code-structure rule 7), but the commands the interface calls cannot choose whose data they read. If the interface could pass any profile id, a PIN would protect nothing. The same session decides what Kids may see: book lists, facets, folders, single books and the `book://` protocol are all limited to their folders, and the protocol never serves `Notes/`, backups or the database.

A library opens straight into the only profile when there is one and it has no PIN; otherwise the profile picker shows. Locking (Mod+Shift+P, the profile menu, or auto-lock after the chosen idle time, 15 minutes by default) signs out and returns to the picker in every window.

## PINs

- Exactly 6 digits. Runs (123456, 654321), a repeated digit (000000) and two alternating digits (121212) are refused.
- Stored as Argon2id PHC strings with a random salt (`libreri-profiles`).
- After 5 wrong PINs the profile waits 30 seconds, after 10 wrong PINs 5 minutes, and after 15 an hour. The count survives restarting the app.
- The owner can set, change or remove anyone's PIN. Everyone else changes their own PIN with the current one.
- When the owner sets a PIN they get a one-time recovery code (16 characters, about 80 bits). The code resets the owner's forgotten PIN and is then replaced.

PINs keep people who share a computer out of each other's notes. They are **not encryption**: anyone who can read the library folder can read the book files and the Markdown notebooks, and a 6-digit PIN hash can be guessed offline. The Settings page says so. Encrypting a profile's notes is a possible later option.

## Files

- `.library-data/profiles/<id>.json`: the profile (name, colour, kind, PIN and recovery hashes, Kids folders, preferences).
- `.library-data/profiles/<id>.collections.json`: smart collections.
- `.library-data/annotations/<profile>/<book>.json` (format 2): status, rating, favourite, progress, position and annotations for one book. Format 1 files (only the list of annotations) still load.
- `Notes/<profile name>/`: notebooks. Renaming a profile renames the folder; removing a profile moves it to the system Trash.

A rebuilt database restores all of these, so profiles, PINs and personal data survive "Rebuild library index". Guests write no backups.
