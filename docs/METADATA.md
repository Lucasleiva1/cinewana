# TMDB metadata and persistent artwork

CINE WANA uses TMDB as its active online metadata provider. The previous Wikipedia client remains in the Rust metadata crate for rollback, but the desktop service does not instantiate or query it.

## Import flow

- Movies use the TMDB movie search and details endpoints.
- Episodes first identify the TMDB series and then request the matching season/episode details when those numbers are available.
- Imported data includes localized title, year/date, overview, genres, cast, source URL, official poster, and backdrop or episode still.
- Exact, high-confidence matches are imported automatically. Ambiguous and missing matches are added to **Configuración → Errores y coincidencias por revisar**. Candidate posters are shown before the user chooses one.
- A generated video frame remains only as a fallback. Once a TMDB poster is stored, later media scans do not overwrite it.

## Local persistence

TMDB is consulted only while importing, manually refreshing a title, or refreshing data that has reached TMDB's six-month cache limit. Normal application startup and browsing read local data, so imported titles do not generate a request on every launch:

- `%APPDATA%\com.cinewana.app\cine-wana.db` stores the imported fields, provider state, candidates, and local artwork references.
- `%APPDATA%\com.cinewana.app\cache\tmdb\posters` stores downloaded posters.
- `%APPDATA%\com.cinewana.app\cache\tmdb\backdrops` stores downloaded backdrops and episode stills.
- `%APPDATA%\com.cinewana.app\cache\metadata` stores a JSON snapshot by media fingerprint.
- `<carpeta de la película>\.cinewana\items\<id>\metadata.json` stores the permanent portable title record.
- The same portable item directory stores the selected poster and backdrop, without modifying the video.

These locations are independent of the installer directory. SQLite makes normal startup and browsing fast, while the `.cinewana` package travels with the movie folder and can reconstruct that cache without another TMDB request.

For another Windows computer, copy each complete movie folder, including its hidden `.cinewana` directory, and select the copied library as the root. CINE WANA reads the portable records, rebuilds its local SQLite cache, and uses the included artwork. Only `.cinewana` is writable; original media files remain read-only.

## Credential handling

Development and release builds read `TMDB_READ_ACCESS_TOKEN` or `TMDB_API_KEY` from the ignored root `.env` file through `scripts/run-tauri.ps1`. The build can retain the configured credential for the installed desktop client, while the plaintext `.env` file is never committed or bundled. Credentials must never be logged or placed in source-controlled configuration.

TMDB API requests follow the official search-then-details workflow. Image URLs use TMDB's documented image base and fixed poster/backdrop sizes; the resulting bytes are copied into the private persistent cache before the database is marked imported. Imported TMDB records become eligible for refresh after 180 days so the local cache remains within TMDB's current API terms.
