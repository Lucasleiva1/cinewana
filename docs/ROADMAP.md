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
- Moved internal-player image scanning to Rust/FFmpeg so scene, light, and color analysis works with local Tauri media URLs without browser canvas security errors
- Image scanning now pauses playback, reports live scan progress, and resumes from the original playback position when finished
- Reworked shadows/highlights adjustment as a tonal curve so shadow correction no longer washes the whole frame white
- Added automatic numbered movie-sequel handoff for local sagas, with a cancellable next-up prompt near the end of playback
- Prepared version 0.2.0 as an important signed updater release for the internal-player image correction workflow

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
- Prepared version 0.1.3 as the signed updater release for carousel navigation and automatic sequel handoff
- Prepared version 0.1.4 to keep carousel drag navigation while preserving normal card click/play behavior
- Prepared version 0.1.5 to auto-hide internal player controls during playback and use native fullscreen
- Fixed desktop window startup, clickable series cards, bottom-safe browsing space, and hidden external scan helper windows
- Added responsive desktop window modes down to compact floating playback sizes
- Prepared version 0.1.6 as the responsive desktop-window updater release

## Implemented for richer title pages

- Artwork generation now keeps poster/backdrop frames even when an eight-second video preview cannot be encoded
- Scanner tries several timestamps so short or difficult videos still get a usable still image when possible
- Detail pages now include editable description, year, genres, cast, poster, and backdrop
- Title cards keep center play behavior, while the title text opens the full title page
- Added local genre-based recommendations and starter genre inference from filenames/paths
- Added a no-key Wikipedia metadata importer in the Rust backend, with Spanish-first lookup, English fallback, cast extraction, source URL, cached metadata JSON, manual retry, and ambiguity selection

## Implemented for resilient series identification

- Series scanning now combines the video filename with `Serie / Temporada` folder context instead of requiring a perfect `SxxExx` filename
- Season folders accept `Temporada`, `Season`, and `Sxx`; episode files accept `SxxExx`, `1x01`, `Episodio`, `Episode`, `Capitulo`, `Chapter`, and leading episode numbers
- Folder context has priority when a localized series folder conflicts with a filename, while the contradiction is sent to a review queue
- Settings now includes an identification review queue for confirming a movie or manually assigning series, season, episode, and episode title
- Review items can open Windows Explorer with the original file selected, while paths remain resolved only by the Rust backend
- Each review item can rescan only its original folder, reconnect a renamed video by fingerprint, and remove the warning immediately when the new name is unambiguous
- Manual identification decisions survive later scans and rebuild the SQLite series/season/episode hierarchy without renaming or moving source media
- Series cards now open a series view grouped by season with playable episode rows instead of opening an arbitrary episode directly
- Episode rows display a generated video thumbnail, play/detail actions, watch progress, and a short stored description when available
- Unchanged files are reconciled from size and modification time before FFprobe/fingerprint work, avoiding repeated full analysis and duplicate catalog records
- CINE WANA writes lightweight identification/description JSON manifests only to its private application cache; configured media roots remain read-only
- Home keeps recently added movies chronological while rotating the main movie shelf and featured titles once per local calendar day

## Pending after the functional media pass

- File-system watcher/debounce and scan concurrency tuning
- Local NFO/images, optional TMDB official posters, richer online posters, and manual match correction
- Fine-grained audio/subtitle selectors and persisted image profiles
- Final visual refinements after the functional player is reviewed
- Backup/export diagnostics and hardening

## In progress: local remote control PWA

- Prepared version 0.2.1 as the installable Windows x64 test update that exposes **Configuración → Control remoto** in the desktop application
- Prepared version 0.3.0-rc.1 as the important pre-final updater release for resilient series identification, targeted rescans, daily discovery rotation, and the local remote-control PWA
- Windows-hosted HTTP/WebSocket service for same-Wi-Fi control
- QR and manual-URL pairing with approved, revocable device tokens
- Mobile web remote for the internal player and existing library actions
- Remote library mirrors the desktop movie rotation and exposes series as series → seasons → episodes, with episode artwork, description, details, and play actions
- Offline-capable PWA shell, responsive Android layout, reconnection, and local security hardening
- Validation on desktop first, followed by same-network mobile testing before commit or packaging

## Deferred

- Native Android/iOS applications, video streaming, cloud APIs, and store distribution
