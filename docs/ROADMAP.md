# Roadmap

## Completed for the development preview

- Windows/Tauri monorepo and reproducible pnpm/Rust toolchain
- SQLite migrations and default `D:\peliculas-y-series` root
- Read-only recursive scan, incremental reconciliation, cancellation, Unicode names, movie/episode parsing, and external subtitle association
- Real-data Home, catalog, series grouping, search, favorites, Mi lista, detail, and Library settings
- Native Tauri development launch and strict frontend/Rust checks

## Implemented after the first visual review

- Joined the two-color CINE/WANA wordmark without a visual gap across desktop, account access, compact layouts, and the remote control
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
- Added left/right double-click seeking in cumulative 10-second steps, press-and-hold 2× playback, and a visible 0%-to-100% control whose upper half reaches 200% of the original media level
- Routes one media signal through Web Audio after the visible 50% safety detent, avoiding delayed parallel playback or echo while scaling up to 2× gain
- Served original media read-only through a tokenized loopback URL with byte-range and CORS support so Windows WebView2 can amplify audio without muting it
- Added non-destructive image scanning and runtime adjustments for brightness, contrast, saturation, shadows, highlights, and temperature
- Moved internal-player image scanning to Rust/FFmpeg so scene, light, and color analysis works with local Tauri media URLs without browser canvas security errors
- Image scanning now pauses playback, reports live scan progress, and resumes from the original playback position when finished
- Image analysis now runs only once per active playback session, reopens instantly with its existing measurements and adjustments, and is discarded when that title leaves the player without writing analysis residue to disk
- Hid the automatic suggested image adjustment after playback review found its shadow correction too aggressive; measurements and manual controls remain available, and the implementation is retained behind an internal flag for later recalibration
- Opening Image now starts its scan automatically from both the desktop player and the local-network remote control
- Reworked shadows/highlights adjustment as a tonal curve so shadow correction no longer washes the whole frame white
- Added automatic numbered movie-sequel handoff for local sagas, with a cancellable next-up prompt near the end of playback
- The next-content offer now appears during the final 60 seconds with explicit **Play now** and **Cancel** actions on both desktop and remote; cancellation hides the offer and disables autoplay
- The next-content offer no longer pins the rest of the player interface: after mouse inactivity, the top bar, playback controls, central play button, and pointer disappear while the recommendation remains visible; moving the mouse reveals the controls again
- Series now advance to the next numbered episode across seasons, while movies recommend a genre/year-related title with no progress in the active account
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
- Replaced the active Wikipedia flow with TMDB while retaining the Wikipedia client disabled for rollback
- Added persistent official TMDB posters, backdrops, localized details, genres, and cast, with movie and series/episode-specific lookup
- Added poster-first ambiguity review in Settings, including candidate cover previews, missing-title errors, manual correction, and retry
- Poster candidates in Settings now apply on click without opening the large title view, expose an explicit **Usar esta portada** action, confirm the saved change, and keep any separate name/classification correction visible until it is resolved
- Every movie and episode detail now includes a compact **Revisión** entry that opens a focused correction window for that title, without requiring the user to find it again in the global Settings list
- Individual review now keeps alternative-language TMDB matches available for manual confirmation (for example, a local Spanish title matching its English TMDB title), preserves the user's visible title when applying that match, and offers multiple official posters
- Individual review accepts a user-selected JPG, PNG, or WebP poster/background; CINE WANA copies it into its private cache and the title's portable `.cinewana` package without modifying the original video or the source image
- Made TMDB metadata and artwork reusable without online lookup after import, independent of the Windows install directory, with cache-path rebasing for a transferred application-data directory
- Added the official TMDB attribution and a 180-day refresh eligibility window for compliance with TMDB's cache terms

## Implemented for resilient series identification

