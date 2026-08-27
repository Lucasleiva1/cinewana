pub mod genres;
pub mod sagas;

use genres::SERIES_PREFIX;
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
    pub overview: Option<String>,
    pub progress_percent: f64,
    pub favorite: bool,
    pub in_watchlist: bool,
    pub completed: bool,
    pub offline: bool,
    pub added_at: String,
    /// Cuándo se vio por última vez.
    ///
    /// Es lo que ordena «Continuar viendo». El dato siempre estuvo guardado, pero no llegaba hasta
    /// acá, así que la fila terminaba ordenada por fecha de alta en la biblioteca —que no tiene
    /// nada que ver con lo último que miraste— y la película recién vista podía no aparecer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_watched_at: Option<String>,
    pub artwork_url: Option<String>,
    pub backdrop_url: Option<String>,
    pub preview_url: Option<String>,
    pub technical: MediaTechnical,
    /// Canonical categories this title is shelved under. Never empty for a shelved title: a title
    /// with no usable genre carries [`genres::UNCATEGORIZED_LABEL`].
    #[serde(default)]
    pub categories: Vec<String>,
    /// Whether the stored sheet is missing a genre or a synopsis and needs manual review.
    #[serde(default)]
    pub incomplete: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saga_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saga_title: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saga_position: Option<i32>,
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
    /// Direction, writing and cast, in that order, each with its photo when the provider has one.
    #[serde(default)]
    pub people: Vec<MediaPerson>,
}

/// What a person did in a title.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PersonRole {
    Director,
    Writer,
    Actor,
}

/// One person credited on a title, with the photo that travels beside the video.
///
/// The same shape serves the three stages of the trip: the importer fills `photo_source` with the
/// provider URL, the cache fills `photo_url` with the local path the interface renders, and the
/// portable package fills `photo_file` with the name of the copy stored next to the movie.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MediaPerson {
    pub name: String,
    pub role: PersonRole,
    /// Character played, only for actors.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    /// Absolute path of the cached photo; the interface turns it into an asset URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo_url: Option<String>,
    /// Name of the photo inside the portable `cast` folder.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo_file: Option<String>,
    /// Provider URL, kept so a missing photo can be downloaded again.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub photo_source: Option<String>,
}

/// How many actors are kept per title.
///
/// The provider answers ordered by billing, and the tail grows without limit: 9 names for `Alien`,
/// 103 for `Avengers: Endgame`. Ten covers the people an audience recognizes and keeps the sheet
/// readable on every title.
pub const MAX_CAST: usize = 10;

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
    #[serde(default)]
    pub poster_url: Option<String>,
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
    pub provider: String,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<String>,
    pub source_url: String,
    pub source_language: String,
    pub poster_url: Option<String>,
    pub backdrop_url: Option<String>,
    /// Provider collection the title belongs to, when the provider knows of one.
    #[serde(default)]
    pub collection_id: Option<String>,
    #[serde(default)]
    pub collection_name: Option<String>,
    /// Credited people carrying the provider URL of each photo.
    #[serde(default)]
    pub people: Vec<MediaPerson>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PortableMediaMetadata {
    pub schema_version: u32,
    pub portable_id: String,
    pub video_file_name: String,
    pub file_size: i64,
    pub fingerprint: String,
    pub kind: MediaKind,
    pub title: String,
    pub year: Option<i32>,
    pub overview: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<String>,
    pub series_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub identification_source: String,
    pub needs_review: bool,
    pub review_reason: Option<String>,
    pub manual_classification: bool,
    pub manual_metadata: bool,
    pub metadata_status: String,
    pub metadata_source_url: Option<String>,
    pub metadata_imported_at: Option<String>,
    pub metadata_candidates: Vec<MediaMetadataCandidate>,
    pub poster_file: Option<String>,
    pub backdrop_file: Option<String>,
    /// Collection this title belongs to, so sagas survive moving the drive to another computer.
    #[serde(default)]
    pub saga_id: Option<String>,
    #[serde(default)]
    pub saga_title: Option<String>,
    #[serde(default)]
    pub saga_position: Option<i32>,
    /// Credited people whose photos live in the `cast` folder beside this metadata file.
    #[serde(default)]
    pub people: Vec<MediaPerson>,
    pub updated_at: String,
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
    /// Shelves in the order this account chose, with hidden shelves already removed.
    #[serde(default)]
    pub categories: Vec<CategoryRow>,
    /// Every shelf known to the library, including hidden ones, for the reorder screen.
    #[serde(default)]
    pub category_settings: Vec<CategoryOption>,
    /// Look of the category strip chosen by this account: `gold` or `dark`.
    #[serde(default)]
    pub category_style: String,
    /// Shelves the account created, with their membership, for the assignment controls.
    #[serde(default)]
    pub custom_categories: Vec<CustomCategory>,
    /// Whether a shelf can be dragged sideways with the pointer. Off by default: the grab cursor
    /// sits on top of every poster and gets in the way of simply opening a title.
    #[serde(default)]
    pub carousel_drag: bool,
}

