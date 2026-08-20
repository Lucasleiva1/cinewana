use super::MetadataSearchOutcome;
use anyhow::{Context, Result};
use cinewana_core::{ImportedMediaMetadata, MediaKind, MediaMetadataCandidate};
use reqwest::{Client, StatusCode};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const API_BASE: &str = "https://api.themoviedb.org/3";
const IMAGE_BASE: &str = "https://image.tmdb.org/t/p";
const SOURCE_BASE: &str = "https://www.themoviedb.org";
const PRIMARY_LANGUAGE: &str = "es-AR";
const FALLBACK_LANGUAGE: &str = "en-US";

#[derive(Clone)]
enum Credential {
    ReadAccessToken(String),
    ApiKey(String),
    Missing,
}

#[derive(Debug, Clone, Default)]
pub struct CachedArtwork {
    pub poster_path: Option<PathBuf>,
    pub backdrop_path: Option<PathBuf>,
}

pub struct TmdbMetadataClient {
    client: Client,
    credential: Credential,
}

impl TmdbMetadataClient {
    pub fn from_environment() -> Result<Self> {
        let read_token = std::env::var("TMDB_READ_ACCESS_TOKEN")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| option_env!("TMDB_READ_ACCESS_TOKEN").map(str::to_owned));
        let api_key = std::env::var("TMDB_API_KEY")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .or_else(|| option_env!("TMDB_API_KEY").map(str::to_owned));
        let credential = read_token
            .map(|value| Credential::ReadAccessToken(value.trim().to_owned()))
            .or_else(|| api_key.map(|value| Credential::ApiKey(value.trim().to_owned())))
            .unwrap_or(Credential::Missing);
        let client = Client::builder()
            .user_agent("CINE-WANA/0.3.7 (Windows desktop metadata importer)")
            .connect_timeout(std::time::Duration::from_secs(10))
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .context("create TMDB HTTP client")?;
        Ok(Self { client, credential })
    }

    pub fn configured(&self) -> bool {
        !matches!(self.credential, Credential::Missing)
    }

    pub async fn search_media(
        &self,
        title: &str,
        year: Option<i32>,
        kind: &MediaKind,
        season_number: Option<i32>,
        episode_number: Option<i32>,
    ) -> Result<MetadataSearchOutcome> {
        self.require_credential()?;
        let media_type = match kind {
            MediaKind::Movie => "movie",
            MediaKind::Episode => "tv",
        };
        let mut query = vec![
            ("query", title.to_owned()),
            ("language", PRIMARY_LANGUAGE.to_owned()),
            ("include_adult", "false".to_owned()),
            ("page", "1".to_owned()),
        ];
        if let Some(year) = year {
            query.push((
                if media_type == "movie" {
                    "primary_release_year"
                } else {
                    "first_air_date_year"
                },
                year.to_string(),
            ));
        }
        let response: SearchResponse = self
            .get_json(&format!("/search/{media_type}"), &query)
            .await?;
        let mut scored = score_search_results(
            title,
            year,
            media_type,
            season_number,
            episode_number,
            response.results,
        );
        scored.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.title.cmp(&right.1.title))
        });
        let Some((best_score, best_candidate)) = scored.first().cloned() else {
            return Ok(MetadataSearchOutcome::NotFound);
        };
        let runner_up = scored.get(1).map(|entry| entry.0).unwrap_or(0);
        let confident = best_score >= 92
            || (best_score >= 76 && best_score - runner_up >= 10)
            || (best_score >= 76 && scored.len() == 1);
        if confident {
            if let Some(metadata) = self.import_candidate(&best_candidate).await? {
                return Ok(MetadataSearchOutcome::Imported(metadata));
            }
        }
        Ok(MetadataSearchOutcome::Ambiguous(
            scored
                .into_iter()
                .take(6)
                .map(|(_, candidate)| candidate)
                .collect(),
        ))
    }

    pub async fn import_candidate(
        &self,
        candidate: &MediaMetadataCandidate,
    ) -> Result<Option<ImportedMediaMetadata>> {
        self.require_credential()?;
        let Some(target) = parse_candidate_id(&candidate.id) else {
            anyhow::bail!(
                "La coincidencia guardada pertenece al proveedor anterior; volvé a buscar con TMDB"
            );
        };
        match target.media_type.as_str() {
            "movie" => self.import_movie(target.tmdb_id).await.map(Some),
            "tv" => self
                .import_tv(target.tmdb_id, target.season_number, target.episode_number)
                .await
                .map(Some),
            _ => Ok(None),
        }
    }

    pub async fn poster_options(
        &self,
        candidates: &[MediaMetadataCandidate],
    ) -> Result<Vec<MediaMetadataCandidate>> {
        self.require_credential()?;
        let mut options = Vec::new();
        let mut targets = HashSet::new();
        for candidate in candidates {
            let Some(target) = parse_candidate_id(&candidate.id) else {
                continue;
            };
            let target_key = format!("{}:{}", target.media_type, target.tmdb_id);
            if !targets.insert(target_key) || targets.len() > 4 {
                continue;
            }
            let endpoint = format!("/{}/{}/images", target.media_type, target.tmdb_id);
            let query = vec![("include_image_language", "es,null,en".to_owned())];
            let response = self.get_json::<ImageResponse>(&endpoint, &query).await;
            let mut poster_paths = Vec::new();
            if let Some(primary) = candidate.poster_url.as_deref() {
                poster_paths.push(primary.to_owned());
            }
            if let Ok(response) = response {
                for image in response.posters.into_iter().take(8) {
                    let url = image_url("w500", Some(&image.file_path));
                    if let Some(url) = url
                        && !poster_paths
                            .iter()
                            .any(|existing| existing.rsplit('/').next() == url.rsplit('/').next())
                    {
                        poster_paths.push(url);
                    }
                    if poster_paths.len() >= 6 {
                        break;
                    }
                }
            }
            for (index, poster_url) in poster_paths.into_iter().enumerate() {
                let mut option = candidate.clone();
                option.id = format!("{}:poster:{index}", candidate.id);
                option.poster_url = Some(poster_url);
                options.push(option);
            }
        }
        Ok(options)
    }

    pub async fn cache_artwork(
        &self,
        cache_root: &Path,
        fingerprint: &str,
        metadata: &ImportedMediaMetadata,
    ) -> Result<CachedArtwork> {
        let poster_key = artwork_cache_key(metadata.poster_url.as_deref(), fingerprint);
        let backdrop_key = artwork_cache_key(metadata.backdrop_url.as_deref(), fingerprint);
        let poster_directory = cache_root.join("tmdb").join("posters");
        let backdrop_directory = cache_root.join("tmdb").join("backdrops");
        tokio::fs::create_dir_all(&poster_directory)
            .await
            .context("create persistent TMDB poster cache")?;
        tokio::fs::create_dir_all(&backdrop_directory)
            .await
            .context("create persistent TMDB backdrop cache")?;
        let poster_path = poster_directory.join(format!("{poster_key}.jpg"));
        let backdrop_path = backdrop_directory.join(format!("{backdrop_key}.jpg"));
        let (poster_result, backdrop_result) = tokio::join!(
            self.download_image(metadata.poster_url.as_deref(), &poster_path),
            self.download_image(metadata.backdrop_url.as_deref(), &backdrop_path),
        );
        Ok(CachedArtwork {
            poster_path: poster_result?,
            backdrop_path: backdrop_result?,
        })
    }

    async fn import_movie(&self, tmdb_id: i64) -> Result<ImportedMediaMetadata> {
        let primary = self.movie_details(tmdb_id, PRIMARY_LANGUAGE).await?;
        let fallback = if primary.overview.as_deref().is_none_or(str::is_empty) {
            Some(self.movie_details(tmdb_id, FALLBACK_LANGUAGE).await?)
        } else {
            None
        };
        let overview = non_empty(primary.overview).or_else(|| {
            fallback
                .as_ref()
                .and_then(|detail| non_empty(detail.overview.clone()))
        });
        Ok(ImportedMediaMetadata {
            provider: "tmdb".to_owned(),
            title: primary.title.unwrap_or_else(|| "Sin título".to_owned()),
            year: year_from_date(primary.release_date.as_deref()),
            overview,
            genres: primary.genres.into_iter().map(|genre| genre.name).collect(),
            cast: cast_names(primary.credits),
            source_url: format!("{SOURCE_BASE}/movie/{tmdb_id}"),
            source_language: PRIMARY_LANGUAGE.to_owned(),
            poster_url: image_url("w500", primary.poster_path.as_deref()),
            backdrop_url: image_url("w1280", primary.backdrop_path.as_deref()),
        })
    }

    async fn import_tv(
        &self,
        tmdb_id: i64,
        season_number: Option<i32>,
        episode_number: Option<i32>,
    ) -> Result<ImportedMediaMetadata> {
        let series = self.tv_details(tmdb_id, PRIMARY_LANGUAGE).await?;
        let mut title = series
            .name
            .clone()
            .unwrap_or_else(|| "Sin título".to_owned());
        let mut year = year_from_date(series.first_air_date.as_deref());
        let mut overview = non_empty(series.overview.clone());
        let mut cast = cast_names(series.aggregate_credits.clone());
        let mut backdrop_path = series.backdrop_path.clone();
        let mut source_url = format!("{SOURCE_BASE}/tv/{tmdb_id}");
        if let (Some(season), Some(episode)) = (season_number, episode_number) {
            let details = self
                .episode_details(tmdb_id, season, episode, PRIMARY_LANGUAGE)
                .await?;
            if let Some(episode_title) = non_empty(details.name) {
                title = episode_title;
            }
            year = year_from_date(details.air_date.as_deref()).or(year);
            overview = non_empty(details.overview).or(overview);
            let episode_cast = cast_names(details.credits);
            if !episode_cast.is_empty() {
                cast = episode_cast;
            }
            if details.still_path.is_some() {
                backdrop_path = details.still_path;
            }
            source_url = format!("{SOURCE_BASE}/tv/{tmdb_id}/season/{season}/episode/{episode}");
        }
        if overview.is_none() {
            let fallback = self.tv_details(tmdb_id, FALLBACK_LANGUAGE).await?;
            overview = non_empty(fallback.overview);
        }
        Ok(ImportedMediaMetadata {
            provider: "tmdb".to_owned(),
            title,
            year,
            overview,
            genres: series.genres.into_iter().map(|genre| genre.name).collect(),
            cast,
            source_url,
            source_language: PRIMARY_LANGUAGE.to_owned(),
            poster_url: image_url("w500", series.poster_path.as_deref()),
            backdrop_url: image_url("w1280", backdrop_path.as_deref()),
        })
    }

    async fn movie_details(&self, tmdb_id: i64, language: &str) -> Result<MediaDetails> {
        self.get_json(
            &format!("/movie/{tmdb_id}"),
            &[
                ("language", language.to_owned()),
                ("append_to_response", "credits".to_owned()),
            ],
        )
        .await
    }

    async fn tv_details(&self, tmdb_id: i64, language: &str) -> Result<MediaDetails> {
        self.get_json(
            &format!("/tv/{tmdb_id}"),
            &[
                ("language", language.to_owned()),
                ("append_to_response", "aggregate_credits".to_owned()),
            ],
        )
        .await
    }

    async fn episode_details(
        &self,
        tmdb_id: i64,
        season: i32,
        episode: i32,
        language: &str,
    ) -> Result<EpisodeDetails> {
        self.get_json(
            &format!("/tv/{tmdb_id}/season/{season}/episode/{episode}"),
            &[
                ("language", language.to_owned()),
                ("append_to_response", "credits".to_owned()),
            ],
        )
        .await
    }

    async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
        query: &[(impl AsRef<str>, String)],
    ) -> Result<T> {
        let url = format!("{API_BASE}{path}");
        let normalized_query = query
            .iter()
            .map(|(key, value)| (key.as_ref(), value.as_str()))
            .collect::<Vec<_>>();
        let mut request = self.client.get(url).query(&normalized_query);
        request = match &self.credential {
            Credential::ReadAccessToken(token) => request.bearer_auth(token),
            Credential::ApiKey(key) => request.query(&[("api_key", key)]),
            Credential::Missing => {
                self.require_credential()?;
                unreachable!()
            }
        };
        let response = request.send().await.context("consult TMDB")?;
        let status = response.status();
        if status != StatusCode::OK {
            anyhow::bail!("TMDB respondió con estado {status}");
        }
        response.json().await.context("parse TMDB response")
    }

    async fn download_image(&self, source: Option<&str>, target: &Path) -> Result<Option<PathBuf>> {
        let Some(source) = source else {
            return Ok(None);
        };
        if tokio::fs::metadata(target)
            .await
            .is_ok_and(|metadata| metadata.len() > 4_000)
        {
            return Ok(Some(target.to_path_buf()));
        }
        let response = self
            .client
            .get(source)
            .send()
            .await
            .context("download TMDB artwork")?;
        if !response.status().is_success() {
            anyhow::bail!("TMDB image server responded with {}", response.status());
        }
        let bytes = response.bytes().await.context("read TMDB artwork")?;
        if bytes.len() < 4_000 {
            anyhow::bail!("TMDB returned an invalid artwork file");
        }
        tokio::fs::write(target, bytes)
            .await
            .context("write persistent TMDB artwork")?;
        Ok(Some(target.to_path_buf()))
    }

    fn require_credential(&self) -> Result<()> {
        if self.configured() {
            Ok(())
        } else {
            anyhow::bail!(
                "TMDB no está configurado. Definí TMDB_READ_ACCESS_TOKEN o TMDB_API_KEY en el entorno de CINE WANA"
            )
        }
    }
}

