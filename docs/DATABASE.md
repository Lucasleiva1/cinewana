# Database

SQLite is stored below the platform application-data directory and opened in WAL mode with foreign keys enabled. Migrations create roots, media hierarchy, files, tracks, local accounts, per-account progress, per-account lists, per-account history, heroes, image profiles, settings, and scan jobs. A completed reconciliation marks unseen files offline instead of deleting account state.

Local accounts use a display name plus a 4-10 character alphanumeric password. Passwords are stored as salted SHA-256 hashes. Legacy global progress/list/history rows are copied into the first local account created so existing development data keeps its resume state.

Title metadata is stored on `media_items`: description, inferred or manual genres, cast, poster path, backdrop path, and a manual-metadata flag. Manual edits are protected from scan title/genre rewrites, while missing poster/backdrop paths can still be filled by later artwork generation. Detail pages derive local recommendations by comparing kind, series, year proximity, and shared genres.

Wikipedia imports add `metadata_status`, source URL, import/check timestamps, candidate matches, and the cached `metadata.json` path. CINE WANA does not write sidecar files into media folders; original movie/series roots stay read-only, and metadata JSON files are stored under the application cache by media fingerprint.
