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

## Pending after the functional media pass

- File-system watcher/debounce and scan concurrency tuning
- Local NFO/images, optional TMDB official posters, and manual match correction
- Fine-grained audio/subtitle selectors, progress polling, and image profiles
- Final visual refinements after the functional player is reviewed
- Backup/export diagnostics and hardening
- NSIS installer (explicitly postponed until after the development review)

## Deferred

- Android, API, WebSocket, streaming, QR, remote control