fn score_search_results(
    requested_title: &str,
    requested_year: Option<i32>,
    media_type: &str,
    season_number: Option<i32>,
    episode_number: Option<i32>,
    results: Vec<SearchResult>,
) -> Vec<(i32, MediaMetadataCandidate)> {
    results
        .into_iter()
        .filter_map(|result| {
            let candidate_title = result.display_title()?.to_owned();
            let candidate_year = result.year();
            let score = score_result(
                requested_title,
                requested_year,
                &candidate_title,
                candidate_year,
            );
            // TMDB text search already checks translated and alternative titles. A localized
            // alias can therefore legitimately look nothing like the displayed canonical title
            // (for example "La Hermandad" -> "Daybreakers"). Keep low-scoring API matches for
            // explicit review, while the confidence threshold above still prevents auto-import.
            Some((
                score,
                MediaMetadataCandidate {
                    id: candidate_context_id(media_type, result.id, season_number, episode_number),
                    language: PRIMARY_LANGUAGE.to_owned(),
                    page_id: result.id,
                    title: candidate_title,
                    year: candidate_year,
                    description: non_empty(result.overview),
                    source_url: tmdb_source_url(
                        media_type,
                        result.id,
                        season_number,
                        episode_number,
                    ),
                    poster_url: image_url("w342", result.poster_path.as_deref()),
                },
            ))
        })
        .collect()
}

