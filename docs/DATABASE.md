# Database

SQLite is stored below the platform application-data directory and opened in WAL mode with foreign keys enabled. Migrations create roots, media hierarchy, files, tracks, local accounts, per-account progress, per-account lists, per-account history, heroes, image profiles, settings, and scan jobs. A completed reconciliation marks unseen files offline instead of deleting account state.

Local accounts use a display name plus a 4-10 character alphanumeric password. Passwords are stored as salted SHA-256 hashes. Legacy global progress/list/history rows are copied into the first local account created so existing development data keeps its resume state.

Title metadata is stored on `media_items`: description, inferred or manual genres, cast, poster path, backdrop path, and a manual-metadata flag. Manual edits are protected from scan title/genre rewrites, while missing poster/backdrop paths can still be filled by later artwork generation. Detail pages derive local recommendations by comparing kind, series, year proximity, and shared genres.

TMDB imports add `metadata_status`, source URL, import/check timestamps, candidate matches, and the portable `metadata.json` path. The private application cache remains available for fast artwork generation and downloads, but every identified video is also assigned a stable `portable_id` and exported below the containing folder's `.cinewana/items/<portable_id>/` directory with its metadata, poster, and backdrop. Original video, subtitle, and user-owned files stay read-only. The disabled Wikipedia importer remains available in code only as a rollback provider.

Scanner identification also stores its source, review state, review reason, and whether the final classification was chosen manually. A manual movie/episode decision is protected from later automatic scans. During reconciliation the portable package wins over stale SQLite fields; if the database is empty, scanning the copied folder rebuilds the catalog from those packages. SQLite remains the disposable high-speed index plus local account progress, favorites, history, and settings.
