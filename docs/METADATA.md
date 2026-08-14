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

These locations are independent of the installer directory, so an update or reinstall to a different folder under the same Windows user reuses the catalog and artwork without another TMDB request.

For another Windows computer, close CINE WANA and copy the complete `%APPDATA%\com.cinewana.app` directory to the same application-data location on the destination computer. Cache paths are rebased automatically at startup. If the video library has a different physical location, select the new root and rescan; original media files remain read-only. Copying only the database without the cache is detected and reported in the review queue as missing artwork.

## Credential handling

Development and release builds read `TMDB_READ_ACCESS_TOKEN` or `TMDB_API_KEY` from the ignored root `.env` file through `scripts/run-tauri.ps1`. The build can retain the configured credential for the installed desktop client, while the plaintext `.env` file is never committed or bundled. Credentials must never be logged or placed in source-controlled configuration.

TMDB API requests follow the official search-then-details workflow. Image URLs use TMDB's documented image base and fixed poster/backdrop sizes; the resulting bytes are copied into the private persistent cache before the database is marked imported. Imported TMDB records become eligible for refresh after 180 days so the local cache remains within TMDB's current API terms.
