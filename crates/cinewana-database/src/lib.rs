use anyhow::{Context, Result};
use chrono::Utc;
use cinewana_core::{
    CatalogQuery, HomeDto, LibraryRootDto, MediaDetail, MediaKind, MediaSummary, MediaTechnical,
    MediaTrack, ParsedMediaName, RootStatus, SeriesSummary,
};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::{collections::BTreeMap, path::Path};
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

#[derive(Debug, Default, Clone, Copy)]
pub struct UpsertOutcome {
    pub inserted: bool,
    pub skipped: bool,
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
        let database = Self { connection: Mutex::new(connection) };
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
              runtime_ms INTEGER, series_title TEXT, season_number INTEGER, episode_number INTEGER,
              poster_cache_key TEXT, backdrop_cache_key TEXT, manual_metadata INTEGER NOT NULL DEFAULT 0,
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
            INSERT OR IGNORE INTO schema_migrations(version) VALUES(1);
            "#,
        )?;
        Ok(())
    }

    pub fn seed_root(&self, path: &str) -> Result<String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let display = Path::new(path).file_name().and_then(|s| s.to_str()).unwrap_or("Biblioteca");
        self.connection.lock().execute(
            "INSERT OR IGNORE INTO library_roots(id,path,display_name,status,created_at,updated_at) VALUES(?1,?2,?3,'disconnected',?4,?4)",
            params![id, path, display, now],
        )?;
        self.connection.lock().query_row("SELECT id FROM library_roots WHERE path=?1", [path], |r| r.get(0)).map_err(Into::into)
    }

    pub fn replace_root(&self, path: &str) -> Result<String> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::new_v4().to_string();
        let display = Path::new(path).file_name().and_then(|s| s.to_str()).filter(|s| !s.is_empty()).unwrap_or(path);
        let conn = self.connection.lock();
        conn.execute("UPDATE library_roots SET enabled=0,updated_at=?1", [&now])?;
        conn.execute(
            "INSERT INTO library_roots(id,path,display_name,enabled,recursive,watch_enabled,status,created_at,updated_at) VALUES(?1,?2,?3,1,1,1,'disconnected',?4,?4) ON CONFLICT(path) DO UPDATE SET enabled=1,display_name=excluded.display_name,updated_at=excluded.updated_at",
            params![id, path, display, now],
        )?;
        conn.query_row("SELECT id FROM library_roots WHERE path=?1", [path], |r| r.get(0)).map_err(Into::into)
    }

    pub fn roots(&self, include_local_path: bool) -> Result<Vec<LibraryRootDto>> {
        let conn = self.connection.lock();
        let mut statement = conn.prepare(
            "SELECT r.id,r.path,r.display_name,r.enabled,r.recursive,r.watch_enabled,r.status,r.last_scan_at,COALESCE(SUM(CASE WHEN f.offline=1 THEN 1 ELSE 0 END),0) FROM library_roots r LEFT JOIN media_files f ON f.library_root_id=r.id GROUP BY r.id ORDER BY r.created_at",
        )?;
        let rows = statement.query_map([], |r| {
            let status: String = r.get(6)?;
            Ok(LibraryRootDto {
                id: r.get(0)?, display_name: r.get(2)?, enabled: r.get::<_, i64>(3)? != 0,
                recursive: r.get::<_, i64>(4)? != 0, watch_enabled: r.get::<_, i64>(5)? != 0,
                status: parse_root_status(&status), last_scan_at: r.get(7)?, disconnected_count: r.get::<_, i64>(8)? as u64,
                local_path: include_local_path.then(|| r.get(1)).transpose()?,
            })
        })?;
        rows.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn enabled_roots_with_paths(&self) -> Result<Vec<(String, String)>> {
        let conn = self.connection.lock();
        let mut statement = conn.prepare("SELECT id,path FROM library_roots WHERE enabled=1 ORDER BY created_at")?;
        statement.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<rusqlite::Result<Vec<_>>>().map_err(Into::into)
    }

    pub fn set_root_status(&self, root_id: &str, status: &str) -> Result<()> {
        self.connection.lock().execute("UPDATE library_roots SET status=?1,updated_at=?2 WHERE id=?3", params![status, Utc::now().to_rfc3339(), root_id])?;
        Ok(())
    }

    pub fn start_scan(&self, root_id: &str, scan_id: &str, reason: &str) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        conn.execute("INSERT INTO scan_jobs(id,library_root_id,reason,status,started_at) VALUES(?1,?2,?3,'running',?4)", params![scan_id, root_id, reason, now])?;
        conn.execute("UPDATE library_roots SET status='scanning',updated_at=?1 WHERE id=?2", params![now, root_id])?;
        Ok(())
    }

    pub fn upsert_file(&self, root_id: &str, scan_id: &str, file: &DiscoveredFile) -> Result<UpsertOutcome> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        if let Some((file_id, media_id, size, modified)) = conn.query_row(
            "SELECT id,media_item_id,file_size,modified_at FROM media_files WHERE path=?1", [&file.path],
            |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?, r.get::<_, i64>(2)?, r.get::<_, i64>(3)?)),
        ).optional()? {
            if size == file.file_size && modified == file.modified_at {
                conn.execute("UPDATE media_files SET offline=0,last_seen_scan=?1,updated_at=?2 WHERE id=?3", params![scan_id, now, file_id])?;
                return Ok(UpsertOutcome { inserted: false, skipped: true });
            }
            update_media_and_file(&conn, &media_id, &file_id, root_id, scan_id, file, &now)?;
            self.replace_external_tracks_locked(&conn, &file_id, &file.external_subtitles)?;
            return Ok(UpsertOutcome { inserted: false, skipped: false });
        }

        if let Some((file_id, media_id)) = conn.query_row(
            "SELECT id,media_item_id FROM media_files WHERE library_root_id=?1 AND fingerprint=?2 LIMIT 1",
            params![root_id, file.fingerprint], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
        ).optional()? {
            update_media_and_file(&conn, &media_id, &file_id, root_id, scan_id, file, &now)?;
            self.replace_external_tracks_locked(&conn, &file_id, &file.external_subtitles)?;
            return Ok(UpsertOutcome { inserted: false, skipped: false });
        }

        let media_id = Uuid::new_v4().to_string();
        let file_id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO media_items(id,kind,title,sort_title,year,runtime_ms,series_title,season_number,episode_number,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
            params![media_id, kind_text(&file.parsed.kind), file.parsed.title, file.parsed.title.to_lowercase(), file.parsed.year, file.technical.duration_ms, file.parsed.series_title, file.parsed.season_number, file.parsed.episode_number, now],
        )?;
        conn.execute(
            "INSERT INTO media_files(id,media_item_id,library_root_id,path,file_name,file_size,modified_at,fingerprint,container,duration_ms,width,height,video_codec,audio_codec,hdr_type,offline,last_seen_scan,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,0,?16,?17,?17)",
            params![file_id, media_id, root_id, file.path, file.file_name, file.file_size, file.modified_at, file.fingerprint, file.technical.container, file.technical.duration_ms, file.technical.width, file.technical.height, file.technical.video_codec, file.technical.audio_codec, file.technical.hdr_type, scan_id, now],
        )?;
        self.ensure_episode_hierarchy_locked(&conn, &media_id, &file.parsed, &now)?;
        self.replace_external_tracks_locked(&conn, &file_id, &file.external_subtitles)?;
        Ok(UpsertOutcome { inserted: true, skipped: false })
    }

    fn ensure_episode_hierarchy_locked(&self, conn: &Connection, media_id: &str, parsed: &ParsedMediaName, now: &str) -> Result<()> {
        if parsed.kind != MediaKind::Episode { return Ok(()); }
        let title = parsed.series_title.as_deref().unwrap_or("Serie");
        let series_id = Uuid::new_v4().to_string();
        conn.execute("INSERT OR IGNORE INTO series(id,title,sort_title,created_at,updated_at) VALUES(?1,?2,?3,?4,?4)", params![series_id,title,title.to_lowercase(),now])?;
        let series_id: String = conn.query_row("SELECT id FROM series WHERE title=?1", [title], |r| r.get(0))?;
        let season_number = parsed.season_number.unwrap_or(0);
        let season_id = Uuid::new_v4().to_string();
        conn.execute("INSERT OR IGNORE INTO seasons(id,series_id,season_number,title) VALUES(?1,?2,?3,?4)", params![season_id,series_id,season_number,format!("Temporada {season_number}")])?;
        let season_id: String = conn.query_row("SELECT id FROM seasons WHERE series_id=?1 AND season_number=?2", params![series_id,season_number], |r| r.get(0))?;
        conn.execute("INSERT OR REPLACE INTO episodes(id,season_id,media_item_id,episode_number) VALUES(COALESCE((SELECT id FROM episodes WHERE media_item_id=?2),?1),?3,?2,?4)", params![Uuid::new_v4().to_string(),media_id,season_id,parsed.episode_number.unwrap_or(0)])?;
        Ok(())
    }

    fn replace_external_tracks_locked(&self, conn: &Connection, file_id: &str, subtitles: &[String]) -> Result<()> {
        conn.execute("DELETE FROM media_tracks WHERE media_file_id=?1 AND external_path IS NOT NULL", [file_id])?;
        for (index, subtitle) in subtitles.iter().enumerate() {
            conn.execute(
                "INSERT INTO media_tracks(id,media_file_id,track_type,stream_index,title,codec,external_path) VALUES(?1,?2,'subtitle',?3,?4,?5,?6)",
                params![Uuid::new_v4().to_string(), file_id, index as i32, Path::new(subtitle).file_name().and_then(|s| s.to_str()), Path::new(subtitle).extension().and_then(|s| s.to_str()), subtitle],
            )?;
        }
        Ok(())
    }

    pub fn finish_scan(&self, root_id: &str, scan_id: &str, status: &str, found: u64, processed: u64, skipped: u64, errors: u64) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        if status == "completed" {
            conn.execute("UPDATE media_files SET offline=1,updated_at=?1 WHERE library_root_id=?2 AND COALESCE(last_seen_scan,'')<>?3", params![now,root_id,scan_id])?;
        }
        conn.execute("UPDATE scan_jobs SET status=?1,found=?2,processed=?3,skipped=?4,errors=?5,finished_at=?6 WHERE id=?7", params![status,found,processed,skipped,errors,now,scan_id])?;
        conn.execute("UPDATE library_roots SET status=?1,last_scan_at=?2,updated_at=?2 WHERE id=?3", params![if status=="completed"{"online"}else{"error"},now,root_id])?;
        Ok(())
    }

    pub fn catalog(&self, query: &CatalogQuery) -> Result<Vec<MediaSummary>> {
        let conn = self.connection.lock();
        let mut statement = conn.prepare(MEDIA_SELECT)?;
        let mut items = statement.query_map([], row_to_summary)?.collect::<rusqlite::Result<Vec<_>>>()?;
        if let Some(search) = query.search.as_ref().map(|s| s.trim().to_lowercase()).filter(|s| !s.is_empty()) {
            items.retain(|m| m.title.to_lowercase().contains(&search) || m.series_title.as_deref().unwrap_or("").to_lowercase().contains(&search) || m.year.map(|y| y.to_string().contains(&search)).unwrap_or(false));
        }
        if let Some(kind) = query.kind.as_deref() {
            items.retain(|m| matches!((kind, &m.kind), ("movie", MediaKind::Movie) | ("episode", MediaKind::Episode) | ("series", MediaKind::Episode)));
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
            Some("title") => items.sort_by(|a,b| a.title.to_lowercase().cmp(&b.title.to_lowercase())),
            Some("year_desc") => items.sort_by(|a,b| b.year.cmp(&a.year)),
            Some("progress") => items.sort_by(|a,b| b.progress_percent.total_cmp(&a.progress_percent)),
            _ => items.sort_by(|a,b| b.added_at.cmp(&a.added_at)),
        }
        let offset = query.offset.unwrap_or(0) as usize;
        let limit = query.limit.unwrap_or(500).min(2000) as usize;
        Ok(items.into_iter().skip(offset).take(limit).collect())
    }

    pub fn home(&self) -> Result<HomeDto> {
        let all = self.catalog(&CatalogQuery::default())?;
        let movies: Vec<_> = all.iter().filter(|m| m.kind == MediaKind::Movie).cloned().collect();
        let continue_watching = all.iter().filter(|m| m.progress_percent > 0.0 && !m.completed).take(20).cloned().collect();
        let favorites = all.iter().filter(|m| m.favorite).take(20).cloned().collect();
        let heroes = all.iter().take(3).cloned().collect();
        let recently_added = all.iter().take(30).cloned().collect();
        let mut series_map: BTreeMap<String, (u32, u32, String)> = BTreeMap::new();
        for item in all.iter().filter(|m| m.kind == MediaKind::Episode) {
            let title = item.series_title.clone().unwrap_or_else(|| item.title.clone());
            let entry = series_map.entry(title).or_insert((0,0,item.added_at.clone()));
            entry.1 += 1;
            entry.0 = entry.0.max(item.season_number.unwrap_or(0).max(0) as u32);
            entry.2 = entry.2.clone().max(item.added_at.clone());
        }
        let series = series_map.into_iter().map(|(title,(seasons,episodes,latest_added_at))| SeriesSummary { title, seasons, episodes, artwork_url: None, latest_added_at }).collect();
        Ok(HomeDto { heroes, continue_watching, recently_added, movies, series, favorites })
    }

    pub fn media_detail(&self, id: &str) -> Result<Option<MediaDetail>> {
        let conn = self.connection.lock();
        let summary = conn.query_row(MEDIA_SELECT_BY_ID, [id], row_to_summary).optional()?;
        let Some(summary) = summary else { return Ok(None); };
        let (overview, genres_json, runtime_ms, file_name): (Option<String>,String,Option<i64>,String) = conn.query_row("SELECT m.overview,m.genres_json,COALESCE(m.runtime_ms,f.duration_ms),f.file_name FROM media_items m JOIN media_files f ON f.media_item_id=m.id WHERE m.id=?1", [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?)))?;
        let mut tracks_stmt = conn.prepare("SELECT t.id,t.track_type,t.stream_index,t.language,t.title,t.codec,t.channels,t.default_track,t.forced_track,t.external_path IS NOT NULL FROM media_tracks t JOIN media_files f ON f.id=t.media_file_id WHERE f.media_item_id=?1 ORDER BY t.track_type,t.stream_index")?;
        let tracks = tracks_stmt.query_map([id], |r| Ok(MediaTrack { id:r.get(0)?,track_type:r.get(1)?,stream_index:r.get(2)?,language:r.get(3)?,title:r.get(4)?,codec:r.get(5)?,channels:r.get(6)?,default_track:r.get::<_,i64>(7)?!=0,forced_track:r.get::<_,i64>(8)?!=0,external:r.get::<_,i64>(9)?!=0 }))?.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(Some(MediaDetail { summary, overview, genres: serde_json::from_str(&genres_json).unwrap_or_default(), runtime_ms, tracks, file_name }))
    }

    pub fn set_flag(&self, media_id: &str, flag: &str, value: bool) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        conn.execute("INSERT OR IGNORE INTO user_flags(media_item_id,updated_at) VALUES(?1,?2)", params![media_id,now])?;
        match flag {
            "favorite" => conn.execute("UPDATE user_flags SET favorite=?1,updated_at=?2 WHERE media_item_id=?3", params![value as i64,now,media_id])?,
            "watchlist" => conn.execute("UPDATE user_flags SET in_watchlist=?1,updated_at=?2 WHERE media_item_id=?3", params![value as i64,now,media_id])?,
            _ => anyhow::bail!("unsupported flag"),
        };
        Ok(())
    }

    pub fn save_progress(&self, media_id: &str, position_ms: i64, duration_ms: i64) -> Result<()> {
        let completed = duration_ms > 0 && ((position_ms as f64 / duration_ms as f64) >= 0.9 || duration_ms-position_ms <= 120_000);
        let now = Utc::now().to_rfc3339();
        let conn = self.connection.lock();
        conn.execute("INSERT INTO watch_progress(media_item_id,position_ms,duration_ms,completed,last_watched_at) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(media_item_id) DO UPDATE SET position_ms=excluded.position_ms,duration_ms=excluded.duration_ms,completed=excluded.completed,last_watched_at=excluded.last_watched_at", params![media_id,position_ms,duration_ms,completed as i64,now])?;
        conn.execute("INSERT INTO watch_history(id,media_item_id,played_at,position_ms) VALUES(?1,?2,?3,?4)", params![Uuid::new_v4().to_string(),media_id,now,position_ms])?;
        Ok(())
    }

    pub fn media_path(&self, media_id: &str) -> Result<Option<String>> {
        self.connection.lock().query_row("SELECT path FROM media_files WHERE media_item_id=?1 AND offline=0", [media_id], |r| r.get(0)).optional().map_err(Into::into)
    }

    pub fn backup_to(&self, target: &Path) -> Result<()> {
        let escaped = target.to_string_lossy().replace('\'', "''");
        self.connection.lock().execute_batch(&format!("VACUUM INTO '{escaped}'"))?;
        Ok(())
    }
}