#[derive(Debug)]
struct CandidateTarget {
    media_type: String,
    tmdb_id: i64,
    season_number: Option<i32>,
    episode_number: Option<i32>,
}

fn candidate_context_id(
    media_type: &str,
    tmdb_id: i64,
    season_number: Option<i32>,
    episode_number: Option<i32>,
) -> String {
    match (media_type, season_number, episode_number) {
        ("tv", Some(season), Some(episode)) => {
            format!("tmdb:tv:{tmdb_id}:{season}:{episode}")
        }
        _ => format!("tmdb:{media_type}:{tmdb_id}"),
    }
}

fn parse_candidate_id(value: &str) -> Option<CandidateTarget> {
    let parts = value.split(':').collect::<Vec<_>>();
    if parts.first().copied()? != "tmdb" {
        return None;
    }
    let media_type = parts.get(1)?.to_string();
    let tmdb_id = parts.get(2)?.parse().ok()?;
    let season_number = parts.get(3).and_then(|value| value.parse().ok());
    let episode_number = parts.get(4).and_then(|value| value.parse().ok());
    Some(CandidateTarget {
        media_type,
        tmdb_id,
        season_number,
        episode_number,
    })
}

fn tmdb_source_url(
    media_type: &str,
    tmdb_id: i64,
    season_number: Option<i32>,
    episode_number: Option<i32>,
) -> String {
    match (media_type, season_number, episode_number) {
        ("tv", Some(season), Some(episode)) => {
            format!("{SOURCE_BASE}/tv/{tmdb_id}/season/{season}/episode/{episode}")
        }
        _ => format!("{SOURCE_BASE}/{media_type}/{tmdb_id}"),
    }
}

