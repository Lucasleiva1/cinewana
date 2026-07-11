# CINE WANA project rules

- The product name is always **CINE WANA**. Do not add animals to the identity.
- This stage targets Windows x64 only. Do not add Android, iOS, streaming, QR, or remote-control implementations.
- Treat every configured media root as read-only. Never rename, move, delete, rewrite, or transcode an original media file.
- React components consume typed IPC contracts; filesystem, database, scanner, metadata, and player behavior belongs in Rust services.
- UI/API-facing DTOs must use opaque IDs. Physical paths are only exposed by the explicit local technical-details command.
- Invoke external programs with argument arrays. Do not concatenate shell commands from paths or user input.
- Every visible control must be connected to working behavior or be hidden.
- Update `docs/ROADMAP.md` after completing a phase and run the relevant tests before committing.

