use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::Path;

pub const DEFAULT_LIBRARY_ROOT: &str = r"D:\peliculas-y-series";
pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "mkv", "mp4", "m4v", "avi", "mov", "webm", "ts", "m2ts", "mpg", "mpeg", "wmv", "flv",
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Movie,
    Episode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RootStatus {
    Online,
    Disconnected,
    Scanning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryRootDto {
    pub id: String,
    pub display_name: String,
    pub enabled: bool,
    pub recursive: bool,
    pub watch_enabled: bool,
    pub status: RootStatus,
    pub last_scan_at: Option<String>,
    pub disconnected_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountDto {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaTechnical {
    pub duration_ms: Option<i64>,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub container: Option<String>,
    pub video_codec: Option<String>,
    pub audio_codec: Option<String>,
    pub hdr_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSummary {
    pub id: String,
    pub kind: MediaKind,
    pub title: String,
    pub year: Option<i32>,
    pub series_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub progress_percent: f64,
    pub favorite: bool,
    pub in_watchlist: bool,
    pub completed: bool,
    pub offline: bool,
    pub added_at: String,
    pub artwork_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub preview_url: Option<String>,
    pub technical: MediaTechnical,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaTrack {
    pub id: String,
    pub track_type: String,
    pub stream_index: i32,
    pub language: Option<String>,
    pub title: Option<String>,
    pub codec: Option<String>,
    pub channels: Option<i32>,
    pub default_track: bool,
    pub forced_track: bool,
    pub external: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaDetail {
    #[serde(flatten)]
    pub summary: MediaSummary,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<String>,
    pub runtime_ms: Option<i64>,
    pub tracks: Vec<MediaTrack>,
    pub file_name: String,
    pub manual_metadata: bool,
    pub metadata_status: String,
    pub metadata_source_url: Option<String>,
    pub metadata_imported_at: Option<String>,
    pub metadata_candidates: Vec<MediaMetadataCandidate>,
    pub recommendations: Vec<MediaSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadataCandidate {
    pub id: String,
    pub language: String,
    pub page_id: i64,
    pub title: String,
    pub year: Option<i32>,
    pub description: Option<String>,
    pub source_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaMetadataUpdate {
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<String>,
    pub poster_path: Option<String>,
    pub backdrop_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ImportedMediaMetadata {
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub cast: Vec<String>,
    pub source_url: String,
    pub source_language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ScanProgress {
    pub job_id: Option<String>,
    pub running: bool,
    pub cancel_requested: bool,
    pub found: u64,
    pub processed: u64,
    pub skipped: u64,
    pub errors: u64,
    pub current_file: Option<String>,
    pub percent: f64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogQuery {
    pub search: Option<String>,
    pub kind: Option<String>,
    pub filter: Option<String>,
    pub sort: Option<String>,
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

impl Default for CatalogQuery {
    fn default() -> Self {
        Self {
            search: None,
            kind: None,
            filter: None,
            sort: Some("added_desc".into()),
            limit: Some(500),
            offset: Some(0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct HomeDto {
    pub heroes: Vec<MediaSummary>,
    pub continue_watching: Vec<MediaSummary>,
    pub recently_added: Vec<MediaSummary>,
    pub movies: Vec<MediaSummary>,
    pub series: Vec<SeriesSummary>,
    pub favorites: Vec<MediaSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesSummary {
    pub episode_id: String,
    pub title: String,
    pub seasons: u32,
    pub episodes: u32,
    pub artwork_url: Option<String>,
    pub latest_added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BootstrapDto {
    pub roots: Vec<LibraryRootDto>,
    pub scan: ScanProgress,
    pub home: HomeDto,
    pub accounts: Vec<AccountDto>,
    pub active_account: Option<AccountDto>,
    pub ffprobe_available: bool,
    pub player_available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageProfile {
    pub brightness: f64,
    pub contrast: f64,
    pub gamma: f64,
    pub saturation: f64,
    pub hue: f64,
    pub temperature: f64,
    pub shadows: f64,
    pub highlights: f64,
    pub black_level: f64,
    pub sharpness: f64,
    pub noise_reduction: f64,
}

impl Default for ImageProfile {
    fn default() -> Self {
        Self {
            brightness: 0.0,
            contrast: 0.0,
            gamma: 0.0,
            saturation: 0.0,
            hue: 0.0,
            temperature: 0.0,
            shadows: 0.0,
            highlights: 0.0,
            black_level: 0.0,
            sharpness: 0.0,
            noise_reduction: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub media_id: Option<String>,
    pub title: Option<String>,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub volume: f64,
    pub muted: bool,
    pub playback_speed: f64,
    pub audio_track_id: Option<String>,
    pub subtitle_track_id: Option<String>,
    pub quality: String,
    pub fullscreen: bool,
    pub available: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PlayerCommand {
    Play { media_id: Option<String> },
    Pause,
    TogglePlayback,
    Stop,
    SeekAbsolute { position_ms: i64 },
    SeekRelative { seconds: f64 },
    SetVolume { value: f64 },
    ToggleMute,
    SelectAudioTrack { track_id: String },
    SelectSubtitleTrack { track_id: String },
    SetPlaybackSpeed { value: f64 },
    SetImageProfile { profile: ImageProfile },
    SetQuality { quality: String },
    SetFullscreen { fullscreen: bool },
    GetPlayerState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMediaName {
    pub kind: MediaKind,
    pub title: String,
    pub year: Option<i32>,
    pub series_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
}

pub fn is_supported_video(path: &Path) -> bool {
    path.extension()
        .and_then(|v| v.to_str())
        .map(|v| SUPPORTED_EXTENSIONS.contains(&v.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

pub fn parse_media_name(path: &Path) -> ParsedMediaName {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let normalized = stem.replace(['.', '_'], " ");
    let episode_re = Regex::new(r"(?i)^(?P<series>.*?)[\s.-]*(?:S(?P<s>\d{1,2})E(?P<e>\d{1,3})|(?P<s2>\d{1,2})x(?P<e2>\d{1,3}))(?:\b|[\s._-])").unwrap();
    if let Some(caps) = episode_re.captures(&normalized) {
        let series = clean_title(caps.name("series").map(|m| m.as_str()).unwrap_or_default());
        let season = caps
            .name("s")
            .or_else(|| caps.name("s2"))
            .and_then(|m| m.as_str().parse().ok());
        let episode = caps
            .name("e")
            .or_else(|| caps.name("e2"))
            .and_then(|m| m.as_str().parse().ok());
        let year = extract_year(&normalized);
        return ParsedMediaName {
            kind: MediaKind::Episode,
            title: format!("Episodio {}", episode.unwrap_or_default()),
            year,
            series_title: Some(series.clone()),
            season_number: season,
            episode_number: episode,
        };
    }
    let year = extract_year(&normalized);
    let before_year = if let Some(year) = year {
        normalized
            .split(&year.to_string())
            .next()
            .unwrap_or(&normalized)
    } else {
        normalized.as_str()
    };
    ParsedMediaName {
        kind: MediaKind::Movie,
        title: clean_title(before_year),
        year,
        series_title: None,
        season_number: None,
        episode_number: None,
    }
}

fn extract_year(value: &str) -> Option<i32> {
    Regex::new(r"(?:^|\D)((?:19|20)\d{2})(?:\D|$)")
        .unwrap()
        .captures(value)
        .and_then(|c| c.get(1))
        .and_then(|m| m.as_str().parse().ok())
}

fn clean_title(value: &str) -> String {
    let noise = Regex::new(r"(?i)\b(?:2160p|1080p|720p|480p|uhd|bluray|brrip|webrip|web-dl|x26[45]|h26[45]|hevc|hdr10?|dv|dual|latino|lat|castellano|aac|dts|ac3)\b.*$").unwrap();
    let stripped = noise.replace(value, "");
    let spaces = Regex::new(r"\s+").unwrap();
    let cleaned = spaces.replace_all(
        stripped.trim_matches(|c: char| c.is_whitespace() || "-._[]()".contains(c)),
        " ",
    );
    if cleaned.is_empty() {
        "Sin título".into()
    } else {
        cleaned.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_movie_with_year_and_noise() {
        let parsed = parse_media_name(Path::new("Alien.2.El.Regreso.1986.1080P-Dual-Lat.mp4"));
        assert_eq!(parsed.kind, MediaKind::Movie);
        assert_eq!(parsed.title, "Alien 2 El Regreso");
        assert_eq!(parsed.year, Some(1986));
    }

    #[test]
    fn parses_unicode_series_episode() {
        let parsed = parse_media_name(Path::new(
            "La.Casa.Del.Dragón.S03e02.2026.1080P-Dual-Lat.mkv",
        ));
        assert_eq!(parsed.kind, MediaKind::Episode);
        assert_eq!(parsed.series_title.as_deref(), Some("La Casa Del Dragón"));
        assert_eq!(parsed.season_number, Some(3));
        assert_eq!(parsed.episode_number, Some(2));
    }
}
