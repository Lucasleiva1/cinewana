# CINE WANA project rules

- The product name is always **CINE WANA**. Do not add animals to the identity.
- The desktop host targets Windows x64. The current phase also permits a local-network remote-control PWA, QR pairing, and authenticated WebSocket/HTTP services hosted by the Windows app. Do not add native Android/iOS apps or video streaming.
- Never rewrite, transcode, or delete an original media file. The only writable area inside a configured media root is CINE WANA's own `.cinewana` metadata directory, with one deliberate exception: the `peliculas nuevas` tray. Titles dropped there are moved once into the movies folder by `apps/desktop/src-tauri/src/ingest.rs` and never touched again. The owner asked for this on purpose so that opening the application stops costing a full library scan; do not "restore" the old rule by removing the move. Nothing outside that tray may be renamed, moved, or deleted.
- React components consume typed IPC contracts; filesystem, database, scanner, metadata, and player behavior belongs in Rust services.
- UI/API-facing DTOs must use opaque IDs. Physical paths are only exposed by the explicit local technical-details command.
- Invoke external programs with argument arrays. Do not concatenate shell commands from paths or user input.
- Every visible control must be connected to working behavior or be hidden.
- Update `docs/ROADMAP.md` after completing a phase and run the relevant tests before committing.
