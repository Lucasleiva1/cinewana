# Roadmap

## Completed for the development preview

- Windows/Tauri monorepo and reproducible pnpm/Rust toolchain
- SQLite migrations and default `D:\peliculas-y-series` root
- Read-only recursive scan, incremental reconciliation, cancellation, Unicode names, movie/episode parsing, and external subtitle association
- Real-data Home, catalog, series grouping, search, favorites, Mi lista, detail, and Library settings
- Native Tauri development launch and strict frontend/Rust checks

## Implemented after the first visual review

- Installed FFmpeg/ffprobe 8.1.2 and mpv 0.41.0 for development
- Cached real poster crops, cinematic backdrops, and eight-second preview clips generated from each local video
- Asset-protocol delivery restricted to the application cache
- Play actions wired to an mpv process embedded in a dedicated full-screen CINE WANA window
- Home rows no longer duplicate individual episodes as recently added movies
- Series season count reflects distinct seasons present, not the highest season number

## Adjusted after local playback verification

- Windows Play actions now default to Windows Media Player when the installed mpv build is not compatible with the machine
- mpv remains available behind `CINE_WANA_USE_MPV=1` for future embedded-player testing

## Implemented for the internal player pass

- Play actions now open an in-app CINE WANA player first, with external playback kept as an explicit option
- Internal playback saves watch progress through the existing `watch_progress` database path for continue/resume behavior
- Added playback-only controls: play/pause, seek, restart, volume, mute, fullscreen, and close
- Added non-destructive image scanning and runtime adjustments for brightness, contrast, saturation, shadows, highlights, and temperature

## Implemented for local account profiles

- Added local account creation and login with name plus a 4-10 character alphanumeric password
- Stored local account passwords as salted hashes instead of plaintext
- Scoped continue/resume progress, completion, history, favorites, and Mi lista to the active account
- Added a first-run account screen and session controls in the desktop UI
- Migrated any legacy global watch state into the first local account created

## Prepared for signed GitHub updates

- Added the Tauri updater plugin, static GitHub Releases endpoint, and Windows x64 updater permissions
- Generated a local updater signing key outside the repository and embedded only the public key
- Added a manual update check/install surface in Settings
- Added release-asset documentation and a helper script for creating `latest.json` from signed NSIS artifacts
- Built the first signed Windows x64 NSIS installer and updater signature assets for version 0.1.0
- Prepared version 0.1.2 to publish a signed GitHub updater release and hide the Windows console host in installed builds

## Implemented for richer title pages

- Artwork generation now keeps poster/backdrop frames even when an eight-second video preview cannot be encoded
- Scanner tries several timestamps so short or difficult videos still get a usable still image when possible
- Detail pages now include editable description, year, genres, cast, poster, and backdrop
- Title cards keep center play behavior, while the title text opens the full title page
- Added local genre-based recommendations and starter genre inference from filenames/paths
- Added a no-key Wikipedia metadata importer in the Rust backend, with Spanish-first lookup, English fallback, cast extraction, source URL, cached metadata JSON, manual retry, and ambiguity selection

## Pending after the functional media pass

- File-system watcher/debounce and scan concurrency tuning
- Local NFO/images, optional TMDB official posters, richer online posters, and manual match correction
- Fine-grained audio/subtitle selectors and persisted image profiles
- Final visual refinements after the functional player is reviewed
- Backup/export diagnostics and hardening

## Deferred

- Android, API, WebSocket, streaming, QR, remote control