- Series scanning now combines the video filename with `Serie / Temporada` folder context instead of requiring a perfect `SxxExx` filename
- Any video below an exact `SERIE` or `SERIES` container is forced to series classification; season folders set the season and simple embedded numbers identify episodes
- Any video below `PELÍCULAS` or `PELICULAS` is forced to movie classification, even when its filename resembles a television episode
- Season folders accept `Temporada`, `Season`, and `Sxx`; episode files accept `SxxExx`, `1x01`, `Episodio`, `Episode`, `Capitulo`, `Chapter`, and leading episode numbers
- Folder context has priority when a localized series folder conflicts with a filename, while the contradiction is sent to a review queue
- Settings now includes an identification review queue for confirming a movie or manually assigning series, season, episode, and episode title
- Review items can open Windows Explorer with the original file selected, while paths remain resolved only by the Rust backend
- Each review item can rescan only its original folder, reconnect a renamed video by fingerprint, and remove the warning immediately when the new name is unambiguous
- Manual identification decisions survive later scans and rebuild the SQLite series/season/episode hierarchy without renaming or moving source media
- Series cards now open a series view grouped by season with playable episode rows instead of opening an arbitrary episode directly
- The library footer now separates movie, series, and chapter totals; series cards show chapter totals with a per-season breakdown, and every season header reports its own chapter count
- Episode rows display a generated video thumbnail, play/detail actions, watch progress, and a short stored description when available
- Unchanged files are reconciled from size and modification time before FFprobe/fingerprint work, avoiding repeated full analysis and duplicate catalog records
- Changing the library folder now preserves one root identity instead of adding a second catalog, hides inactive roots from every user-facing query, and consolidates legacy cross-disk duplicates by fingerprint while retaining progress, flags, history, manual decisions, and cached metadata
- Every scanned title now receives a stable portable identity and a `.cinewana/items/<id>/metadata.json` package beside its video, including identification, manual corrections, TMDB state, poster, and backdrop; SQLite remains the fast rebuildable cache and original videos remain untouched
- Home keeps recently added movies chronological while rotating the main movie shelf and featured titles once per local calendar day
- Home groups the library into categories: a sticky name strip under the hero preview, one shelf per canonical genre, separate shelves for series, a saga shelf built from TMDB collections plus a conservative title heuristic, and a `Sin categoría` repair queue that guarantees every movie is shelved somewhere
- Category order and visibility are chosen per account from Configuración by dragging one list that governs both the name strip and the shelves, defaulting to science fiction first and appending later genres at the end instead of reshuffling a hand-made order
- Accounts can create their own categories and assign movies and series to them from each title's sheet; a single `Series` shelf holds every show while the per-genre series shelves start hidden instead of being removed
- Reordering categories is a grab-and-carry gesture that reorders live and saves on release, replacing the up/down buttons
- Every category carries its own icon, and the name strip ships in two looks selectable per account from Configuración: `gold`, the default, paints the whole row in the brand gold, while `dark` keeps neutral chips and gilds only the icons
- The remote control mirrors the same categories in the same order, flattening sagas into a single shelf for the phone
- Title sheets show direction, writing and the ten billed actors with their photo and the character they play, falling back to initials when the provider has no picture
- Cast photos are copied into each title's portable `.cinewana` folder, so faces survive moving the drive to a computer that never saw the metadata
- Configuración can refresh every sheet in one pass, with visible progress and a cancel, so titles imported before a feature existed catch up
- Movie title sheets once again occupy the complete CINE WANA window, keep their close control fixed, hide native Windows scrollbars, and open cast photos in a centered, keyboard-dismissable viewer whose close control sits directly on the photo

## Opening the application no longer costs a library scan

- A `peliculas nuevas` tray sits beside the finished `PELICULAS` and `SERIES` folders. Startup looks only at that tray, so an ordinary launch with nothing new touches neither the disk nor the database
- Each movie left in the tray is moved once into the movies folder and then processed there: technical data, generated artwork, TMDB sheet, cast, and portable `.cinewana` package. A title that cannot be identified is moved anyway and surfaces through the existing identification-review flow, so the tray always ends up empty
- A loose video file gets its own folder to match the shape of the existing library, and its external subtitles travel with it. An existing title is never overwritten
- Series stay manual on purpose: they are dropped straight into `SERIES` and picked up by the Reescanear button, which still rebuilds every sheet
- The automatic pass no longer rewrites the identification, portable package, or metadata of files that did not change; only the manual rescan does
- Titles TMDB has no cast for are no longer re-queried on every launch — the stored check date is now honoured for thirty days
- The scan owns a second SQLite connection, so browsing the library stays responsive while a pass runs
- A full pass still runs every five days to catch what was deleted or moved from the Explorer, in the background, and reports what it found when it finishes