fn score_result(
    requested_title: &str,
    requested_year: Option<i32>,
    candidate_title: &str,
    candidate_year: Option<i32>,
) -> i32 {
    let requested = normalize_title(requested_title);
    let candidate = normalize_title(candidate_title);
    let mut score = if requested == candidate {
        76
    } else if requested.contains(&candidate) || candidate.contains(&requested) {
        54
    } else {
        let requested_tokens = requested.split_whitespace().collect::<HashSet<_>>();
        let candidate_tokens = candidate.split_whitespace().collect::<HashSet<_>>();
        let shared = requested_tokens.intersection(&candidate_tokens).count() as i32;
        let total = requested_tokens.len().max(candidate_tokens.len()).max(1) as i32;
        12 + shared * 52 / total
    };
    if let (Some(requested_year), Some(candidate_year)) = (requested_year, candidate_year) {
        score += match (requested_year - candidate_year).abs() {
            0 => 24,
            1 => 10,
            _ => -14,
        };
    }
    score
}

fn normalize_title(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            other if other.is_alphanumeric() || other.is_whitespace() => other,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn image_url(size: &str, path: Option<&str>) -> Option<String> {
    path.filter(|value| !value.trim().is_empty())
        .map(|value| format!("{IMAGE_BASE}/{size}{value}"))
}

fn artwork_cache_key(source_url: Option<&str>, fingerprint: &str) -> String {
    let source_key = source_url
        .and_then(|url| url.rsplit('/').next())
        .and_then(|file_name| file_name.split('.').next());
    let key = source_key
        .unwrap_or(fingerprint)
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .take(64)
        .collect::<String>();
    if key.is_empty() {
        "media".to_owned()
    } else {
        key
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}

fn year_from_date(value: Option<&str>) -> Option<i32> {
    value
        .and_then(|date| date.get(0..4))
        .and_then(|year| year.parse().ok())
}

fn cast_names(credits: Option<TmdbCredits>) -> Vec<String> {
    let mut cast = credits.map(|credits| credits.cast).unwrap_or_default();
    cast.sort_by_key(|person| person.order.unwrap_or(i32::MAX));
    let mut names = Vec::new();
    for person in cast {
        let name = person.name.trim();
        if name.is_empty() || names.iter().any(|existing: &String| existing == name) {
            continue;
        }
        names.push(name.to_owned());
        if names.len() >= 32 {
            break;
        }
    }
    names
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    #[serde(default)]
    results: Vec<SearchResult>,
}

#[derive(Debug, Deserialize)]
struct ImageResponse {
    #[serde(default)]
    posters: Vec<TmdbImage>,
}

#[derive(Debug, Deserialize)]
struct TmdbImage {
    file_path: String,
}

#[derive(Debug, Deserialize)]
struct SearchResult {
    id: i64,
    title: Option<String>,
    name: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    overview: Option<String>,
    poster_path: Option<String>,
}

impl SearchResult {
    fn display_title(&self) -> Option<&str> {
        self.title.as_deref().or(self.name.as_deref())
    }

    fn year(&self) -> Option<i32> {
        year_from_date(
            self.release_date
                .as_deref()
                .or(self.first_air_date.as_deref()),
        )
    }
}

#[derive(Debug, Deserialize)]
struct MediaDetails {
    title: Option<String>,
    name: Option<String>,
    overview: Option<String>,
    release_date: Option<String>,
    first_air_date: Option<String>,
    #[serde(default)]
    genres: Vec<TmdbGenre>,
    poster_path: Option<String>,
    backdrop_path: Option<String>,
    credits: Option<TmdbCredits>,
    aggregate_credits: Option<TmdbCredits>,
}

#[derive(Debug, Deserialize)]
struct EpisodeDetails {
    name: Option<String>,
    overview: Option<String>,
    air_date: Option<String>,
    still_path: Option<String>,
    credits: Option<TmdbCredits>,
}

#[derive(Debug, Deserialize)]
struct TmdbGenre {
    name: String,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbCredits {
    #[serde(default)]
    cast: Vec<TmdbCastMember>,
}

#[derive(Debug, Clone, Deserialize)]
struct TmdbCastMember {
    name: String,
    order: Option<i32>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_title_and_year_is_confident() {
        assert_eq!(score_result("Alien", Some(1979), "Alien", Some(1979)), 100);
    }

    #[test]
    fn wrong_remake_year_is_penalized() {
        assert!(
            score_result("The Thing", Some(1982), "The Thing", Some(1982))
                > score_result("The Thing", Some(1982), "The Thing", Some(2011))
        );
    }

    #[test]
    fn keeps_a_completely_different_alternative_title_for_manual_review() {
        let candidates = score_search_results(
            "La Hermandad",
            Some(2010),
            "movie",
            None,
            None,
            vec![SearchResult {
                id: 19_901,
                title: Some("Daybreakers".into()),
                name: None,
                release_date: Some("2010-01-06".into()),
                first_air_date: None,
                overview: Some("Un mundo dominado por vampiros.".into()),
                poster_path: Some("/daybreakers.jpg".into()),
            }],
        );

        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].1.title, "Daybreakers");
        assert!(
            candidates[0].0 < 76,
            "an alternative title must require review"
        );
    }

    #[test]
    fn candidate_id_preserves_episode_context() {
        let parsed = parse_candidate_id("tmdb:tv:1399:2:4").expect("valid candidate");
        assert_eq!(parsed.media_type, "tv");
        assert_eq!(parsed.tmdb_id, 1399);
        assert_eq!(parsed.season_number, Some(2));
        assert_eq!(parsed.episode_number, Some(4));
    }

    #[test]
    fn builds_expected_image_url() {
        assert_eq!(
            image_url("w500", Some("/poster.jpg")).as_deref(),
            Some("https://image.tmdb.org/t/p/w500/poster.jpg")
        );
    }

    #[test]
    fn shares_the_same_official_poster_between_series_episodes() {
        assert_eq!(
            artwork_cache_key(
                Some("https://image.tmdb.org/t/p/w500/shared-poster.jpg"),
                "episode-one"
            ),
            artwork_cache_key(
                Some("https://image.tmdb.org/t/p/w500/shared-poster.jpg"),
                "episode-two"
            )
        );
    }
}