const MEDIA_SELECT: &str = r#"
SELECT m.id,m.kind,m.title,m.year,m.series_title,m.season_number,m.episode_number,
       COALESCE(p.position_ms,0),COALESCE(p.duration_ms,COALESCE(m.runtime_ms,f.duration_ms,0)),COALESCE(p.completed,0),
       COALESCE(u.favorite,0),COALESCE(u.in_watchlist,0),f.offline,m.created_at,m.poster_cache_key,
       COALESCE(m.runtime_ms,f.duration_ms),f.width,f.height,f.container,f.video_codec,f.audio_codec,f.hdr_type
FROM media_items m JOIN media_files f ON f.media_item_id=m.id
LEFT JOIN watch_progress p ON p.media_item_id=m.id LEFT JOIN user_flags u ON u.media_item_id=m.id
WHERE f.offline=0 ORDER BY m.created_at DESC
"#;

const MEDIA_SELECT_BY_ID: &str = r#"
SELECT m.id,m.kind,m.title,m.year,m.series_title,m.season_number,m.episode_number,
       COALESCE(p.position_ms,0),COALESCE(p.duration_ms,COALESCE(m.runtime_ms,f.duration_ms,0)),COALESCE(p.completed,0),
       COALESCE(u.favorite,0),COALESCE(u.in_watchlist,0),f.offline,m.created_at,m.poster_cache_key,
       COALESCE(m.runtime_ms,f.duration_ms),f.width,f.height,f.container,f.video_codec,f.audio_codec,f.hdr_type
