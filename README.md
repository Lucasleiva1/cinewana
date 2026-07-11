# CINE WANA

CINE WANA is a private Windows media library for movies and TV series stored on local, external, or network drives. The application indexes files without modifying them and keeps its database and cache in the Windows application-data directory.

## Development

Requirements are checked by `scripts/setup-windows.ps1`. It installs/project-provisions pnpm and validates Microsoft C++ Build Tools, WebView2, FFmpeg/ffprobe, and libmpv.

```powershell
powershell -ExecutionPolicy Bypass -File scripts/setup-windows.ps1
pnpm install
pnpm desktop:dev
```

## Verification

```powershell
pnpm typecheck
pnpm test
cargo test --workspace
pnpm desktop:build
```

The initial media root is `D:\peliculas-y-series`. It is only seeded for a new database and can be replaced from Settings. CINE WANA never moves, deletes, renames, or rewrites media files.

