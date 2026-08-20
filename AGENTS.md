# CINE WANA project rules

- The product name is always **CINE WANA**. Do not add animals to the identity.
- The desktop host targets Windows x64. The current phase also permits a local-network remote-control PWA, QR pairing, and authenticated WebSocket/HTTP services hosted by the Windows app. Do not add native Android/iOS apps or video streaming.
- Never rename, move, delete, rewrite, or transcode an original media file. The only writable area inside a configured media root is CINE WANA's own `.cinewana` metadata directory; no other media-root content may be changed.
- React components consume typed IPC contracts; filesystem, database, scanner, metadata, and player behavior belongs in Rust services.
- UI/API-facing DTOs must use opaque IDs. Physical paths are only exposed by the explicit local technical-details command.
- Invoke external programs with argument arrays. Do not concatenate shell commands from paths or user input.
- Every visible control must be connected to working behavior or be hidden.
- Update `docs/ROADMAP.md` after completing a phase and run the relevant tests before committing.