FROM media_items m JOIN media_files f ON f.media_item_id=m.id
LEFT JOIN watch_progress p ON p.media_item_id=m.id LEFT JOIN user_flags u ON u.media_item_id=m.id
WHERE f.offline=0 AND m.id=?1
"#;

fn row_to_summary(row: &Row<'_>) -> rusqlite::Result<MediaSummary> {
    let kind: String = row.get(1)?;
    let position: i64 = row.get(7)?;
    let duration: i64 = row.get(8)?;
    Ok(MediaSummary {
        id: row.get(0)?, kind: if kind=="episode"{MediaKind::Episode}else{MediaKind::Movie}, title: row.get(2)?, year: row.get(3)?, series_title: row.get(4)?, season_number: row.get(5)?, episode_number: row.get(6)?,
        progress_percent: if duration>0 { (position as f64/duration as f64*100.0).clamp(0.0,100.0) } else { 0.0 },
        completed: row.get::<_,i64>(9)?!=0, favorite: row.get::<_,i64>(10)?!=0, in_watchlist: row.get::<_,i64>(11)?!=0, offline: row.get::<_,i64>(12)?!=0,
        added_at: row.get(13)?, artwork_url: row.get(14)?, technical: MediaTechnical { duration_ms:row.get(15)?,width:row.get(16)?,height:row.get(17)?,container:row.get(18)?,video_codec:row.get(19)?,audio_codec:row.get(20)?,hdr_type:row.get(21)? },
    })
}

