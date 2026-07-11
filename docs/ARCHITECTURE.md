# Architecture

The React presentation layer uses typed Tauri IPC. Rust owns domain models and application services. SQLite is the durable source of truth; media roots are read-only inputs. Scanner and metadata work run away from the UI thread and publish progress events.

`LibraryService` coordinates the database and catalog. `ScannerService` discovers and reconciles files. `MetadataService` resolves local/TMDB/generated data. `PlayerService` wraps libmpv. `ProgressService`, `SettingsService`, and `ImageProfileService` persist user state. `FutureNetworkService` is an unimplemented boundary reserved for a later local server.

All public DTOs use opaque UUIDs. Physical paths remain backend-only except for the explicit local technical details view. Player commands and state are serializable so a future transport can reuse them unchanged.

