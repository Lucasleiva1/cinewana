use anyhow::{Context, Result};
use chrono::{Datelike, Local, Utc};
use cinewana_core::{
    AccountDto, CatalogQuery, ClassificationUpdate, HomeDto, IdentificationReview,
    ImportedMediaMetadata, LibraryRootDto, MediaDetail, MediaKind, MediaMetadataCandidate,
    MediaMetadataUpdate, MediaSummary, MediaTechnical, MediaTrack, ParsedMediaName, RootStatus,
    SeriesSeasonSummary, SeriesSummary,
};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension, Row, params};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::Path,
};
use uuid::Uuid;

pub struct Database {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone)]
pub struct DiscoveredFile {
    pub path: String,
    pub file_name: String,
    pub file_size: i64,
    pub modified_at: i64,
    pub fingerprint: String,
    pub parsed: ParsedMediaName,
    pub technical: MediaTechnical,
    pub external_subtitles: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct UpsertOutcome {
    pub media_id: String,
    pub inserted: bool,
    pub skipped: bool,
}

#[derive(Debug, Clone)]
pub struct MetadataImportTarget {
    pub media_id: String,
    pub title: String,
    pub year: Option<i32>,
    pub kind: MediaKind,
    pub file_name: String,
    pub fingerprint: String,
}

#[derive(Debug, Clone)]
pub struct IdentificationCacheEntry {
    pub fingerprint: String,
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct MediaScanTarget {
    pub media_id: String,
    pub root_id: String,
    pub path: String,
    pub file_size: i64,
    pub modified_at: i64,
    pub fingerprint: String,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent).context("create application data directory")?;
        }
        let connection = Connection::open(path).context("open SQLite database")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        let database = Self {
            connection: Mutex::new(connection),
        };
        database.migrate()?;
        Ok(database)
    }

    fn migrate(&self) -> Result<()> {
        self.connection.lock().execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY);
            CREATE TABLE IF NOT EXISTS library_roots(
              id TEXT PRIMARY KEY, path TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL,
              enabled INTEGER NOT NULL DEFAULT 1, recursive INTEGER NOT NULL DEFAULT 1,
              watch_enabled INTEGER NOT NULL DEFAULT 1, status TEXT NOT NULL DEFAULT 'disconnected',
              last_scan_at TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS media_items(
              id TEXT PRIMARY KEY, kind TEXT NOT NULL, title TEXT NOT NULL, original_title TEXT,
              sort_title TEXT NOT NULL, year INTEGER, overview TEXT, genres_json TEXT NOT NULL DEFAULT '[]',
              cast_json TEXT NOT NULL DEFAULT '[]',
              runtime_ms INTEGER, series_title TEXT, season_number INTEGER, episode_number INTEGER,
              poster_cache_key TEXT, backdrop_cache_key TEXT, manual_metadata INTEGER NOT NULL DEFAULT 0,
              metadata_status TEXT NOT NULL DEFAULT 'pending', metadata_source_url TEXT,
              metadata_imported_at TEXT, metadata_checked_at TEXT,
              metadata_candidates_json TEXT NOT NULL DEFAULT '[]', metadata_json_path TEXT,
              created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS media_files(
              id TEXT PRIMARY KEY, media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
              library_root_id TEXT NOT NULL REFERENCES library_roots(id), path TEXT NOT NULL UNIQUE,
              file_name TEXT NOT NULL, file_size INTEGER NOT NULL, modified_at INTEGER NOT NULL,
              fingerprint TEXT NOT NULL, container TEXT, duration_ms INTEGER, width INTEGER, height INTEGER,
              video_codec TEXT, audio_codec TEXT, hdr_type TEXT, offline INTEGER NOT NULL DEFAULT 0,
              last_seen_scan TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS series(
              id TEXT PRIMARY KEY, title TEXT NOT NULL UNIQUE, sort_title TEXT NOT NULL,
              poster_cache_key TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS seasons(
              id TEXT PRIMARY KEY, series_id TEXT NOT NULL REFERENCES series(id) ON DELETE CASCADE,
              season_number INTEGER NOT NULL, title TEXT NOT NULL, poster_cache_key TEXT,
              UNIQUE(series_id, season_number)
            );
            CREATE TABLE IF NOT EXISTS episodes(
              id TEXT PRIMARY KEY, season_id TEXT NOT NULL REFERENCES seasons(id) ON DELETE CASCADE,
              media_item_id TEXT NOT NULL UNIQUE REFERENCES media_items(id) ON DELETE CASCADE,
              episode_number INTEGER NOT NULL, absolute_number INTEGER
            );
            CREATE TABLE IF NOT EXISTS media_tracks(
              id TEXT PRIMARY KEY, media_file_id TEXT NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
              track_type TEXT NOT NULL, stream_index INTEGER NOT NULL, language TEXT, title TEXT, codec TEXT,
              channels INTEGER, default_track INTEGER NOT NULL DEFAULT 0, forced_track INTEGER NOT NULL DEFAULT 0,
              external_path TEXT
            );
            CREATE TABLE IF NOT EXISTS accounts(
              id TEXT PRIMARY KEY, name TEXT NOT NULL, normalized_name TEXT NOT NULL UNIQUE,
              password_salt TEXT NOT NULL, password_hash TEXT NOT NULL,
              created_at TEXT NOT NULL, updated_at TEXT NOT NULL, last_login_at TEXT
            );
            CREATE TABLE IF NOT EXISTS account_watch_progress(
              account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
              media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
              position_ms INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER NOT NULL DEFAULT 0,
              completed INTEGER NOT NULL DEFAULT 0, last_watched_at TEXT,
              PRIMARY KEY(account_id, media_item_id)
            );
            CREATE TABLE IF NOT EXISTS account_user_flags(
              account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
              media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
              favorite INTEGER NOT NULL DEFAULT 0, in_watchlist INTEGER NOT NULL DEFAULT 0,
              updated_at TEXT NOT NULL,
              PRIMARY KEY(account_id, media_item_id)
            );
            CREATE TABLE IF NOT EXISTS account_watch_history(
              id TEXT PRIMARY KEY, account_id TEXT NOT NULL REFERENCES accounts(id) ON DELETE CASCADE,
              media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
              played_at TEXT NOT NULL, position_ms INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS watch_progress(
              media_item_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
              position_ms INTEGER NOT NULL DEFAULT 0, duration_ms INTEGER NOT NULL DEFAULT 0,
              completed INTEGER NOT NULL DEFAULT 0, last_watched_at TEXT
            );
            CREATE TABLE IF NOT EXISTS user_flags(
              media_item_id TEXT PRIMARY KEY REFERENCES media_items(id) ON DELETE CASCADE,
              favorite INTEGER NOT NULL DEFAULT 0, in_watchlist INTEGER NOT NULL DEFAULT 0,
              updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS watch_history(
              id TEXT PRIMARY KEY, media_item_id TEXT NOT NULL REFERENCES media_items(id) ON DELETE CASCADE,
              played_at TEXT NOT NULL, position_ms INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE IF NOT EXISTS hero_slots(
              slot INTEGER PRIMARY KEY, media_item_id TEXT REFERENCES media_items(id), pinned INTEGER NOT NULL DEFAULT 0,
              selected_at TEXT, expires_at TEXT
            );
            CREATE TABLE IF NOT EXISTS image_profiles(
              id TEXT PRIMARY KEY, media_item_id TEXT REFERENCES media_items(id) ON DELETE CASCADE,
              profile_json TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS settings(
              key TEXT PRIMARY KEY, value_json TEXT NOT NULL, updated_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS scan_jobs(
              id TEXT PRIMARY KEY, library_root_id TEXT NOT NULL REFERENCES library_roots(id),
              reason TEXT NOT NULL, status TEXT NOT NULL, found INTEGER NOT NULL DEFAULT 0,
              processed INTEGER NOT NULL DEFAULT 0, skipped INTEGER NOT NULL DEFAULT 0,
              errors INTEGER NOT NULL DEFAULT 0, started_at TEXT NOT NULL, finished_at TEXT
            );
            CREATE INDEX IF NOT EXISTS idx_media_title ON media_items(sort_title);
            CREATE INDEX IF NOT EXISTS idx_media_series ON media_items(series_title, season_number, episode_number);
            CREATE INDEX IF NOT EXISTS idx_files_root_path ON media_files(library_root_id, path);
            CREATE INDEX IF NOT EXISTS idx_files_fingerprint ON media_files(library_root_id, fingerprint);
            CREATE INDEX IF NOT EXISTS idx_files_offline ON media_files(offline);
            CREATE INDEX IF NOT EXISTS idx_progress_last ON watch_progress(last_watched_at);
            CREATE INDEX IF NOT EXISTS idx_account_progress_last ON account_watch_progress(account_id,last_watched_at);
            CREATE INDEX IF NOT EXISTS idx_account_history_played ON account_watch_history(account_id,played_at);
            INSERT OR IGNORE INTO schema_migrations(version) VALUES(1);
            "#,
        )?;
        self.ensure_media_column("preview_cache_key", "TEXT", 2)?;
        self.ensure_media_column("cast_json", "TEXT NOT NULL DEFAULT '[]'", 3)?;
        self.ensure_media_column("metadata_status", "TEXT NOT NULL DEFAULT 'pending'", 4)?;
        self.ensure_media_column("metadata_source_url", "TEXT", 5)?;
        self.ensure_media_column("metadata_imported_at", "TEXT", 6)?;
        self.ensure_media_column("metadata_checked_at", "TEXT", 7)?;
        self.ensure_media_column("metadata_candidates_json", "TEXT NOT NULL DEFAULT '[]'", 8)?;
        self.ensure_media_column("metadata_json_path", "TEXT", 9)?;
        self.ensure_media_column(
            "identification_source",
            "TEXT NOT NULL DEFAULT 'legacy'",
            10,
        )?;
        self.ensure_media_column("needs_review", "INTEGER NOT NULL DEFAULT 0", 11)?;
        self.ensure_media_column("review_reason", "TEXT", 12)?;
        self.ensure_media_column("manual_classification", "INTEGER NOT NULL DEFAULT 0", 13)?;
        Ok(())
    }

    fn ensure_media_column(&self, name: &str, definition: &str, version: i64) -> Result<()> {
        let exists = {
            let conn = self.connection.lock();
            let mut statement = conn.prepare("PRAGMA table_info(media_items)")?;
            let columns = statement
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            columns.iter().any(|column| column == name)
        };
        if !exists {
            let conn = self.connection.lock();
            conn.execute(
                &format!("ALTER TABLE media_items ADD COLUMN {name} {definition}"),
                [],
            )?;
            conn.execute(
                "INSERT OR REPLACE INTO schema_migrations(version) VALUES(?1)",
                [version],
            )?;
        }
        Ok(())
    }

    pub fn accounts(&self) -> Result<Vec<AccountDto>> {
        let conn = self.connection.lock();
        let mut statement =
            conn.prepare("SELECT id,name FROM accounts ORDER BY normalized_name")?;
        statement
            .query_map([], account_from_row)?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn active_account(&self) -> Result<Option<AccountDto>> {
        let conn = self.connection.lock();
        active_account_locked(&conn)
    }

    pub fn require_active_account_id(&self) -> Result<String> {
        self.active_account()?
            .map(|account| account.id)
            .ok_or_else(|| anyhow::anyhow!("Primero tenés que entrar con una cuenta local"))
    }

    pub fn create_account(&self, name: &str, password: &str) -> Result<AccountDto> {
        let name = validate_account_name(name)?;
        validate_password(password)?;
        let normalized = normalize_account_name(&name);
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let salt = Uuid::new_v4().to_string();
        let hash = password_hash(&salt, password);
        let conn = self.connection.lock();
        if conn
            .query_row(
                "SELECT 1 FROM accounts WHERE normalized_name=?1",
                [&normalized],
                |_| Ok(()),
            )
            .optional()?
            .is_some()
        {
            anyhow::bail!("Ya existe una cuenta con ese nombre");
        }
        let first_account =
            conn.query_row("SELECT COUNT(*) FROM accounts", [], |r| r.get::<_, i64>(0))? == 0;
        conn.execute(
            "INSERT INTO accounts(id,name,normalized_name,password_salt,password_hash,created_at,updated_at,last_login_at) VALUES(?1,?2,?3,?4,?5,?6,?6,?6)",
            params![id, name, normalized, salt, hash, now],
        )?;
        if first_account {
            migrate_legacy_user_state_locked(&conn, &id)?;
        }
        set_active_account_locked(&conn, &id, &now)?;
        Ok(AccountDto { id, name })
    }

    pub fn login_account(&self, name: &str, password: &str) -> Result<AccountDto> {
        let name = validate_account_name(name)?;
        validate_password(password)?;
        let normalized = normalize_account_name(&name);
        let conn = self.connection.lock();
        let account = conn
            .query_row(
                "SELECT id,name,password_salt,password_hash FROM accounts WHERE normalized_name=?1",
                [&normalized],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, String>(2)?,
                        r.get::<_, String>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((id, name, salt, expected_hash)) = account else {
            anyhow::bail!("No existe una cuenta con ese nombre");
        };
        if password_hash(&salt, password) != expected_hash {
            anyhow::bail!("Contraseña incorrecta");
        }
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE accounts SET last_login_at=?1,updated_at=?1 WHERE id=?2",
            params![now, id],
        )?;
        set_active_account_locked(&conn, &id, &now)?;
        Ok(AccountDto { id, name })
    }

    pub fn logout_account(&self) -> Result<()> {
        self.connection
            .lock()
            .execute("DELETE FROM settings WHERE key='active_account_id'", [])?;
        Ok(())
    }

    pub fn seed_root(&self, path: &str) -> Result<String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let display = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("Biblioteca");
        self.connection.lock().execute(
            "INSERT OR IGNORE INTO library_roots(id,path,display_name,status,created_at,updated_at) VALUES(?1,?2,?3,'disconnected',?4,?4)",
            params![id, path, display, now],
        )?;
        self.connection
            .lock()
            .query_row("SELECT id FROM library_roots WHERE path=?1", [path], |r| {
                r.get(0)
            })
            .map_err(Into::into)
    }

    pub fn replace_root(&self, path: &str) -> Result<String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let display = Path::new(path)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(path);
        let conn = self.connection.lock();
        conn.execute("UPDATE library_roots SET enabled=0,updated_at=?1", [&now])?;
        conn.execute(
            "INSERT INTO library_roots(id,path,display_name,enabled,recursive,watch_enabled,status,created_at,updated_at) VALUES(?1,?2,?3,1,1,1,'disconnected',?4,?4) ON CONFLICT(path) DO UPDATE SET enabled=1,display_name=excluded.display_name,updated_at=excluded.updated_at",
            params![id, path, display, now],
        )?;
        conn.query_row("SELECT id FROM library_roots WHERE path=?1", [path], |r| {
            r.get(0)
        })
        .map_err(Into::into)
    }

    pub fn roots(&self, include_local_path: bool) -> Result<Vec<LibraryRootDto>> {
        let conn = self.connection.lock();
        let mut statement = conn.prepare(
            "SELECT r.id,r.path,r.display_name,r.enabled,r.recursive,r.watch_enabled,r.status,r.last_scan_at,COALESCE(SUM(CASE WHEN f.offline=1 THEN 1 ELSE 0 END),0) FROM library_roots r LEFT JOIN media_files f ON f.library_root_id=r.id GROUP BY r.id ORDER BY r.created_at",
        )?;
        let rows = statement.query_map([], |r| {
            let status: String = r.get(6)?;
            Ok(LibraryRootDto {
                id: r.get(0)?,
                display_name: r.get(2)?,
                enabled: r.get::<_, i64>(3)? != 0,
                recursive: r.get::<_, i64>(4)? != 0,
                watch_enabled: r.get::<_, i64>(5)? != 0,
                status: parse_root_status(&status),
                last_scan_at: r.get(7)?,
                disconnected_count: r.get::<_, i64>(8)? as u64,
                local_path: include_local_path.then(|| r.get(1)).transpose()?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn enabled_roots_with_paths(&self) -> Result<Vec<(String, String)>> {
        let conn = self.connection.lock();
        let mut statement =
            conn.prepare("SELECT id,path FROM library_roots WHERE enabled=1 ORDER BY created_at")?;
        statement
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn set_root_status(&self, root_id: &str, status: &str) -> Result<()> {
        self.connection.lock().execute(
            "UPDATE library_roots SET status=?1,updated_at=?2 WHERE id=?3",
            params![status, Utc::now().to_rfc3339(), root_id],
        )?;
        Ok(())
    }

    pub fn start_scan(&self, root_id: &str, scan_id: &str, reason: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        conn.execute("INSERT INTO scan_jobs(id,library_root_id,reason,status,started_at) VALUES(?1,?2,?3,'running',?4)", params![scan_id, root_id, reason, now])?;
        conn.execute(
            "UPDATE library_roots SET status='scanning',updated_at=?1 WHERE id=?2",
            params![now, root_id],
        )?;
        Ok(())
    }

    pub fn upsert_file(
        &self,
        root_id: &str,
        scan_id: &str,
        file: &DiscoveredFile,
    ) -> Result<UpsertOutcome> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        if let Some((file_id, media_id, size, modified)) = conn
            .query_row(
                "SELECT id,media_item_id,file_size,modified_at FROM media_files WHERE path=?1",
                [&file.path],
                |r| {
                    Ok((
                        r.get::<_, String>(0)?,
                        r.get::<_, String>(1)?,
                        r.get::<_, i64>(2)?,
                        r.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
        {
            if size == file.file_size && modified == file.modified_at {
                conn.execute(
                    "UPDATE media_files SET offline=0,last_seen_scan=?1,updated_at=?2 WHERE id=?3",
                    params![scan_id, now, file_id],
                )?;
                self.sync_identification_locked(&conn, &media_id, &file.parsed, &now)?;
                return Ok(UpsertOutcome {
                    media_id,
                    inserted: false,
                    skipped: true,
                });
            }
            update_media_and_file(&conn, &media_id, &file_id, root_id, scan_id, file, &now)?;
            self.sync_identification_locked(&conn, &media_id, &file.parsed, &now)?;
            self.replace_external_tracks_locked(&conn, &file_id, &file.external_subtitles)?;
            return Ok(UpsertOutcome {
                media_id,
                inserted: false,
                skipped: false,
            });
        }

        if let Some((file_id, media_id)) = conn.query_row(
            "SELECT id,media_item_id FROM media_files WHERE library_root_id=?1 AND fingerprint=?2 LIMIT 1",
            params![root_id, file.fingerprint], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        ).optional()? {
            update_media_and_file(&conn, &media_id, &file_id, root_id, scan_id, file, &now)?;
            self.sync_identification_locked(&conn, &media_id, &file.parsed, &now)?;
            self.replace_external_tracks_locked(&conn, &file_id, &file.external_subtitles)?;
            return Ok(UpsertOutcome { media_id, inserted: false, skipped: false });
        }

        let media_id = Uuid::new_v4().to_string();
        let file_id = Uuid::new_v4().to_string();
        let genres_json = serde_json::to_string(&infer_genres(file))?;
        conn.execute(
            "INSERT INTO media_items(id,kind,title,sort_title,year,genres_json,runtime_ms,series_title,season_number,episode_number,identification_source,needs_review,review_reason,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?14)",
            params![media_id, kind_text(&file.parsed.kind), file.parsed.title, file.parsed.title.to_lowercase(), file.parsed.year, genres_json, file.technical.duration_ms, file.parsed.series_title, file.parsed.season_number, file.parsed.episode_number, file.parsed.identification_source, file.parsed.needs_review, file.parsed.review_reason, now],
        )?;
        conn.execute(
            "INSERT INTO media_files(id,media_item_id,library_root_id,path,file_name,file_size,modified_at,fingerprint,container,duration_ms,width,height,video_codec,audio_codec,hdr_type,offline,last_seen_scan,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,0,?16,?17,?17)",
            params![file_id, media_id, root_id, file.path, file.file_name, file.file_size, file.modified_at, file.fingerprint, file.technical.container, file.technical.duration_ms, file.technical.width, file.technical.height, file.technical.video_codec, file.technical.audio_codec, file.technical.hdr_type, scan_id, now],
        )?;
        self.ensure_episode_hierarchy_locked(&conn, &media_id, &file.parsed, &now)?;
        self.replace_external_tracks_locked(&conn, &file_id, &file.external_subtitles)?;
        Ok(UpsertOutcome {
            media_id,
            inserted: true,
            skipped: false,
        })
    }

    pub fn reconcile_unchanged_file(
        &self,
        scan_id: &str,
        path: &str,
        file_size: i64,
        modified_at: i64,
        parsed: &ParsedMediaName,
    ) -> Result<Option<UpsertOutcome>> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        let existing = conn
            .query_row(
                "SELECT id,media_item_id FROM media_files WHERE path=?1 AND file_size=?2 AND modified_at=?3",
                params![path, file_size, modified_at],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((file_id, media_id)) = existing else {
            return Ok(None);
        };
        conn.execute(
            "UPDATE media_files SET offline=0,last_seen_scan=?1,updated_at=?2 WHERE id=?3",
            params![scan_id, now, file_id],
        )?;
        self.sync_identification_locked(&conn, &media_id, parsed, &now)?;
        Ok(Some(UpsertOutcome {
            media_id,
            inserted: false,
            skipped: true,
        }))
    }

    pub fn set_artwork(
        &self,
        media_id: &str,
        poster: &str,
        backdrop: &str,
        preview: &str,
    ) -> Result<()> {
        self.connection.lock().execute(
            "UPDATE media_items SET
             poster_cache_key=CASE WHEN manual_metadata=1 AND COALESCE(poster_cache_key,'')<>'' THEN poster_cache_key ELSE ?1 END,
             backdrop_cache_key=CASE WHEN manual_metadata=1 AND COALESCE(backdrop_cache_key,'')<>'' THEN backdrop_cache_key ELSE ?2 END,
             preview_cache_key=?3,updated_at=?4 WHERE id=?5",
            params![poster,backdrop,preview,Utc::now().to_rfc3339(),media_id],
        )?;
        Ok(())
    }

    fn sync_identification_locked(
        &self,
        conn: &Connection,
        media_id: &str,
        parsed: &ParsedMediaName,
        now: &str,
    ) -> Result<()> {
        let manual_classification = conn.query_row(
            "SELECT manual_classification FROM media_items WHERE id=?1",
            [media_id],
            |row| row.get::<_, i64>(0),
        )? != 0;
        if manual_classification {
            return Ok(());
        }
        conn.execute(
            "UPDATE media_items SET
             kind=?1,
             title=CASE WHEN manual_metadata=0 THEN ?2 ELSE title END,
             sort_title=CASE WHEN manual_metadata=0 THEN ?3 ELSE sort_title END,
             year=CASE WHEN manual_metadata=0 THEN ?4 ELSE year END,
             series_title=?5,season_number=?6,episode_number=?7,
             identification_source=?8,needs_review=?9,review_reason=?10,updated_at=?11
             WHERE id=?12",
            params![
                kind_text(&parsed.kind),
                parsed.title,
                parsed.title.to_lowercase(),
                parsed.year,
                parsed.series_title,
                parsed.season_number,
                parsed.episode_number,
                parsed.identification_source,
                parsed.needs_review,
                parsed.review_reason,
                now,
                media_id
            ],
        )?;
        conn.execute("DELETE FROM episodes WHERE media_item_id=?1", [media_id])?;
        self.ensure_episode_hierarchy_locked(conn, media_id, parsed, now)?;
        cleanup_empty_series_locked(conn)?;
        Ok(())
    }

    fn ensure_episode_hierarchy_locked(
        &self,
        conn: &Connection,
        media_id: &str,
        parsed: &ParsedMediaName,
        now: &str,
    ) -> Result<()> {
        if parsed.kind != MediaKind::Episode {
            return Ok(());
        }
        let title = parsed.series_title.as_deref().unwrap_or("Serie");
        let series_id = Uuid::new_v4().to_string();
        conn.execute("INSERT OR IGNORE INTO series(id,title,sort_title,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)", params![series_id,title,title.to_lowercase(),now])?;
        let series_id: String =
            conn.query_row("SELECT id FROM series WHERE title=?1", [title], |r| {
                r.get(0)
            })?;
        let season_number = parsed.season_number.unwrap_or(0);
        let season_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT OR IGNORE INTO seasons(id,series_id,season_number,title) VALUES(?1,?2,?3,?4)",
            params![
                season_id,
                series_id,
                season_number,
                format!("Temporada {season_number}")
            ],
        )?;
        let season_id: String = conn.query_row(
            "SELECT id FROM seasons WHERE series_id=?1 AND season_number=?2",
            params![series_id, season_number],
            |r| r.get(0),
        )?;
        conn.execute("INSERT OR REPLACE INTO episodes(id,season_id,media_item_id,episode_number) VALUES(COALESCE((SELECT id FROM episodes WHERE media_item_id=?2),?1),?3,?2,?4)", params![Uuid::new_v4().to_string(),media_id,season_id,parsed.episode_number.unwrap_or(0)])?;
        Ok(())
    }

    fn replace_external_tracks_locked(
        &self,
        conn: &Connection,
        file_id: &str,
        subtitles: &[String],
    ) -> Result<()> {
        conn.execute(
            "DELETE FROM media_tracks WHERE media_file_id=?1 AND external_path IS NOT NULL",
            [file_id],
        )?;
        for (index, subtitle) in subtitles.iter().enumerate() {
            conn.execute(
                "INSERT INTO media_tracks(id,media_file_id,track_type,stream_index,title,codec,external_path) VALUES(?1,?2,'subtitle',?3,?4,?5,?6)",
                params![Uuid::new_v4().to_string(), file_id, index as i32, Path::new(subtitle).file_name().and_then(|s| s.to_str()), Path::new(subtitle).extension().and_then(|s| s.to_str()), subtitle],
            )?;
        }
        Ok(())
    }

    pub fn finish_scan(
        &self,
        root_id: &str,
        scan_id: &str,
        status: &str,
        found: u64,
        processed: u64,
        skipped: u64,
        errors: u64,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        if status == "completed" {
            conn.execute("UPDATE media_files SET offline=1,updated_at=?1 WHERE library_root_id=?2 AND COALESCE(last_seen_scan,'')<>?3", params![now,root_id,scan_id])?;
        }
        conn.execute("UPDATE scan_jobs SET status=?1,found=?2,processed=?3,skipped=?4,errors=?5,finished_at=?6 WHERE id=?7", params![status,found,processed,skipped,errors,now,scan_id])?;
        conn.execute(
            "UPDATE library_roots SET status=?1,last_scan_at=?2,updated_at=?2 WHERE id=?3",
            params![
                if status == "completed" {
                    "online"
                } else {
                    "error"
                },
                now,
                root_id
            ],
        )?;
        Ok(())
    }

    pub fn catalog(
        &self,
        account_id: Option<&str>,
        query: &CatalogQuery,
    ) -> Result<Vec<MediaSummary>> {
        let account_id = account_id.unwrap_or("");
        let conn = self.connection.lock();
        let mut statement = conn.prepare(MEDIA_SELECT)?;
        let mut items = statement
            .query_map([account_id], row_to_summary)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        if let Some(search) = query
            .search
            .as_ref()
            .map(|s| s.trim().to_lowercase())
            .filter(|s| !s.is_empty())
        {
            items.retain(|m| {
                m.title.to_lowercase().contains(&search)
                    || m.series_title
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&search)
                    || m.year
                        .map(|y| y.to_string().contains(&search))
                        .unwrap_or(false)
            });
        }
        if let Some(kind) = query.kind.as_deref() {
            items.retain(|m| {
                matches!(
                    (kind, &m.kind),
                    ("movie", MediaKind::Movie)
                        | ("episode", MediaKind::Episode)
                        | ("series", MediaKind::Episode)
                )
            });
        }
        match query.filter.as_deref() {
            Some("favorites") => items.retain(|m| m.favorite),
            Some("watchlist") => items.retain(|m| m.in_watchlist),
            Some("continue") => items.retain(|m| m.progress_percent > 0.0 && !m.completed),
            Some("unwatched") => items.retain(|m| m.progress_percent == 0.0 && !m.completed),
            Some("history") => items.retain(|m| m.progress_percent > 0.0 || m.completed),
            _ => {}
        }
        match query.sort.as_deref() {
            Some("title") => {
                items.sort_by(|a, b| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
            }
            Some("year_desc") => items.sort_by(|a, b| b.year.cmp(&a.year)),
            Some("progress") => {
                items.sort_by(|a, b| b.progress_percent.total_cmp(&a.progress_percent))
            }
            _ => items.sort_by(|a, b| b.added_at.cmp(&a.added_at)),
        }
        let offset = query.offset.unwrap_or(0) as usize;
        let limit = query.limit.unwrap_or(500).min(2000) as usize;
        Ok(items.into_iter().skip(offset).take(limit).collect())
    }

    pub fn identification_reviews(&self) -> Result<Vec<IdentificationReview>> {
        let conn = self.connection.lock();
        let mut statement = conn.prepare(
            "SELECT m.id,f.file_name,m.kind,m.title,m.series_title,m.season_number,m.episode_number,
                    COALESCE(m.review_reason,'Identificacion pendiente')
             FROM media_items m
             JOIN media_files f ON f.media_item_id=m.id
             WHERE m.needs_review=1 AND f.offline=0
             ORDER BY lower(f.file_name)",
        )?;
        statement
            .query_map([], |row| {
                let kind: String = row.get(2)?;
                Ok(IdentificationReview {
                    media_id: row.get(0)?,
                    file_name: row.get(1)?,
                    kind: if kind == "episode" {
                        MediaKind::Episode
                    } else {
                        MediaKind::Movie
                    },
                    title: row.get(3)?,
                    series_title: row.get(4)?,
                    season_number: row.get(5)?,
                    episode_number: row.get(6)?,
                    reason: row.get(7)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(Into::into)
    }

    pub fn resolve_identification(
        &self,
        media_id: &str,
        update: &ClassificationUpdate,
    ) -> Result<()> {
        let title = update.title.trim();
        if title.is_empty() {
            anyhow::bail!("El titulo no puede estar vacio");
        }
        if update.kind == MediaKind::Episode
            && (update
                .series_title
                .as_deref()
                .map(str::trim)
                .unwrap_or_default()
                .is_empty()
                || update.season_number.is_none()
                || update.episode_number.is_none())
        {
            anyhow::bail!("Una serie necesita nombre, temporada y episodio");
        }
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        let series_title = if update.kind == MediaKind::Episode {
            update.series_title.as_deref().map(str::trim)
        } else {
            None
        };
        conn.execute(
            "UPDATE media_items SET kind=?1,title=?2,sort_title=?3,series_title=?4,
             season_number=?5,episode_number=?6,identification_source='manual',needs_review=0,
             review_reason=NULL,manual_classification=1,updated_at=?7 WHERE id=?8",
            params![
                kind_text(&update.kind),
                title,
                title.to_lowercase(),
                series_title,
                if update.kind == MediaKind::Episode {
                    update.season_number
                } else {
                    None
                },
                if update.kind == MediaKind::Episode {
                    update.episode_number
                } else {
                    None
                },
                now,
                media_id
            ],
        )?;
        conn.execute("DELETE FROM episodes WHERE media_item_id=?1", [media_id])?;
        if update.kind == MediaKind::Episode {
            let parsed = ParsedMediaName {
                kind: MediaKind::Episode,
                title: title.to_string(),
                year: None,
                series_title: series_title.map(str::to_string),
                season_number: update.season_number,
                episode_number: update.episode_number,
                identification_source: "manual".into(),
                needs_review: false,
                review_reason: None,
            };
            self.ensure_episode_hierarchy_locked(&conn, media_id, &parsed, &now)?;
        }
        cleanup_empty_series_locked(&conn)?;
        Ok(())
    }

    pub fn identification_cache_entry(
        &self,
        media_id: &str,
    ) -> Result<Option<IdentificationCacheEntry>> {
        let conn = self.connection.lock();
        conn.query_row(
            "SELECT f.fingerprint,f.file_name,m.kind,m.title,m.year,m.series_title,
                    m.season_number,m.episode_number,m.identification_source,m.needs_review,
                    m.review_reason,m.overview,m.genres_json,m.cast_json,m.updated_at
             FROM media_items m JOIN media_files f ON f.media_item_id=m.id WHERE m.id=?1 LIMIT 1",
            [media_id],
            |row| {
                let fingerprint: String = row.get(0)?;
                Ok(IdentificationCacheEntry {
                    fingerprint,
                    payload: serde_json::json!({
                        "mediaId": media_id,
                        "fileName": row.get::<_, String>(1)?,
                        "kind": row.get::<_, String>(2)?,
                        "title": row.get::<_, String>(3)?,
                        "year": row.get::<_, Option<i32>>(4)?,
                        "seriesTitle": row.get::<_, Option<String>>(5)?,
                        "seasonNumber": row.get::<_, Option<i32>>(6)?,
                        "episodeNumber": row.get::<_, Option<i32>>(7)?,
                        "identificationSource": row.get::<_, String>(8)?,
                        "needsReview": row.get::<_, i64>(9)? != 0,
                        "reviewReason": row.get::<_, Option<String>>(10)?,
                        "overview": row.get::<_, Option<String>>(11)?,
                        "genres": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(12)?).unwrap_or_else(|_| serde_json::json!([])),
                        "cast": serde_json::from_str::<serde_json::Value>(&row.get::<_, String>(13)?).unwrap_or_else(|_| serde_json::json!([])),
                        "updatedAt": row.get::<_, String>(14)?,
                    }),
                })
            },
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn home(&self, account_id: Option<&str>) -> Result<HomeDto> {
        let all = self.catalog(account_id, &CatalogQuery::default())?;
        let mut movies: Vec<_> = all
            .iter()
            .filter(|m| m.kind == MediaKind::Movie)
            .cloned()
            .collect();
        let continue_watching = all
            .iter()
            .filter(|m| m.progress_percent > 0.0 && !m.completed)
            .take(20)
            .cloned()
            .collect();
        let favorites = all
            .iter()
            .filter(|m| m.favorite)
            .take(20)
            .cloned()
            .collect();
        let recently_added = movies.iter().take(30).cloned().collect();
        order_movies_for_rotation(
            &mut movies,
            daily_movie_rotation_bucket(Local::now().date_naive()),
        );
        let heroes = movies.iter().take(3).cloned().collect();
        let mut series_map: BTreeMap<
            String,
            (
                BTreeMap<i32, Vec<MediaSummary>>,
                String,
                Option<String>,
                String,
            ),
        > = BTreeMap::new();
        for item in all.iter().filter(|m| m.kind == MediaKind::Episode) {
            let title = item
                .series_title
                .clone()
                .unwrap_or_else(|| item.title.clone());
            let entry = series_map.entry(title).or_insert((
                BTreeMap::new(),
                item.added_at.clone(),
                item.artwork_url.clone(),
                item.id.clone(),
            ));
            entry
                .0
                .entry(item.season_number.unwrap_or(0))
                .or_default()
                .push(item.clone());
            if item.added_at > entry.1 {
                entry.1 = item.added_at.clone();
                entry.3 = item.id.clone();
            }
            if entry.2.is_none() {
                entry.2 = item.artwork_url.clone();
            }
        }
        let series = series_map
            .into_iter()
            .map(
                |(title, (mut seasons, latest_added_at, artwork_url, episode_id))| {
                    let episode_count = seasons.values().map(Vec::len).sum::<usize>() as u32;
                    let season_items = seasons
                        .iter_mut()
                        .map(|(season_number, episodes)| {
                            episodes.sort_by_key(|episode| episode.episode_number.unwrap_or(0));
                            SeriesSeasonSummary {
                                season_number: *season_number,
                                title: format!("Temporada {season_number}"),
                                episodes: episodes.clone(),
                            }
                        })
                        .collect::<Vec<_>>();
                    SeriesSummary {
                        episode_id,
                        title,
                        seasons: season_items.len() as u32,
                        episodes: episode_count,
                        artwork_url,
                        latest_added_at,
                        season_items,
                    }
                },
            )
            .collect();
        Ok(HomeDto {
            heroes,
            continue_watching,
            recently_added,
            movies,
            series,
            favorites,
        })
    }

    pub fn media_detail(&self, account_id: Option<&str>, id: &str) -> Result<Option<MediaDetail>> {
        let account_id = account_id.unwrap_or("");
        let conn = self.connection.lock();
        let summary = conn
            .query_row(MEDIA_SELECT_BY_ID, params![account_id, id], row_to_summary)
            .optional()?;
        let Some(summary) = summary else {
            return Ok(None);
        };
        let (
            overview,
            genres_json,
            cast_json,
            runtime_ms,
            file_name,
            manual_metadata,
            metadata_status,
            metadata_source_url,
            metadata_imported_at,
            metadata_candidates_json,
        ): (
            Option<String>,
            String,
            String,
            Option<i64>,
            String,
            bool,
            String,
            Option<String>,
            Option<String>,
            String,
        ) = conn.query_row(
            "SELECT m.overview,m.genres_json,m.cast_json,COALESCE(m.runtime_ms,f.duration_ms),f.file_name,m.manual_metadata,m.metadata_status,m.metadata_source_url,m.metadata_imported_at,m.metadata_candidates_json FROM media_items m JOIN media_files f ON f.media_item_id=m.id WHERE m.id=?1",
            [id],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get::<_, i64>(5)? != 0,
                    r.get(6)?,
                    r.get(7)?,
                    r.get(8)?,
                    r.get(9)?,
                ))
            },
        )?;
        let mut tracks_stmt = conn.prepare("SELECT t.id,t.track_type,t.stream_index,t.language,t.title,t.codec,t.channels,t.default_track,t.forced_track,t.external_path IS NOT NULL FROM media_tracks t JOIN media_files f ON f.id=t.media_file_id WHERE f.media_item_id=?1 ORDER BY t.track_type,t.stream_index")?;
        let tracks = tracks_stmt
            .query_map([id], |r| {
                Ok(MediaTrack {
                    id: r.get(0)?,
                    track_type: r.get(1)?,
                    stream_index: r.get(2)?,
                    language: r.get(3)?,
                    title: r.get(4)?,
                    codec: r.get(5)?,
                    channels: r.get(6)?,
                    default_track: r.get::<_, i64>(7)? != 0,
                    forced_track: r.get::<_, i64>(8)? != 0,
                    external: r.get::<_, i64>(9)? != 0,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let genres = normalize_tags(serde_json::from_str(&genres_json).unwrap_or_default(), 12);
        Ok(Some(MediaDetail {
            recommendations: recommendations_locked(&conn, account_id, id, &genres, &summary)?,
            summary,
            overview,
            genres,
            cast: normalize_tags(serde_json::from_str(&cast_json).unwrap_or_default(), 32),
            runtime_ms,
            tracks,
            file_name,
            manual_metadata,
            metadata_status,
            metadata_source_url,
            metadata_imported_at,
            metadata_candidates: serde_json::from_str(&metadata_candidates_json)
                .unwrap_or_default(),
        }))
    }

    pub fn next_movie(&self, account_id: Option<&str>, id: &str) -> Result<Option<MediaSummary>> {
        let account_id = account_id.unwrap_or("");
        let conn = self.connection.lock();
        let source = conn
            .query_row(MEDIA_SELECT_BY_ID, params![account_id, id], row_to_summary)
            .optional()?;
        let Some(source) = source else {
            return Ok(None);
        };
        if source.kind != MediaKind::Movie {
            return Ok(None);
        }
        let source_key = sequel_key(&source.title);
        let mut statement = conn.prepare(MEDIA_SELECT)?;
        let mut candidates = statement
            .query_map([account_id], row_to_summary)?
            .collect::<rusqlite::Result<Vec<_>>>()?
            .into_iter()
            .filter(|item| item.id != source.id && item.kind == MediaKind::Movie)
            .filter(|item| {
                let key = sequel_key(&item.title);
                key.base == source_key.base && key.sequence == source_key.sequence + 1
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|a, b| {
            a.year
                .cmp(&b.year)
                .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
        });
        Ok(candidates.into_iter().next())
    }

    pub fn next_up(&self, account_id: Option<&str>, id: &str) -> Result<Option<MediaSummary>> {
        let account_id = account_id.unwrap_or("");
        let conn = self.connection.lock();
        let source = conn
            .query_row(MEDIA_SELECT_BY_ID, params![account_id, id], row_to_summary)
            .optional()?;
        let Some(source) = source else {
            return Ok(None);
        };

        match source.kind {
            MediaKind::Episode => {
                let Some(series_title) = source.series_title.as_deref() else {
                    return Ok(None);
                };
                let (Some(source_season), Some(source_episode)) =
                    (source.season_number, source.episode_number)
                else {
                    return Ok(None);
                };
                let mut statement = conn.prepare(MEDIA_SELECT)?;
                let mut episodes = statement
                    .query_map([account_id], row_to_summary)?
                    .collect::<rusqlite::Result<Vec<_>>>()?
                    .into_iter()
                    .filter(|item| item.kind == MediaKind::Episode && item.id != source.id)
                    .filter(|item| {
                        item.series_title
                            .as_deref()
                            .is_some_and(|title| title.eq_ignore_ascii_case(series_title))
                    })
                    .filter(|item| {
                        item.season_number
                            .zip(item.episode_number)
                            .is_some_and(|position| position > (source_season, source_episode))
                    })
                    .collect::<Vec<_>>();
                episodes.sort_by(|a, b| {
                    a.season_number
                        .cmp(&b.season_number)
                        .then_with(|| a.episode_number.cmp(&b.episode_number))
                        .then_with(|| a.title.to_lowercase().cmp(&b.title.to_lowercase()))
                });
                Ok(episodes.into_iter().next())
            }
            MediaKind::Movie => {
                let genres_json: String = conn.query_row(
                    "SELECT genres_json FROM media_items WHERE id=?1",
                    [id],
                    |row| row.get(0),
                )?;
                let genres = normalize_tags(serde_json::from_str(&genres_json).unwrap_or_default(), 12);
                Ok(ranked_recommendations_locked(&conn, account_id, id, &genres, &source)?
                    .into_iter()
                    .find(|item| {
                        item.kind == MediaKind::Movie
                            && !item.completed
                            && item.progress_percent <= f64::EPSILON
                    }))
            }
        }
    }

    pub fn update_media_metadata(
        &self,
        media_id: &str,
        metadata: &MediaMetadataUpdate,
    ) -> Result<()> {
        let title = metadata.title.trim();
        if title.is_empty() {
            anyhow::bail!("El título no puede estar vacío");
        }
        let genres = normalize_tags(metadata.genres.clone(), 12);
        let cast = normalize_tags(metadata.cast.clone(), 32);
        let overview = metadata
            .overview
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned);
        let poster = metadata
            .poster_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let backdrop = metadata
            .backdrop_path
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty());
        self.connection.lock().execute(
            "UPDATE media_items SET title=?1,sort_title=?2,year=?3,overview=?4,genres_json=?5,cast_json=?6,poster_cache_key=COALESCE(?7,poster_cache_key),backdrop_cache_key=COALESCE(?8,backdrop_cache_key),manual_metadata=1,updated_at=?9 WHERE id=?10",
            params![
                title,
                title.to_lowercase(),
                metadata.year,
                overview,
                serde_json::to_string(&genres)?,
                serde_json::to_string(&cast)?,
                poster,
                backdrop,
                Utc::now().to_rfc3339(),
                media_id
            ],
        )?;
        Ok(())
    }

    pub fn metadata_target(&self, media_id: &str) -> Result<Option<MetadataImportTarget>> {
        self.connection
            .lock()
            .query_row(
                "SELECT m.id,COALESCE(m.series_title,m.title),m.year,m.kind,f.file_name,f.fingerprint
                 FROM media_items m JOIN media_files f ON f.media_item_id=m.id
                 WHERE m.id=?1 AND f.offline=0 LIMIT 1",
                [media_id],
                |row| {
                    let kind: String = row.get(3)?;
                    Ok(MetadataImportTarget {
                        media_id: row.get(0)?,
                        title: row.get(1)?,
                        year: row.get(2)?,
                        kind: if kind == "episode" {
                            MediaKind::Episode
                        } else {
                            MediaKind::Movie
                        },
                        file_name: row.get(4)?,
                        fingerprint: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn should_auto_import_metadata(&self, media_id: &str) -> Result<bool> {
        let row = self
            .connection
            .lock()
            .query_row(
                "SELECT manual_metadata,metadata_status,metadata_source_url,metadata_checked_at FROM media_items WHERE id=?1",
                [media_id],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? != 0,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((manual, status, source_url, checked_at)) = row else {
            return Ok(false);
        };
        Ok(!manual && status == "pending" && source_url.is_none() && checked_at.is_none())
    }

    pub fn apply_imported_metadata(
        &self,
        media_id: &str,
        metadata: &ImportedMediaMetadata,
        metadata_json_path: Option<&str>,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let cast = normalize_tags(metadata.cast.clone(), 32);
        self.connection.lock().execute(
            "UPDATE media_items SET overview=COALESCE(?1,overview),cast_json=?2,metadata_status='imported',metadata_source_url=?3,metadata_imported_at=?4,metadata_checked_at=?4,metadata_candidates_json='[]',metadata_json_path=?5,updated_at=?4 WHERE id=?6",
            params![
                metadata.overview,
                serde_json::to_string(&cast)?,
                metadata.source_url,
                now,
                metadata_json_path,
                media_id
            ],
        )?;
        Ok(())
    }

    pub fn store_metadata_candidates(
        &self,
        media_id: &str,
        candidates: &[MediaMetadataCandidate],
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let status = if candidates.is_empty() {
            "pending"
        } else {
            "ambiguous"
        };
        self.connection.lock().execute(
            "UPDATE media_items SET metadata_status=?1,metadata_checked_at=?2,metadata_candidates_json=?3,updated_at=?2 WHERE id=?4",
            params![status, now, serde_json::to_string(candidates)?, media_id],
        )?;
        Ok(())
    }

    pub fn set_flag(
        &self,
        account_id: &str,
        media_id: &str,
        flag: &str,
        value: bool,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        conn.execute(
            "INSERT OR IGNORE INTO account_user_flags(account_id,media_item_id,updated_at) VALUES(?1,?2,?3)",
            params![account_id, media_id, now],
        )?;
        match flag {
            "favorite" => conn.execute(
                "UPDATE account_user_flags SET favorite=?1,updated_at=?2 WHERE account_id=?3 AND media_item_id=?4",
                params![value as i64, now, account_id, media_id],
            )?,
            "watchlist" => conn.execute(
                "UPDATE account_user_flags SET in_watchlist=?1,updated_at=?2 WHERE account_id=?3 AND media_item_id=?4",
                params![value as i64, now, account_id, media_id],
            )?,
            _ => anyhow::bail!("unsupported flag"),
        };
        Ok(())
    }

    pub fn save_progress(
        &self,
        account_id: &str,
        media_id: &str,
        position_ms: i64,
        duration_ms: i64,
    ) -> Result<()> {
        let completed = duration_ms > 0
            && ((position_ms as f64 / duration_ms as f64) >= 0.9
                || duration_ms - position_ms <= 120_000);
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        conn.execute("INSERT INTO account_watch_progress(account_id,media_item_id,position_ms,duration_ms,completed,last_watched_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(account_id,media_item_id) DO UPDATE SET position_ms=excluded.position_ms,duration_ms=excluded.duration_ms,completed=excluded.completed,last_watched_at=excluded.last_watched_at", params![account_id,media_id,position_ms,duration_ms,completed as i64,now])?;
        conn.execute(
            "INSERT INTO account_watch_history(id,account_id,media_item_id,played_at,position_ms) VALUES(?1,?2,?3,?4,?5)",
            params![Uuid::new_v4().to_string(), account_id, media_id, now, position_ms],
        )?;
        Ok(())
    }

    pub fn media_path(&self, media_id: &str) -> Result<Option<String>> {
        self.connection
            .lock()
            .query_row(
                "SELECT path FROM media_files WHERE media_item_id=?1 AND offline=0",
                [media_id],
                |r| r.get(0),
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn media_scan_target(&self, media_id: &str) -> Result<Option<MediaScanTarget>> {
        self.connection
            .lock()
            .query_row(
                "SELECT f.media_item_id,f.library_root_id,f.path,f.file_size,f.modified_at,f.fingerprint
                 FROM media_files f WHERE f.media_item_id=?1 LIMIT 1",
                [media_id],
                |row| {
                    Ok(MediaScanTarget {
                        media_id: row.get(0)?,
                        root_id: row.get(1)?,
                        path: row.get(2)?,
                        file_size: row.get(3)?,
                        modified_at: row.get(4)?,
                        fingerprint: row.get(5)?,
                    })
                },
            )
            .optional()
            .map_err(Into::into)
    }

    pub fn needs_identification_review(&self, media_id: &str) -> Result<bool> {
        Ok(self
            .connection
            .lock()
            .query_row(
                "SELECT needs_review FROM media_items WHERE id=?1",
                [media_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .unwrap_or_default()
            != 0)
    }

    pub fn backup_to(&self, target: &Path) -> Result<()> {
        let escaped = target.to_string_lossy().replace('\'', "''");
        self.connection
            .lock()
            .execute_batch(&format!("VACUUM INTO '{escaped}'"))?;
        Ok(())
    }
}

const MEDIA_SELECT: &str = r#"
SELECT m.id,m.kind,m.title,m.year,m.series_title,m.season_number,m.episode_number,m.overview,
       COALESCE(p.position_ms,0),COALESCE(p.duration_ms,COALESCE(m.runtime_ms,f.duration_ms,0)),COALESCE(p.completed,0),
       COALESCE(u.favorite,0),COALESCE(u.in_watchlist,0),f.offline,m.created_at,m.poster_cache_key,m.backdrop_cache_key,m.preview_cache_key,
       COALESCE(m.runtime_ms,f.duration_ms),f.width,f.height,f.container,f.video_codec,f.audio_codec,f.hdr_type
FROM media_items m JOIN media_files f ON f.media_item_id=m.id
LEFT JOIN account_watch_progress p ON p.media_item_id=m.id AND p.account_id=?1
LEFT JOIN account_user_flags u ON u.media_item_id=m.id AND u.account_id=?1
WHERE f.offline=0 ORDER BY m.created_at DESC
"#;

const MEDIA_SELECT_BY_ID: &str = r#"
SELECT m.id,m.kind,m.title,m.year,m.series_title,m.season_number,m.episode_number,m.overview,
       COALESCE(p.position_ms,0),COALESCE(p.duration_ms,COALESCE(m.runtime_ms,f.duration_ms,0)),COALESCE(p.completed,0),
       COALESCE(u.favorite,0),COALESCE(u.in_watchlist,0),f.offline,m.created_at,m.poster_cache_key,m.backdrop_cache_key,m.preview_cache_key,
       COALESCE(m.runtime_ms,f.duration_ms),f.width,f.height,f.container,f.video_codec,f.audio_codec,f.hdr_type
FROM media_items m JOIN media_files f ON f.media_item_id=m.id
LEFT JOIN account_watch_progress p ON p.media_item_id=m.id AND p.account_id=?1
LEFT JOIN account_user_flags u ON u.media_item_id=m.id AND u.account_id=?1
WHERE f.offline=0 AND m.id=?2
"#;

fn recommendations_locked(
    conn: &Connection,
    account_id: &str,
    media_id: &str,
    source_genres: &[String],
    source: &MediaSummary,
) -> Result<Vec<MediaSummary>> {
    Ok(ranked_recommendations_locked(
        conn,
        account_id,
        media_id,
        source_genres,
        source,
    )?
    .into_iter()
    .take(12)
    .collect())
}

fn ranked_recommendations_locked(
    conn: &Connection,
    account_id: &str,
    media_id: &str,
    source_genres: &[String],
    source: &MediaSummary,
) -> Result<Vec<MediaSummary>> {
    let mut genres_stmt = conn.prepare("SELECT id,genres_json FROM media_items")?;
    let genre_rows = genres_stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let genre_map = genre_rows
        .into_iter()
        .map(|(id, json)| {
            let values = normalize_tags(serde_json::from_str(&json).unwrap_or_default(), 12);
            (id, values)
        })
        .collect::<BTreeMap<_, _>>();

    let source_set = source_genres
        .iter()
        .map(|genre| genre.to_lowercase())
        .collect::<BTreeSet<_>>();
    let mut statement = conn.prepare(MEDIA_SELECT)?;
    let items = statement
        .query_map([account_id], row_to_summary)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut scored = items
        .into_iter()
        .filter(|item| item.id != media_id)
        .filter_map(|item| {
            let mut score = 0i32;
            if item.kind == source.kind {
                score += 4;
            }
            if item.series_title.is_some() && item.series_title == source.series_title {
                score += 6;
            }
            if let (Some(a), Some(b)) = (source.year, item.year) {
                let distance = (a - b).abs();
                if distance <= 5 {
                    score += 3;
                } else if distance <= 15 {
                    score += 1;
                }
            }
            for genre in genre_map.get(&item.id).into_iter().flatten() {
                if source_set.contains(&genre.to_lowercase()) {
                    score += 10;
                }
            }
            if source_set.is_empty() && item.kind == source.kind {
                score += 1;
            }
            (score > 0).then_some((score, item))
        })
        .collect::<Vec<_>>();
    scored.sort_by(|(score_a, item_a), (score_b, item_b)| {
        score_b.cmp(score_a).then_with(|| {
            item_a
                .title
                .to_lowercase()
                .cmp(&item_b.title.to_lowercase())
        })
    });
    Ok(scored.into_iter().map(|(_, item)| item).collect())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SequelKey {
    base: String,
    sequence: u32,
}

fn sequel_key(title: &str) -> SequelKey {
    let tokens = title
        .split(|value: char| !value.is_alphanumeric())
        .map(normalize_sequence_token)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let Some((index, sequence)) = tokens
        .iter()
        .enumerate()
        .find_map(|(index, token)| sequence_number(token).map(|value| (index, value)))
    else {
        return SequelKey {
            base: tokens.join(" "),
            sequence: 1,
        };
    };
    let mut base_tokens = tokens[..index].to_vec();
    while base_tokens
        .last()
        .map(|token| sequence_prefix(token))
        .unwrap_or(false)
    {
        base_tokens.pop();
    }
    if base_tokens.is_empty() {
        base_tokens = tokens
            .iter()
            .enumerate()
            .filter(|(token_index, _)| *token_index != index)
            .map(|(_, token)| token.clone())
            .collect();
    }
    SequelKey {
        base: base_tokens.join(" "),
        sequence,
    }
}

fn normalize_sequence_token(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            other => other,
        })
        .collect()
}

fn sequence_prefix(value: &str) -> bool {
    matches!(
        value,
        "part"
            | "parte"
            | "chapter"
            | "capitulo"
            | "episode"
            | "episodio"
            | "numero"
            | "nro"
            | "no"
    )
}

fn sequence_number(value: &str) -> Option<u32> {
    if let Ok(number) = value.parse::<u32>() {
        return (1..=30).contains(&number).then_some(number);
    }
    match value {
        "one" | "uno" | "una" | "first" | "primera" | "primero" => Some(1),
        "ii" | "two" | "dos" | "second" | "segunda" | "segundo" => Some(2),
        "iii" | "three" | "tres" | "third" | "tercera" | "tercero" => Some(3),
        "iv" | "four" | "cuatro" | "fourth" | "cuarta" | "cuarto" => Some(4),
        "v" | "five" | "cinco" | "fifth" | "quinta" | "quinto" => Some(5),
        "vi" | "six" | "seis" | "sixth" | "sexta" | "sexto" => Some(6),
        "vii" | "seven" | "siete" | "seventh" | "septima" | "septimo" => Some(7),
        "viii" | "eight" | "ocho" | "eighth" | "octava" | "octavo" => Some(8),
        "ix" | "nine" | "nueve" | "ninth" | "novena" | "noveno" => Some(9),
        "x" | "ten" | "diez" | "tenth" | "decima" | "decimo" => Some(10),
        _ => None,
    }
}

fn normalize_tags(values: Vec<String>, limit: usize) -> Vec<String> {
    let mut seen = BTreeSet::new();
    values
        .into_iter()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .filter(|value| seen.insert(value.to_lowercase()))
        .take(limit)
        .collect()
}

fn infer_genres(file: &DiscoveredFile) -> Vec<String> {
    let haystack = format!(
        "{} {} {}",
        file.parsed.title,
        file.parsed.series_title.as_deref().unwrap_or_default(),
        file.path
    )
    .to_lowercase();
    let rules: [(&str, &[&str]); 10] = [
        (
            "Terror",
            &["horror", "terror", "creepers", "scream", "exorc", "demon"],
        ),
        (
            "Acción",
            &[
                "accion", "acción", "action", "matrix", "mision", "mission", "duro",
            ],
        ),
        ("Drama", &["drama", "vida", "life", "historia"]),
        ("Comedia", &["comedia", "comedy", "funny"]),
        (
            "Ciencia ficción",
            &[
                "sci-fi",
                "scifi",
                "ciencia ficcion",
                "ciencia ficción",
                "alien",
                "matrix",
            ],
        ),
        ("Suspenso", &["thriller", "suspenso", "suspense"]),
        (
            "Aventura",
            &["aventura", "adventure", "jurassic", "pirates"],
        ),
        (
            "Animación",
            &["animacion", "animación", "animation", "anime"],
        ),
        ("Documental", &["documental", "documentary"]),
        ("Romance", &["romance", "amor", "love"]),
    ];
    rules
        .iter()
        .filter_map(|(genre, needles)| {
            needles
                .iter()
                .any(|needle| haystack.contains(needle))
                .then(|| (*genre).to_owned())
        })
        .collect()
}

fn account_from_row(row: &Row<'_>) -> rusqlite::Result<AccountDto> {
    Ok(AccountDto {
        id: row.get(0)?,
        name: row.get(1)?,
    })
}

fn active_account_locked(conn: &Connection) -> Result<Option<AccountDto>> {
    let stored = conn
        .query_row(
            "SELECT value_json FROM settings WHERE key='active_account_id'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    let Some(stored) = stored else {
        return Ok(None);
    };
    let account_id: String = serde_json::from_str(&stored).unwrap_or_default();
    if account_id.is_empty() {
        return Ok(None);
    }
    conn.query_row(
        "SELECT id,name FROM accounts WHERE id=?1",
        [account_id],
        account_from_row,
    )
    .optional()
    .map_err(Into::into)
}

fn set_active_account_locked(conn: &Connection, account_id: &str, now: &str) -> Result<()> {
    let value = serde_json::to_string(account_id)?;
    conn.execute(
        "INSERT INTO settings(key,value_json,updated_at) VALUES('active_account_id',?1,?2) ON CONFLICT(key) DO UPDATE SET value_json=excluded.value_json,updated_at=excluded.updated_at",
        params![value, now],
    )?;
    Ok(())
}

fn migrate_legacy_user_state_locked(conn: &Connection, account_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO account_watch_progress(account_id,media_item_id,position_ms,duration_ms,completed,last_watched_at) SELECT ?1,media_item_id,position_ms,duration_ms,completed,last_watched_at FROM watch_progress",
        [account_id],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO account_user_flags(account_id,media_item_id,favorite,in_watchlist,updated_at) SELECT ?1,media_item_id,favorite,in_watchlist,updated_at FROM user_flags",
        [account_id],
    )?;
    conn.execute(
        "INSERT OR IGNORE INTO account_watch_history(id,account_id,media_item_id,played_at,position_ms) SELECT id,?1,media_item_id,played_at,position_ms FROM watch_history",
        [account_id],
    )?;
    Ok(())
}

fn validate_account_name(name: &str) -> Result<String> {
    let cleaned = name.trim();
    if cleaned.is_empty() {
        anyhow::bail!("El nombre no puede estar vacío");
    }
    if cleaned.chars().count() > 40 {
        anyhow::bail!("El nombre puede tener como máximo 40 caracteres");
    }
    Ok(cleaned.to_owned())
}

fn validate_password(password: &str) -> Result<()> {
    let length = password.chars().count();
    if !(4..=10).contains(&length) {
        anyhow::bail!("La contraseña tiene que tener entre 4 y 10 caracteres");
    }
    if !password.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        anyhow::bail!("La contraseña sólo puede usar letras y números");
    }
    Ok(())
}

fn normalize_account_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn password_hash(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(password.as_bytes());
    to_hex(&hasher.finalize())
}

fn to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn row_to_summary(row: &Row<'_>) -> rusqlite::Result<MediaSummary> {
    let kind: String = row.get(1)?;
    let position: i64 = row.get(8)?;
    let duration: i64 = row.get(9)?;
    Ok(MediaSummary {
        id: row.get(0)?,
        kind: if kind == "episode" {
            MediaKind::Episode
        } else {
            MediaKind::Movie
        },
        title: row.get(2)?,
        year: row.get(3)?,
        series_title: row.get(4)?,
        season_number: row.get(5)?,
        episode_number: row.get(6)?,
        overview: row.get(7)?,
        progress_percent: if duration > 0 {
            (position as f64 / duration as f64 * 100.0).clamp(0.0, 100.0)
        } else {
            0.0
        },
        completed: row.get::<_, i64>(10)? != 0,
        favorite: row.get::<_, i64>(11)? != 0,
        in_watchlist: row.get::<_, i64>(12)? != 0,
        offline: row.get::<_, i64>(13)? != 0,
        added_at: row.get(14)?,
        artwork_url: row.get(15)?,
        backdrop_url: row.get(16)?,
        preview_url: row.get(17)?,
        technical: MediaTechnical {
            duration_ms: row.get(18)?,
            width: row.get(19)?,
            height: row.get(20)?,
            container: row.get(21)?,
            video_codec: row.get(22)?,
            audio_codec: row.get(23)?,
            hdr_type: row.get(24)?,
        },
    })
}

fn cleanup_empty_series_locked(conn: &Connection) -> Result<()> {
    conn.execute(
        "DELETE FROM seasons WHERE NOT EXISTS (SELECT 1 FROM episodes e WHERE e.season_id=seasons.id)",
        [],
    )?;
    conn.execute(
        "DELETE FROM series WHERE NOT EXISTS (SELECT 1 FROM seasons se WHERE se.series_id=series.id)",
        [],
    )?;
    Ok(())
}

fn update_media_and_file(
    conn: &Connection,
    media_id: &str,
    file_id: &str,
    root_id: &str,
    scan_id: &str,
    file: &DiscoveredFile,
    now: &str,
) -> Result<()> {
    let genres_json = serde_json::to_string(&infer_genres(file))?;
    conn.execute("UPDATE media_items SET kind=?1,title=?2,sort_title=?3,year=?4,runtime_ms=?5,series_title=?6,season_number=?7,episode_number=?8,genres_json=?9,updated_at=?10 WHERE id=?11 AND manual_metadata=0 AND manual_classification=0", params![kind_text(&file.parsed.kind),file.parsed.title,file.parsed.title.to_lowercase(),file.parsed.year,file.technical.duration_ms,file.parsed.series_title,file.parsed.season_number,file.parsed.episode_number,genres_json,now,media_id])?;
    conn.execute("UPDATE media_files SET library_root_id=?1,path=?2,file_name=?3,file_size=?4,modified_at=?5,fingerprint=?6,container=?7,duration_ms=?8,width=?9,height=?10,video_codec=?11,audio_codec=?12,hdr_type=?13,offline=0,last_seen_scan=?14,updated_at=?15 WHERE id=?16", params![root_id,file.path,file.file_name,file.file_size,file.modified_at,file.fingerprint,file.technical.container,file.technical.duration_ms,file.technical.width,file.technical.height,file.technical.video_codec,file.technical.audio_codec,file.technical.hdr_type,scan_id,now,file_id])?;
    Ok(())
}

fn kind_text(kind: &MediaKind) -> &'static str {
    match kind {
        MediaKind::Movie => "movie",
        MediaKind::Episode => "episode",
    }
}
fn parse_root_status(value: &str) -> RootStatus {
    match value {
        "online" => RootStatus::Online,
        "scanning" => RootStatus::Scanning,
        "error" => RootStatus::Error,
        _ => RootStatus::Disconnected,
    }
}

fn daily_movie_rotation_bucket(date: chrono::NaiveDate) -> i32 {
    date.num_days_from_ce()
}

fn movie_rotation_key(media_id: &str, bucket: i32) -> [u8; 32] {
    Sha256::digest(format!("{bucket}:{media_id}").as_bytes()).into()
}

fn order_movies_for_rotation(movies: &mut [MediaSummary], bucket: i32) {
    movies.sort_by_cached_key(|movie| movie_rotation_key(&movie.id, bucket));
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use cinewana_core::parse_media_name;

    fn discovered_file(path: &str, fingerprint: &str) -> DiscoveredFile {
        let file_name = path.rsplit('\\').next().unwrap_or(path).to_owned();
        DiscoveredFile {
            path: path.into(),
            file_name: file_name.clone(),
            file_size: 10,
            modified_at: 1,
            fingerprint: fingerprint.into(),
            parsed: parse_media_name(Path::new(&file_name)),
            technical: MediaTechnical::default(),
            external_subtitles: vec![],
        }
    }

    #[test]
    fn movie_rotation_is_stable_for_a_day_and_changes_the_next_day() {
        let first_day = Utc.with_ymd_and_hms(2026, 7, 21, 3, 0, 0).unwrap();
        let same_day = Utc.with_ymd_and_hms(2026, 7, 21, 22, 59, 59).unwrap();
        let next_day = Utc.with_ymd_and_hms(2026, 7, 22, 3, 0, 0).unwrap();
        let ids = [
            "alien",
            "arrival",
            "blade-runner",
            "heat",
            "jaws",
            "moon",
            "parasite",
            "vertigo",
        ];
        let order = |bucket| {
            let mut values = ids;
            values.sort_by_cached_key(|id| movie_rotation_key(id, bucket));
            values
        };

        assert_eq!(
            daily_movie_rotation_bucket(first_day.date_naive()),
            daily_movie_rotation_bucket(same_day.date_naive())
        );
        assert_eq!(
            order(daily_movie_rotation_bucket(first_day.date_naive())),
            order(daily_movie_rotation_bucket(same_day.date_naive()))
        );
        assert_ne!(
            order(daily_movie_rotation_bucket(first_day.date_naive())),
            order(daily_movie_rotation_bucket(next_day.date_naive()))
        );
    }

    #[test]
    fn preserves_progress_when_file_goes_offline() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        db.start_scan(&root, "scan-1", "test").unwrap();
        let file = DiscoveredFile {
            path: r"D:\media\Movie.2020.mkv".into(),
            file_name: "Movie.2020.mkv".into(),
            file_size: 10,
            modified_at: 1,
            fingerprint: "10:1".into(),
            parsed: parse_media_name(Path::new("Movie.2020.mkv")),
            technical: MediaTechnical::default(),
            external_subtitles: vec![],
        };
        db.upsert_file(&root, "scan-1", &file).unwrap();
        db.finish_scan(&root, "scan-1", "completed", 1, 1, 0, 0)
            .unwrap();
        let account = db.create_account("Jael", "abcd1").unwrap();
        let id = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap()[0]
            .id
            .clone();
        db.save_progress(&account.id, &id, 50, 100).unwrap();
        db.start_scan(&root, "scan-2", "test").unwrap();
        db.finish_scan(&root, "scan-2", "completed", 0, 0, 0, 0)
            .unwrap();
        assert!(
            db.catalog(Some(&account.id), &CatalogQuery::default())
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            db.connection
                .lock()
                .query_row(
                    "SELECT position_ms FROM account_watch_progress WHERE account_id=?1 AND media_item_id=?2",
                    params![account.id, id],
                    |r| r.get::<_, i64>(0)
                )
                .unwrap(),
            50
        );
    }

    #[test]
    fn keeps_progress_separate_between_accounts() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        db.start_scan(&root, "scan-1", "test").unwrap();
        let file = DiscoveredFile {
            path: r"D:\media\Series.S01E01.mkv".into(),
            file_name: "Series.S01E01.mkv".into(),
            file_size: 10,
            modified_at: 1,
            fingerprint: "10:1".into(),
            parsed: parse_media_name(Path::new("Series.S01E01.mkv")),
            technical: MediaTechnical {
                duration_ms: Some(100),
                ..MediaTechnical::default()
            },
            external_subtitles: vec![],
        };
        db.upsert_file(&root, "scan-1", &file).unwrap();
        db.finish_scan(&root, "scan-1", "completed", 1, 1, 0, 0)
            .unwrap();
        let first = db.create_account("Cuenta Uno", "abcd1").unwrap();
        let second = db.create_account("Cuenta Dos", "abcd2").unwrap();
        let id = db
            .catalog(Some(&first.id), &CatalogQuery::default())
            .unwrap()[0]
            .id
            .clone();

        db.save_progress(&first.id, &id, 50, 100).unwrap();

        let first_item = db
            .catalog(Some(&first.id), &CatalogQuery::default())
            .unwrap()
            .pop()
            .unwrap();
        let second_item = db
            .catalog(Some(&second.id), &CatalogQuery::default())
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(first_item.progress_percent, 50.0);
        assert_eq!(second_item.progress_percent, 0.0);
    }

    #[test]
    fn validates_local_account_passwords() {
        let db = Database::open(":memory:").unwrap();
        assert!(db.create_account("Jael", "abc").is_err());
        assert!(db.create_account("Jael", "abcde123456").is_err());
        assert!(db.create_account("Jael", "abcd!").is_err());
        assert!(db.create_account("Jael", "abcd1").is_ok());
        assert!(db.login_account("Jael", "abcd2").is_err());
        assert!(db.login_account("Jael", "abcd1").is_ok());
    }

    #[test]
    fn finds_numbered_movie_sequels() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        db.start_scan(&root, "scan-1", "test").unwrap();
        for (path, fingerprint) in [
            (r"D:\media\Alien.1979.mkv", "10:1"),
            (r"D:\media\Alien.2.El.Regreso.1986.mkv", "11:1"),
            (r"D:\media\Alien.3.1992.mkv", "12:1"),
            (r"D:\media\Predator.1987.mkv", "13:1"),
        ] {
            db.upsert_file(&root, "scan-1", &discovered_file(path, fingerprint))
                .unwrap();
        }
        db.finish_scan(&root, "scan-1", "completed", 4, 4, 0, 0)
            .unwrap();
        let account = db.create_account("Jael", "abcd1").unwrap();
        let catalog = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap();
        let alien = catalog.iter().find(|item| item.title == "Alien").unwrap();
        let alien_2 = db
            .next_movie(Some(&account.id), &alien.id)
            .unwrap()
            .unwrap();
        assert_eq!(alien_2.title, "Alien 2 El Regreso");
        let alien_3 = db
            .next_movie(Some(&account.id), &alien_2.id)
            .unwrap()
            .unwrap();
        assert_eq!(alien_3.title, "Alien 3");
    }

    #[test]
    fn ignores_series_episodes_for_movie_sequels() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        db.start_scan(&root, "scan-1", "test").unwrap();
        db.upsert_file(
            &root,
            "scan-1",
            &discovered_file(r"D:\media\Alien.S01E01.mkv", "10:1"),
        )
        .unwrap();
        db.upsert_file(
            &root,
            "scan-1",
            &discovered_file(r"D:\media\Alien.2.1986.mkv", "11:1"),
        )
        .unwrap();
        db.finish_scan(&root, "scan-1", "completed", 2, 2, 0, 0)
            .unwrap();
        let account = db.create_account("Jael", "abcd1").unwrap();
        let catalog = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap();
        let episode = catalog
            .iter()
            .find(|item| item.kind == MediaKind::Episode)
            .unwrap();
        assert!(
            db.next_movie(Some(&account.id), &episode.id)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn next_up_advances_episodes_and_crosses_seasons() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        db.start_scan(&root, "scan-1", "test").unwrap();
        for (path, fingerprint) in [
            (r"D:\media\My.Show.S01E01.mkv", "10:1"),
            (r"D:\media\My.Show.S01E02.mkv", "11:1"),
            (r"D:\media\My.Show.S02E01.mkv", "12:1"),
            (r"D:\media\Other.Show.S01E02.mkv", "13:1"),
        ] {
            db.upsert_file(&root, "scan-1", &discovered_file(path, fingerprint))
                .unwrap();
        }
        db.finish_scan(&root, "scan-1", "completed", 4, 4, 0, 0)
            .unwrap();
        let account = db.create_account("Jael", "abcd1").unwrap();
        let catalog = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap();
        let episode = |season, number| {
            catalog
                .iter()
                .find(|item| {
                    item.series_title.as_deref() == Some("My Show")
                        && item.season_number == Some(season)
                        && item.episode_number == Some(number)
                })
                .unwrap()
        };

        let second = db
            .next_up(Some(&account.id), &episode(1, 1).id)
            .unwrap()
            .unwrap();
        assert_eq!((second.season_number, second.episode_number), (Some(1), Some(2)));

        let next_season = db
            .next_up(Some(&account.id), &second.id)
            .unwrap()
            .unwrap();
        assert_eq!(
            (next_season.season_number, next_season.episode_number),
            (Some(2), Some(1))
        );
        assert!(db
            .next_up(Some(&account.id), &next_season.id)
            .unwrap()
            .is_none());
    }

    #[test]
    fn next_up_recommends_a_similar_unwatched_movie() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        db.start_scan(&root, "scan-1", "test").unwrap();
        for (path, fingerprint) in [
            (r"D:\media\Source.Movie.2022.mkv", "10:1"),
            (r"D:\media\Watched.Match.2021.mkv", "11:1"),
            (r"D:\media\Unwatched.Match.2020.mkv", "12:1"),
            (r"D:\media\Unrelated.Movie.2022.mkv", "13:1"),
        ] {
            db.upsert_file(&root, "scan-1", &discovered_file(path, fingerprint))
                .unwrap();
        }
        db.finish_scan(&root, "scan-1", "completed", 4, 4, 0, 0)
            .unwrap();
        let account = db.create_account("Jael", "abcd1").unwrap();
        let catalog = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap();
        let id = |title: &str| catalog.iter().find(|item| item.title == title).unwrap().id.clone();
        let source_id = id("Source Movie");
        let watched_id = id("Watched Match");
        let unwatched_match_id = id("Unwatched Match");
        let genre_json = serde_json::to_string(&vec!["Terror"]).unwrap();
        for media_id in [&source_id, &watched_id, &unwatched_match_id] {
            db.connection
                .lock()
                .execute(
                    "UPDATE media_items SET genres_json=?1 WHERE id=?2",
                    params![genre_json, media_id],
                )
                .unwrap();
        }
        db.save_progress(&account.id, &watched_id, 100, 100)
            .unwrap();

        let recommendation = db
            .next_up(Some(&account.id), &source_id)
            .unwrap()
            .unwrap();
        assert_eq!(recommendation.id, unwatched_match_id);
        assert_eq!(recommendation.progress_percent, 0.0);
        assert!(!recommendation.completed);
    }

    #[test]
    fn updates_metadata_and_returns_genre_recommendations() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        db.start_scan(&root, "scan-1", "test").unwrap();
        db.upsert_file(
            &root,
            "scan-1",
            &discovered_file(r"D:\media\Horror.Movie.2022.mkv", "10:1"),
        )
        .unwrap();
        db.upsert_file(
            &root,
            "scan-1",
            &discovered_file(r"D:\media\Another.Horror.2021.mkv", "11:1"),
        )
        .unwrap();
        db.finish_scan(&root, "scan-1", "completed", 2, 2, 0, 0)
            .unwrap();
        let account = db.create_account("Jael", "abcd1").unwrap();
        let catalog = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap();
        let source_id = catalog
            .iter()
            .find(|item| item.title == "Horror Movie")
            .unwrap()
            .id
            .clone();
        let related_id = catalog
            .iter()
            .find(|item| item.title == "Another Horror")
            .unwrap()
            .id
            .clone();

        db.update_media_metadata(
            &source_id,
            &MediaMetadataUpdate {
                title: "Night House".into(),
                year: Some(2022),
                overview: Some("A personal description".into()),
                genres: vec!["Terror".into(), "Drama".into()],
                cast: vec!["Actor One".into(), "Actor Two".into()],
                poster_path: Some(r"C:\Users\jaell\poster.jpg".into()),
                backdrop_path: None,
            },
        )
        .unwrap();

        let detail = db
            .media_detail(Some(&account.id), &source_id)
            .unwrap()
            .unwrap();
        assert_eq!(detail.summary.title, "Night House");
        assert_eq!(
            detail.summary.overview.as_deref(),
            Some("A personal description")
        );
        assert_eq!(detail.genres, vec!["Terror", "Drama"]);
        assert_eq!(detail.cast, vec!["Actor One", "Actor Two"]);
        assert!(detail.manual_metadata);
        assert_eq!(
            detail.summary.artwork_url.as_deref(),
            Some(r"C:\Users\jaell\poster.jpg")
        );
        assert!(
            detail
                .recommendations
                .iter()
                .any(|item| item.id == related_id)
        );
    }

    #[test]
    fn renamed_episode_reuses_the_record_and_clears_its_review() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        let old_path = r"D:\media\La Casa Del Dragon Temporada 3\House.Of.The.Dragon.S03E04.mkv";
        let new_path = r"D:\media\La Casa Del Dragon Temporada 3\La.Casa.Del.Dragon.S03E04.mkv";
        let mut file = DiscoveredFile {
            path: old_path.into(),
            file_name: "House.Of.The.Dragon.S03E04.mkv".into(),
            file_size: 10,
            modified_at: 1,
            fingerprint: "same-content".into(),
            parsed: parse_media_name(Path::new(old_path)),
            technical: MediaTechnical::default(),
            external_subtitles: vec![],
        };
        db.start_scan(&root, "scan-1", "test").unwrap();
        let original = db.upsert_file(&root, "scan-1", &file).unwrap();
        assert_eq!(db.identification_reviews().unwrap().len(), 1);

        file.path = new_path.into();
        file.file_name = "La.Casa.Del.Dragon.S03E04.mkv".into();
        file.parsed = parse_media_name(Path::new(new_path));
        let renamed = db.upsert_file(&root, "targeted", &file).unwrap();

        assert_eq!(renamed.media_id, original.media_id);
        assert!(db.identification_reviews().unwrap().is_empty());
        assert_eq!(
            db.media_path(&original.media_id).unwrap().as_deref(),
            Some(new_path)
        );
        assert_eq!(
            db.connection
                .lock()
                .query_row("SELECT COUNT(*) FROM media_files", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            1
        );
    }

    #[test]
    fn manual_episode_identification_survives_later_scans() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        let path = r"D:\media\Mystery Show\Temporada 2\archivo sin numero.mkv";
        let file = DiscoveredFile {
            path: path.into(),
            file_name: "archivo sin numero.mkv".into(),
            file_size: 10,
            modified_at: 1,
            fingerprint: "manual-review".into(),
            parsed: parse_media_name(Path::new(path)),
            technical: MediaTechnical::default(),
            external_subtitles: vec![],
        };
        db.start_scan(&root, "scan-1", "test").unwrap();
        let outcome = db.upsert_file(&root, "scan-1", &file).unwrap();
        assert_eq!(db.identification_reviews().unwrap().len(), 1);

        db.resolve_identification(
            &outcome.media_id,
            &ClassificationUpdate {
                kind: MediaKind::Episode,
                title: "El comienzo".into(),
                series_title: Some("Mystery Show".into()),
                season_number: Some(2),
                episode_number: Some(1),
            },
        )
        .unwrap();
        assert!(db.identification_reviews().unwrap().is_empty());

        db.start_scan(&root, "scan-2", "test").unwrap();
        db.reconcile_unchanged_file("scan-2", path, 10, 1, &parse_media_name(Path::new(path)))
            .unwrap()
            .unwrap();
        let account = db.create_account("Jael", "abcd1").unwrap();
        let item = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(item.kind, MediaKind::Episode);
        assert_eq!(item.title, "El comienzo");
        assert_eq!(item.series_title.as_deref(), Some("Mystery Show"));
        assert_eq!(item.season_number, Some(2));
        assert_eq!(item.episode_number, Some(1));

        let home = db.home(Some(&account.id)).unwrap();
        assert_eq!(home.series.len(), 1);
        assert_eq!(home.series[0].season_items[0].episodes[0].id, item.id);
    }

    #[test]
    fn rescan_reclassifies_an_existing_movie_inside_uppercase_series_container() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        let path = r"D:\media\SERIES\Frieren\Temporada 1\Frieren 1.mkv";
        let mut file = discovered_file(path, "series-container");
        file.parsed = ParsedMediaName {
            kind: MediaKind::Movie,
            title: "Frieren 1".into(),
            year: None,
            series_title: None,
            season_number: None,
            episode_number: None,
            identification_source: "filename".into(),
            needs_review: false,
            review_reason: None,
        };
        db.start_scan(&root, "scan-1", "test").unwrap();
        let original = db.upsert_file(&root, "scan-1", &file).unwrap();

        db.reconcile_unchanged_file("scan-2", path, 10, 1, &parse_media_name(Path::new(path)))
            .unwrap()
            .unwrap();

        let account = db.create_account("Jael", "abcd1").unwrap();
        let item = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(item.id, original.media_id);
        assert_eq!(item.kind, MediaKind::Episode);
        assert_eq!(item.series_title.as_deref(), Some("Frieren"));
        assert_eq!(item.season_number, Some(1));
        assert_eq!(item.episode_number, Some(1));
    }

    #[test]
    fn rescan_reclassifies_an_existing_episode_inside_uppercase_movies_container() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        let path = r"D:\media\PELÍCULAS\Una Pelicula S01E01 2025.mkv";
        let mut file = discovered_file(path, "movies-container");
        file.parsed = ParsedMediaName {
            kind: MediaKind::Episode,
            title: "Episodio 1".into(),
            year: Some(2025),
            series_title: Some("Una Pelicula".into()),
            season_number: Some(1),
            episode_number: Some(1),
            identification_source: "filename".into(),
            needs_review: false,
            review_reason: None,
        };
        db.start_scan(&root, "scan-1", "test").unwrap();
        let original = db.upsert_file(&root, "scan-1", &file).unwrap();

        db.reconcile_unchanged_file("scan-2", path, 10, 1, &parse_media_name(Path::new(path)))
            .unwrap()
            .unwrap();

        let account = db.create_account("Jael", "abcd1").unwrap();
        let item = db
            .catalog(Some(&account.id), &CatalogQuery::default())
            .unwrap()
            .pop()
            .unwrap();
        assert_eq!(item.id, original.media_id);
        assert_eq!(item.kind, MediaKind::Movie);
        assert_eq!(item.title, "Una Pelicula S01E01");
        assert_eq!(item.year, Some(2025));
        assert_eq!(item.series_title, None);
        assert_eq!(item.season_number, None);
        assert_eq!(item.episode_number, None);
    }
}
