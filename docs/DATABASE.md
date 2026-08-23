# Database

SQLite is stored below the platform application-data directory and opened in WAL mode with foreign keys enabled. Migrations create roots, media hierarchy, files, tracks, local accounts, per-account progress, per-account lists, per-account history, heroes, image profiles, settings, and scan jobs. A completed reconciliation marks unseen files offline instead of deleting account state.

Local accounts use a display name plus a 4-10 character alphanumeric password. Passwords are stored as salted SHA-256 hashes. Legacy global progress/list/history rows are copied into the first local account created so existing development data keeps its resume state.

Title metadata is stored on `media_items`: description, inferred or manual genres, cast, poster path, backdrop path, a manual-metadata flag, and the collection the title belongs to. Manual edits are protected from scan title/genre rewrites, while missing poster/backdrop paths can still be filled by later artwork generation. Detail pages derive local recommendations by comparing kind, series, year proximity, and shared genres.

TMDB imports add `metadata_status`, source URL, import/check timestamps, candidate matches, and the portable `metadata.json` path. The private application cache remains available for fast artwork generation and downloads, but every identified video is also assigned a stable `portable_id` and exported below the containing folder's `.cinewana/items/<portable_id>/` directory with its metadata, poster, and backdrop. Original video, subtitle, and user-owned files stay read-only. The disabled Wikipedia importer remains available in code only as a rollback provider.

Scanner identification also stores its source, review state, review reason, and whether the final classification was chosen manually. A manual movie/episode decision is protected from later automatic scans. During reconciliation the portable package wins over stale SQLite fields; if the database is empty, scanning the copied folder rebuilds the catalog from those packages. SQLite remains the disposable high-speed index plus local account progress, favorites, history, and settings.

## Categories, sagas, and shelf order

Raw provider genres are normalized on read into one canonical Spanish vocabulary, so `Suspense`,
`Action & Adventure`, and `Sci-Fi & Fantasy` stop producing duplicate or English shelves. Every
catalog row therefore carries its canonical categories plus an `incomplete` flag raised when the
sheet has no genre or no synopsis. A title whose canonical genre list is empty is shelved under
`Sin categoría`, which guarantees that no movie is ever left off every shelf; a title that has a
genre but no synopsis keeps its genre shelf **and** joins the repair queue.

Sagas come from the TMDB `belongs_to_collection` field, stored on `media_items` as `saga_id`,
`saga_title`, and `saga_position`, and exported in the portable `metadata.json` so collections
survive moving the drive. Movies TMDB never identified are grouped by a conservative title
heuristic that requires a shared base, two distinct parts, and at least one explicit part marker,
which keeps unrelated same-prefix titles and duplicate files from inventing a saga.

Beyond the automatic shelves, an account can create its own and fill them by hand. Custom shelves
live under the `custom_categories:<account_id>` settings key and reference movies by media id and
series by title, because a show's identity in the catalog is its name while the episode standing in
for it changes as newer ones arrive. Assigning a title by hand never removes it from its genre
shelf, and deleting a custom shelf leaves every title untouched. Series also get one shelf holding
every show; the per-genre series shelves still exist but start hidden, listed in Configuración
ready to be switched back on.

Shelves are rebuilt from the catalog on every home load, so a scan reshelves new titles without a
migration. The visible order and per-shelf visibility are saved per account under the
`category_order:<account_id>` settings key. Shelves the account never sorted keep the default
order — the account's own shelves first, then science fiction, sagas and the movie genres by size,
with every series on one shelf just above the repair queue, which stays last —
and land after the saved ones, so a genre arriving with a later scan never disturbs a hand-made
arrangement.

## Credited people and their photos

Each title stores its direction, writing and billed cast in `people_json` on `media_items`. The
cast is capped at ten because the provider answers ordered by billing and the tail is unbounded:
nine names for `Alien`, a hundred and three for `Avengers: Endgame`. Only Director, Screenplay,
Writer and Story are taken from the crew; the rest of a film's credits carry no photo and no
meaning outside the industry.

Photos are cached under `<cache>/tmdb/profiles/` named after the provider file, so one actor is
stored once no matter how many titles credit them. The portable package instead copies each photo
into `.cinewana/items/<portable_id>/cast/`, duplicating a face across every movie that credits it
— deliberately, because each folder has to stand on its own for the library to show faces on a
computer that never saw this metadata. At about ten kilobytes per photo, a full library of two
hundred and seventy titles spends roughly thirty-three megabytes on the whole cast.

Titles imported before this existed keep an empty list and fall back to the plain name list in
`cast_json`. The bulk refresh in Configuración fetches every sheet again, one title at a time to
stay within the provider's rate limits, and can be cancelled without losing what it already did.