/// The two looks available for the category strip.
pub const CATEGORY_STYLES: &[(&str, &str)] = &[("gold", "Dorada"), ("dark", "Sobria")];
/// Look used until the account picks another one.
pub const DEFAULT_CATEGORY_STYLE: &str = "gold";

/// What a shelf holds, so the interface knows which card to draw.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CategoryKind {
    /// Movies of one genre.
    Movies,
    /// Series, either every show or the ones of one genre.
    Series,
    /// Movie collections.
    Sagas,
    /// A shelf the account created and fills by hand.
    Custom,
    /// Titles missing a genre or a synopsis.
    Uncategorized,
}

/// A shelf the account created, holding whatever titles it assigned by hand.
///
/// Movies are referenced by media id. Series are referenced by title because a show's identity in
/// the catalog is its name: the episode standing in for it changes whenever a newer one is added.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CustomCategory {
    pub id: String,
    pub label: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub series: Vec<String>,
}

/// Identifier of the shelf that holds every series regardless of genre.
pub const ALL_SERIES_ID: &str = "series";
/// Visible label for [`ALL_SERIES_ID`].
pub const ALL_SERIES_LABEL: &str = "Series";
/// Prefix marking a shelf the account created.
pub const CUSTOM_PREFIX: &str = "custom:";

/// One shelf of the home screen.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryRow {
    pub id: String,
    pub label: String,
    pub kind: CategoryKind,
    pub count: u32,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub items: Vec<MediaSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub series: Vec<SeriesSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub sagas: Vec<SagaSummary>,
}

/// A shelf as listed on the reorder screen, whether or not it is currently visible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryOption {
    pub id: String,
    pub label: String,
    pub kind: CategoryKind,
    pub count: u32,
    pub hidden: bool,
}

/// One saved shelf preference. Order inside the vector is the order on screen.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CategoryPreference {
    pub id: String,
    #[serde(default)]
    pub hidden: bool,
}

/// A movie collection: `Chucky 1`, `Chucky 2`, `Chucky 3`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SagaSummary {
    pub id: String,
    pub title: String,
    pub artwork_url: Option<String>,
    /// Members ordered by their position inside the collection.
    pub items: Vec<MediaSummary>,
}

impl CategoryRow {
    /// Identifier of the shelf holding movies of `genre`.
    pub fn movies_id(genre: &str) -> String {
        genres::genre_slug(genre)
            .map(str::to_owned)
            .unwrap_or_else(|| genres::fold(genre).replace(' ', "-"))
    }