fn update_media_and_file(conn: &Connection, media_id: &str, file_id: &str, root_id: &str, scan_id: &str, file: &DiscoveredFile, now: &str) -> Result<()> {
    conn.execute("UPDATE media_items SET kind=?1,title=?2,sort_title=?3,year=?4,runtime_ms=?5,series_title=?6,season_number=?7,episode_number=?8,updated_at=?9 WHERE id=?10 AND manual_metadata=0", params![kind_text(&file.parsed.kind),file.parsed.title,file.parsed.title.to_lowercase(),file.parsed.year,file.technical.duration_ms,file.parsed.series_title,file.parsed.season_number,file.parsed.episode_number,now,media_id])?;
    conn.execute("UPDATE media_files SET library_root_id=?1,path=?2,file_name=?3,file_size=?4,modified_at=?5,fingerprint=?6,container=?7,duration_ms=?8,width=?9,height=?10,video_codec=?11,audio_codec=?12,hdr_type=?13,offline=0,last_seen_scan=?14,updated_at=?15 WHERE id=?16", params![root_id,file.path,file.file_name,file.file_size,file.modified_at,file.fingerprint,file.technical.container,file.technical.duration_ms,file.technical.width,file.technical.height,file.technical.video_codec,file.technical.audio_codec,file.technical.hdr_type,scan_id,now,file_id])?;
    Ok(())
}

