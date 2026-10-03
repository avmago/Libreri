# 2. SQLite for the library database

Status: accepted.

Each library keeps one SQLite file in `.library-data/library.db`. It needs no server, travels with the library folder, works in synced folders (with the library lock) and has built-in full-text search (FTS5). PostgreSQL would only be used by a future sync server, never by the desktop app.
