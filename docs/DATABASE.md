# Database

SQLite is stored below the platform application-data directory and opened in WAL mode with foreign keys enabled. Migrations create roots, media hierarchy, files, tracks, progress, lists, history, heroes, image profiles, settings, and scan jobs. A completed reconciliation marks unseen files offline instead of deleting their user state.