fn kind_text(kind: &MediaKind) -> &'static str { match kind { MediaKind::Movie=>"movie", MediaKind::Episode=>"episode" } }
fn parse_root_status(value: &str) -> RootStatus { match value { "online"=>RootStatus::Online,"scanning"=>RootStatus::Scanning,"error"=>RootStatus::Error,_=>RootStatus::Disconnected } }

#[cfg(test)]
mod tests {
    use super::*;
    use cinewana_core::parse_media_name;

    #[test]
    fn preserves_progress_when_file_goes_offline() {
        let db = Database::open(":memory:").unwrap();
        let root = db.seed_root(r"D:\media").unwrap();
        db.start_scan(&root,"scan-1","test").unwrap();
        let file = DiscoveredFile { path:r"D:\media\Movie.2020.mkv".into(),file_name:"Movie.2020.mkv".into(),file_size:10,modified_at:1,fingerprint:"10:1".into(),parsed:parse_media_name(Path::new("Movie.2020.mkv")),technical:MediaTechnical::default(),external_subtitles:vec![] };
        db.upsert_file(&root,"scan-1",&file).unwrap();
        db.finish_scan(&root,"scan-1","completed",1,1,0,0).unwrap();
        let id = db.catalog(&CatalogQuery::default()).unwrap()[0].id.clone();
        db.save_progress(&id,50,100).unwrap();
        db.start_scan(&root,"scan-2","test").unwrap();
        db.finish_scan(&root,"scan-2","completed",0,0,0,0).unwrap();
        assert!(db.catalog(&CatalogQuery::default()).unwrap().is_empty());
        assert_eq!(db.connection.lock().query_row("SELECT position_ms FROM watch_progress WHERE media_item_id=?1",[id],|r|r.get::<_,i64>(0)).unwrap(),50);
    }
}