## Pending after the functional media pass

- File-system watcher/debounce and scan concurrency tuning
- Local NFO/images and optional manual image overrides beyond the completed TMDB poster workflow
- Fine-grained audio/subtitle selectors and persisted image profiles
- Final visual refinements after the functional player is reviewed
- Backup/export diagnostics and hardening

## In progress: local remote control PWA

- Prepared version 0.3.11 as the signed updater release for the `peliculas nuevas` tray: startup stops scanning the finished library, new movies are processed once and moved into place, unchanged titles are left alone, the TMDB re-query loop is closed, browsing no longer waits behind the scan, and a background pass every five days still reports what it found
- Added a release-build guard that blocks Windows installers without a configured TMDB credential, forces Cargo to rebuild when that credential changes, and documented the 0.3.10 installer incident and recovery checklist
- Prepared version 0.3.10 as the signed updater release for full-screen title sheets, cast and director photos, complete metadata rescanning, portable person artwork, draggable-shelf preferences, and a fixed mobile remote-control layout
- The mobile player keeps its progress, transport, volume, and quick-control geometry mounted from the first snapshot; connection updates only replace values, and next-content renders below the fixed volume area
- The internal player now opens the same complete Settings view in a side panel matching the Image panel width, so playback can continue while remote control and other options are managed
- Remote-control Settings now includes a persistent **Siempre activo** switch that starts the authenticated local service automatically when CINE WANA opens while preserving the manual activate/deactivate button
- Prepared version 0.3.6 as the signed updater release for in-player access to the shared Settings view and persistent automatic remote-control startup
- Prepared version 0.3.7 as the signed updater release for persistent TMDB artwork and metadata, poster-first identification review, player chrome inactivity fixes, and complete movie/series/chapter counts
- Prepared version 0.3.8 as the signed updater release for portable `.cinewana` metadata packages, individual title correction with local artwork, and one-pass image analysis per playback session
- Prepared version 0.3.9 as the signed updater release for the category system: canonical genres, sagas, custom categories, per-account order and strip style, plus the remote-control fixes for artwork loading and the volume slider

- Prepared version 0.2.1 as the installable Windows x64 test update that exposes **Configuración → Control remoto** in the desktop application
- Prepared version 0.3.0-rc.1 as the important pre-final updater release for resilient series identification, targeted rescans, daily discovery rotation, and the local remote-control PWA
- Prepared version 0.3.1 as the signed pre-final updater release that makes `SERIES` and `PELÍCULAS` explicit, mandatory classification containers
- Prepared version 0.3.2 as the signed pre-final updater release for cancellable next-episode and unwatched-movie handoff
- Prepared version 0.3.3 as the signed updater release for automatic image analysis, immediate/cancellable 60-second next-content handoff on desktop and remote, and the joined two-color wordmark
- Prepared version 0.3.4 as the signed updater release for cumulative 10-second double-click seeking, press-and-hold 2× playback, and the protected 100%-to-150% volume boost
- Prepared version 0.3.5 as the signed updater release with a single non-echoing audio path, visible 0%-to-100% volume mapped up to 2× gain, and loopback media delivery compatible with Windows WebView2
- Windows-hosted HTTP/WebSocket service for same-Wi-Fi control
- QR and manual-URL pairing with approved, revocable device tokens
- Mobile web remote for the internal player and existing library actions
- Remote library mirrors the desktop movie rotation and exposes series as series → seasons → episodes, with episode artwork, description, details, and play actions
- Offline-capable PWA shell, responsive Android layout, reconnection, and local security hardening
- Validation on desktop first, followed by same-network mobile testing before commit or packaging

## Deferred

- Native Android/iOS applications, video streaming, cloud APIs, and store distribution