    /// Identifier of the shelf holding series of `genre`.
    pub fn series_id(genre: &str) -> String {
        format!("{SERIES_PREFIX}{}", Self::movies_id(genre))
    }
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
    pub season_items: Vec<SeriesSeasonSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeriesSeasonSummary {
    pub season_number: i32,
    pub title: String,
    pub episodes: Vec<MediaSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdentificationReview {
    pub media_id: String,
    pub file_name: String,
    pub kind: MediaKind,
    pub title: String,
    pub series_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
    pub reason: String,
    pub identification_pending: bool,
    pub metadata_status: String,
    pub metadata_candidates: Vec<MediaMetadataCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassificationUpdate {
    pub kind: MediaKind,
    pub title: String,
    pub series_title: Option<String>,
    pub season_number: Option<i32>,
    pub episode_number: Option<i32>,
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
    pub identification_reviews: Vec<IdentificationReview>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImageAdjustment {
    pub brightness: i32,
    pub contrast: i32,
    pub saturation: i32,
    pub shadows: i32,
    pub highlights: i32,
    pub temperature: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ImageAnalysis {
    pub average_light: u32,
    pub shadows_percent: u32,
    pub highlights_percent: u32,
    pub average_saturation: u32,
    pub warmth: i32,
    pub sampled_frames: u32,
    pub suggested: ImageAdjustment,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ImageAnalysisProgress {
    pub media_id: String,
    pub running: bool,
    pub processed_frames: u32,
    pub total_frames: u32,
    pub sampled_frames: u32,
    pub percent: f64,
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
    pub identification_source: String,
    pub needs_review: bool,
    pub review_reason: Option<String>,
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
    let forced_container = infer_forced_media_container(path);
    if forced_container == Some(ForcedMediaContainer::Movies) {
        let year = extract_year(&normalized);
        let before_year = year
            .and_then(|year| normalized.split(&year.to_string()).next())
            .unwrap_or(&normalized);
        return ParsedMediaName {
            kind: MediaKind::Movie,
            title: clean_title(before_year),
            year,
            series_title: None,
            season_number: None,
            episode_number: None,
            identification_source: "movie_container".into(),
            needs_review: false,
            review_reason: None,
        };
    }
    let folder_context = infer_series_folder_context(path);
    let forced_series_context = (forced_container == Some(ForcedMediaContainer::Series))
        .then(|| infer_forced_series_context(path))
        .flatten();
    let episode_re = Regex::new(
        r"(?i)^(?P<series>.*?)[\s.-]*(?:S(?P<s>\d{1,2})E(?P<e>\d{1,3})|(?P<s2>\d{1,2})x(?P<e2>\d{1,3})|(?:temporada|season)\s*0?(?P<s3>\d{1,2})[\s.-]*(?:episodio|episode|ep|cap(?:itulo|.tulo)?|chapter)\s*0?(?P<e3>\d{1,3}))(?:\b|[\s._-])",
    )
    .unwrap();
    if let Some(caps) = episode_re.captures(&normalized) {
        let parsed_series =
            clean_title(caps.name("series").map(|m| m.as_str()).unwrap_or_default());
        let parsed_season = caps
            .name("s")
            .or_else(|| caps.name("s2"))
            .or_else(|| caps.name("s3"))
            .and_then(|m| m.as_str().parse().ok());
        let episode = caps
            .name("e")
            .or_else(|| caps.name("e2"))
            .or_else(|| caps.name("e3"))
            .and_then(|m| m.as_str().parse().ok());
        let matched = caps.get(0).expect("episode capture has a full match");
        let folder_conflicts = folder_context.as_ref().is_some_and(|context| {
            (!parsed_series.starts_with("Sin t")
                && !context.series_title.eq_ignore_ascii_case(&parsed_series))
                || parsed_season.is_some_and(|season| season != context.season_number)
        });
        let missing_series = parsed_series.starts_with("Sin t")
            && folder_context.is_none()
            && forced_series_context
                .as_ref()
                .and_then(|context| context.series_title.as_ref())
                .is_none();
        let (series, season, source) = folder_context
            .as_ref()
            .map(|context| {
                (
                    context.series_title.clone(),
                    Some(context.season_number),
                    "folder_and_filename".to_string(),
                )
            })
            .or_else(|| {
                forced_series_context.as_ref().map(|context| {
                    (
                        context
                            .series_title
                            .clone()
                            .unwrap_or_else(|| parsed_series.clone()),
                        Some(context.season_number),
                        "series_container_and_filename".to_string(),
                    )
                })
            })
            .unwrap_or((parsed_series, parsed_season, "filename".to_string()));
        return ParsedMediaName {
            kind: MediaKind::Episode,
            title: episode_title(&normalized[matched.end()..], episode),
            year: extract_year(&normalized),
            series_title: Some(series),
            season_number: season,
            episode_number: episode,
            identification_source: source,
            needs_review: folder_conflicts || missing_series,
            review_reason: if folder_conflicts {
                Some(
                    "La carpeta y el archivo identifican la serie o temporada de forma distinta"
                        .into(),
                )
            } else if missing_series {
                Some("Se detecto el episodio, pero no el nombre de la serie".into())
            } else {
                None
            },
        };
    }

    if let Some(context) = folder_context.as_ref() {
        if let Some((episode, marker_end)) = extract_episode_number(&normalized) {
            return ParsedMediaName {
                kind: MediaKind::Episode,
                title: episode_title(&normalized[marker_end..], Some(episode)),
                year: extract_year(&normalized),
                series_title: Some(context.series_title.clone()),
                season_number: Some(context.season_number),
                episode_number: Some(episode),
                identification_source: "folder_and_filename".into(),
                needs_review: false,
                review_reason: None,
            };
        }
    }
    if let Some(context) = forced_series_context.as_ref() {
        if let Some((episode, marker_start, marker_end)) =
            extract_forced_episode_number(&normalized)
        {
            let derived_series = derive_forced_series_title(&normalized[..marker_start]);
            let series_title = context.series_title.clone().or(derived_series);
            let missing_series = series_title.is_none();
            return ParsedMediaName {
                kind: MediaKind::Episode,
                title: episode_title(&normalized[marker_end..], Some(episode)),
                year: extract_year(&normalized),
                series_title: Some(series_title.unwrap_or_else(|| "Sin título".into())),
                season_number: Some(context.season_number),
                episode_number: Some(episode),
                identification_source: "series_container_and_filename".into(),
                needs_review: missing_series,
                review_reason: missing_series.then(|| {
                    "La carpeta Serie fuerza un episodio, pero falta el nombre de la serie".into()
                }),
            };
        }

        let series_title = context
            .series_title
            .clone()
            .or_else(|| derive_forced_series_title(&normalized));
        let missing_series = series_title.is_none();
        return ParsedMediaName {
            kind: MediaKind::Episode,
            title: clean_title(&normalized),
            year: extract_year(&normalized),
            series_title: Some(series_title.unwrap_or_else(|| "Sin título".into())),
            season_number: Some(context.season_number),
            episode_number: None,
            identification_source: "series_container".into(),
            needs_review: true,
            review_reason: Some(if missing_series {
                "La carpeta Serie fuerza una serie, pero faltan el nombre y el número de episodio"
                    .into()
            } else {
                "La carpeta Serie fuerza una serie, pero no se encontró el número de episodio"
                    .into()
            }),
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
        identification_source: if folder_context.is_some() {
            "folder".into()
        } else {
            "filename".into()
        },
        needs_review: folder_context.is_some(),
        review_reason: folder_context.map(|_| {
            "La carpeta parece una temporada, pero no se encontro el numero de episodio".into()
        }),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SeriesFolderContext {
    series_title: String,
    season_number: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ForcedMediaContainer {
    Series,
    Movies,
}

fn infer_forced_media_container(path: &Path) -> Option<ForcedMediaContainer> {
    path.ancestors()
        .skip(1)
        .filter_map(|folder| folder.file_name().and_then(|value| value.to_str()))
        .find_map(|name| {
            if is_series_container_name(name) {
                Some(ForcedMediaContainer::Series)
            } else if is_movie_container_name(name) {
                Some(ForcedMediaContainer::Movies)
            } else {
                None
            }
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ForcedSeriesContext {
    series_title: Option<String>,
    season_number: i32,
}

fn infer_forced_series_context(path: &Path) -> Option<ForcedSeriesContext> {
    let folders = path
        .ancestors()
        .skip(1)
        .filter_map(|folder| {
            folder
                .file_name()
                .and_then(|value| value.to_str())
                .map(|name| (folder, name))
        })
        .collect::<Vec<_>>();
    let container_index = folders
        .iter()
        .position(|(_, name)| is_series_container_name(name))?;
    let below_container = &folders[..container_index];
    let season = below_container
        .iter()
        .enumerate()
        .find_map(|(index, (_, name))| {
            parse_season_folder_number(name).map(|number| (index, number))
        });
    let season_number = season.map(|(_, number)| number).unwrap_or(1);
    let series_folder = if let Some((season_index, _)) = season {
        below_container
            .iter()
            .skip(season_index + 1)
            .find(|(_, name)| parse_season_folder_number(name).is_none())
    } else {
        below_container.first()
    };
    let series_title = series_folder
        .map(|(_, name)| clean_context_title(name))
        .filter(|title| !title.starts_with("Sin t"));

    Some(ForcedSeriesContext {
        series_title,
        season_number,
    })
}

fn is_series_container_name(value: &str) -> bool {
    matches!(value.trim().to_lowercase().as_str(), "serie" | "series")
}

fn is_movie_container_name(value: &str) -> bool {
    matches!(
        value.trim().to_lowercase().as_str(),
        "película" | "películas" | "pelicula" | "peliculas"
    )
}

fn parse_season_folder_number(value: &str) -> Option<i32> {
    Regex::new(r"(?i)^(?:temporada|season|s)\s*0?(?P<s>\d{1,2})(?:$|[\s._-])")
        .unwrap()
        .captures(&value.replace(['.', '_'], " "))
        .and_then(|caps| caps.name("s"))
        .and_then(|number| number.as_str().parse().ok())
}

fn infer_series_folder_context(path: &Path) -> Option<SeriesFolderContext> {
    let season_re = Regex::new(
        r"(?i)^(?P<series>.*?)(?:[\s._-]*(?:temporada|season)\s*0?(?P<s>\d{1,2})|[\s._-]+S\s*0?(?P<s2>\d{1,2}))(?:\b|[\s._-])",
    )
    .unwrap();

    for folder in path.ancestors().skip(1).take(3) {
        let folder_name = folder.file_name().and_then(|value| value.to_str())?;
        let normalized = folder_name.replace(['.', '_'], " ");
        let Some(caps) = season_re.captures(&normalized) else {
            continue;
        };
        let season_number = caps
            .name("s")
            .or_else(|| caps.name("s2"))
            .and_then(|value| value.as_str().parse().ok())?;
        let embedded_series = caps
            .name("series")
            .map(|value| clean_context_title(value.as_str()))
            .filter(|value| !value.starts_with("Sin t"));
        let series_title = embedded_series.or_else(|| {
            folder
                .parent()
                .and_then(Path::file_name)
                .and_then(|value| value.to_str())
                .filter(|value| !is_series_container_name(value))
                .map(clean_context_title)
                .filter(|value| !value.starts_with("Sin t"))
        })?;
        return Some(SeriesFolderContext {
            series_title,
            season_number,
        });
    }
    None
}

fn extract_forced_episode_number(value: &str) -> Option<(i32, usize, usize)> {
    let labelled = Regex::new(
        r"(?i)\b(?:episodio|episode|ep|e|cap(?:itulo|.tulo)?|chapter)\s*0?(?P<e>\d{1,3})\b",
    )
    .unwrap();
    if let Some(caps) = labelled.captures(value) {
        let marker = caps.get(0)?;
        return Some((
            caps.name("e")?.as_str().parse().ok()?,
            marker.start(),
            marker.end(),
        ));
    }
    if let Some((episode, marker_end)) = extract_episode_number(value) {
        return Some((episode, 0, marker_end));
    }
    let standalone = Regex::new(r"\b0?(?P<e>\d{1,3})\b").unwrap();
    let caps = standalone.captures_iter(value).last()?;
    let marker = caps.get(0)?;
    Some((
        caps.name("e")?.as_str().parse().ok()?,
        marker.start(),
        marker.end(),
    ))
}

fn derive_forced_series_title(value: &str) -> Option<String> {
    let title = clean_context_title(value);
    (!title.starts_with("Sin t")).then_some(title)
}

fn extract_episode_number(value: &str) -> Option<(i32, usize)> {
    let common_labelled = Regex::new(
        r"(?i)^\s*(?:episodio|episode|ep|e|cap(?:itulo|.tulo)?|chapter)\s*0?(?P<e>\d{1,3})(?:\b|[\s._-])",
    )
    .unwrap();
    if let Some(caps) = common_labelled.captures(value) {
        return Some((caps.name("e")?.as_str().parse().ok()?, caps.get(0)?.end()));
    }
    let leading = Regex::new(r"^\s*0?(?P<e>\d{1,3})(?:\b|[\s._-])").unwrap();
    let caps = leading.captures(value)?;
    Some((caps.name("e")?.as_str().parse().ok()?, caps.get(0)?.end()))
}

fn episode_title(tail: &str, episode: Option<i32>) -> String {
    let year = extract_year(tail);
    let before_year = year
        .and_then(|year| tail.split(&year.to_string()).next())
        .unwrap_or(tail);
    let title = clean_title(before_year);
    if title.starts_with("Sin t") {
        return format!("Episodio {}", episode.unwrap_or_default());
    }
    title
}

fn clean_context_title(value: &str) -> String {
    let year = extract_year(value);
    let before_year = year
        .and_then(|year| value.split(&year.to_string()).next())
        .unwrap_or(value);
    clean_title(before_year)
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

    #[test]
    fn folder_context_overrides_a_conflicting_episode_filename() {
        let parsed = parse_media_name(Path::new(
            r"D:\media\La.Casa.Del.Dragon.temporada3.2026.1080P-Dual-Lat\House.Of.The.Dragon.S03E04.2026.mkv",
        ));

        assert_eq!(parsed.kind, MediaKind::Episode);
        assert_eq!(parsed.series_title.as_deref(), Some("La Casa Del Dragon"));
        assert_eq!(parsed.season_number, Some(3));
        assert_eq!(parsed.episode_number, Some(4));
        assert!(parsed.needs_review);
    }

    #[test]
    fn infers_number_and_title_inside_a_season_folder() {
        let parsed = parse_media_name(Path::new(
            r"D:\media\The Show\Temporada 2\03 - El regreso.mkv",
        ));

        assert_eq!(parsed.kind, MediaKind::Episode);
        assert_eq!(parsed.series_title.as_deref(), Some("The Show"));
        assert_eq!(parsed.season_number, Some(2));
        assert_eq!(parsed.episode_number, Some(3));
        assert_eq!(parsed.title, "El regreso");
        assert!(!parsed.needs_review);
    }

    #[test]
    fn flags_a_season_file_without_an_episode_number() {
        let parsed = parse_media_name(Path::new(
            r"D:\media\The Show\Season 2\archivo sin numero.mkv",
        ));

        assert_eq!(parsed.kind, MediaKind::Movie);
        assert!(parsed.needs_review);
        assert!(parsed.review_reason.is_some());
    }

    #[test]
    fn series_container_forces_a_named_file_to_be_an_episode() {
        let parsed = parse_media_name(Path::new(
            r"D:\media\Serie\Mi Anime\Temporada 1\Mi Anime 1.mkv",
        ));

        assert_eq!(parsed.kind, MediaKind::Episode);
        assert_eq!(parsed.series_title.as_deref(), Some("Mi Anime"));
        assert_eq!(parsed.season_number, Some(1));
        assert_eq!(parsed.episode_number, Some(1));
        assert_eq!(parsed.title, "Episodio 1");
        assert!(!parsed.needs_review);
    }

    #[test]
    fn series_container_accepts_plural_name_and_embedded_episode_number() {
        let parsed = parse_media_name(Path::new(
            r"D:\media\SERIES\Frieren\Season 2\Frieren aventura 12 final.mkv",
        ));

        assert_eq!(parsed.kind, MediaKind::Episode);
        assert_eq!(parsed.series_title.as_deref(), Some("Frieren"));
        assert_eq!(parsed.season_number, Some(2));
        assert_eq!(parsed.episode_number, Some(12));
        assert_eq!(parsed.title, "final");
        assert!(!parsed.needs_review);
    }

    #[test]
    fn series_container_derives_title_when_the_season_is_directly_inside_it() {
        let parsed = parse_media_name(Path::new(r"D:\media\Serie\Temporada 3\Dungeon Meshi 4.mkv"));

        assert_eq!(parsed.kind, MediaKind::Episode);
        assert_eq!(parsed.series_title.as_deref(), Some("Dungeon Meshi"));
        assert_eq!(parsed.season_number, Some(3));
        assert_eq!(parsed.episode_number, Some(4));
        assert!(!parsed.needs_review);
    }

    #[test]
    fn series_container_never_falls_back_to_movie_when_episode_is_unclear() {
        let parsed = parse_media_name(Path::new(r"D:\media\Serie\Mi Anime\archivo sin numero.mkv"));

        assert_eq!(parsed.kind, MediaKind::Episode);
        assert_eq!(parsed.series_title.as_deref(), Some("Mi Anime"));
        assert_eq!(parsed.season_number, Some(1));
        assert_eq!(parsed.episode_number, None);
        assert!(parsed.needs_review);
    }

    #[test]
    fn numbered_file_outside_series_container_remains_a_movie() {
        let parsed = parse_media_name(Path::new(r"D:\media\Mi Anime 1.mkv"));

        assert_eq!(parsed.kind, MediaKind::Movie);
    }

    #[test]
    fn uppercase_accented_movies_container_forces_movie_classification() {
        let parsed = parse_media_name(Path::new(
            r"D:\media\PELÍCULAS\Una Pelicula S01E01 2025.mkv",
        ));

        assert_eq!(parsed.kind, MediaKind::Movie);
        assert_eq!(parsed.title, "Una Pelicula S01E01");
        assert_eq!(parsed.year, Some(2025));
        assert_eq!(parsed.series_title, None);
        assert_eq!(parsed.season_number, None);
        assert_eq!(parsed.episode_number, None);
        assert_eq!(parsed.identification_source, "movie_container");
        assert!(!parsed.needs_review);
    }

    #[test]
    fn movies_container_also_accepts_name_without_accent() {
        let parsed = parse_media_name(Path::new(r"D:\media\PELICULAS\Alien.2.El.Regreso.1986.mkv"));

        assert_eq!(parsed.kind, MediaKind::Movie);
        assert_eq!(parsed.title, "Alien 2 El Regreso");
        assert_eq!(parsed.year, Some(1986));
    }
}
