use anyhow::{Context, Result};
use chrono::Utc;
use cinewana_core::{ImportedMediaMetadata, MediaMetadataCandidate};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::process::Command;

pub const WIKIPEDIA_USER_AGENT: &str =
    "CINE-WANA/0.1.0 (local desktop metadata importer; https://github.com/Lucasleiva1/cinewana)";

#[derive(Debug, Clone)]
pub enum MetadataSearchOutcome {
    Imported(ImportedMediaMetadata),
    Ambiguous(Vec<MediaMetadataCandidate>),
    NotFound,
}

pub struct WikipediaMetadataClient {
    curl: String,
}

impl WikipediaMetadataClient {
    pub fn new() -> Result<Self> {
        Ok(Self {
            curl: if cfg!(windows) {
                "curl.exe".into()
            } else {
                "curl".into()
            },
        })
    }

    pub async fn search_movie(
        &self,
        title: &str,
        year: Option<i32>,
    ) -> Result<MetadataSearchOutcome> {
        let spanish = self.search_language("es", title, year).await?;
        if !spanish.is_empty() {
            return Ok(select_candidate(spanish));
        }
        let english = self.search_language("en", title, year).await?;
        if english.is_empty() {
            return Ok(MetadataSearchOutcome::NotFound);
        }
        Ok(select_candidate(english))
    }

    pub async fn import_candidate(
        &self,
        candidate: &MediaMetadataCandidate,
    ) -> Result<Option<ImportedMediaMetadata>> {
        let pages = self
            .fetch_pages(&candidate.language, &[candidate.page_id])
            .await?;
        Ok(pages
            .into_iter()
            .next()
            .and_then(|page| page_to_imported(&candidate.language, &page, Some(candidate))))
    }

    async fn search_language(
        &self,
        language: &str,
        title: &str,
        year: Option<i32>,
    ) -> Result<Vec<ScoredCandidate>> {
        let label = if language == "es" {
            "película"
        } else {
            "film"
        };
        let search = match year {
            Some(year) => format!("{title} {year} {label}"),
            None => format!("{title} {label}"),
        };
        let url = api_url(
            language,
            &[
                ("action", "query"),
                ("list", "search"),
                ("srsearch", search.as_str()),
                ("srlimit", "6"),
                ("srnamespace", "0"),
                ("format", "json"),
                ("formatversion", "2"),
                ("utf8", "1"),
            ],
        )?;
        let response = self.fetch_json::<SearchResponse>(&url).await?;
        let hits = response.query.map(|query| query.search).unwrap_or_default();
        let page_ids = hits.iter().map(|hit| hit.pageid).collect::<Vec<_>>();
        if page_ids.is_empty() {
            return Ok(vec![]);
        }
        let pages = self.fetch_pages(language, &page_ids).await?;
        let mut scored = pages
            .into_iter()
            .filter_map(|page| {
                let hit = hits.iter().find(|hit| hit.pageid == page.pageid);
                let score = score_page(title, year, language, &page, hit);
                if score < 35 {
                    return None;
                }
                let candidate = page_to_candidate(language, &page, hit, year)?;
                Some(ScoredCandidate {
                    score,
                    imported: page_to_imported(language, &page, Some(&candidate)),
                    candidate,
                })
            })
            .collect::<Vec<_>>();
        scored.sort_by(|a, b| {
            b.score
                .cmp(&a.score)
                .then_with(|| a.candidate.title.cmp(&b.candidate.title))
        });
        Ok(scored)
    }

    async fn fetch_pages(&self, language: &str, page_ids: &[i64]) -> Result<Vec<WikiPage>> {
        let ids = page_ids
            .iter()
            .map(i64::to_string)
            .collect::<Vec<_>>()
            .join("|");
        let url = api_url(
            language,
            &[
                ("action", "query"),
                ("pageids", ids.as_str()),
                ("prop", "extracts|revisions"),
                ("exintro", "1"),
                ("explaintext", "1"),
                ("rvprop", "content"),
                ("rvslots", "main"),
                ("format", "json"),
                ("formatversion", "2"),
                ("utf8", "1"),
            ],
        )?;
        let response = self.fetch_json::<PageResponse>(&url).await?;
        Ok(response.query.map(|query| query.pages).unwrap_or_default())
    }

    async fn fetch_json<T: for<'de> Deserialize<'de>>(&self, url: &str) -> Result<T> {
        let output = Command::new(&self.curl)
            .args([
                "-L",
                "--fail",
                "--silent",
                "--show-error",
                "--max-time",
                "20",
                "-A",
                WIKIPEDIA_USER_AGENT,
                url,
            ])
            .output()
            .await
            .context("launch curl for Wikipedia metadata")?;
        if !output.status.success() {
            anyhow::bail!(
                "Wikipedia request failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        serde_json::from_slice(&output.stdout).context("parse Wikipedia JSON")
    }
}

pub fn write_metadata_json(
    cache_root: &Path,
    fingerprint: &str,
    metadata: &ImportedMediaMetadata,
) -> Result<PathBuf> {
    let directory = cache_root.join("metadata");
    std::fs::create_dir_all(&directory).context("create metadata cache directory")?;
    let key = fingerprint.chars().take(48).collect::<String>();
    let path = directory.join(format!("{key}.metadata.json"));
    let sidecar = MetadataJson {
        provider: "wikipedia",
        imported_at: Utc::now().to_rfc3339(),
        metadata,
    };
    std::fs::write(&path, serde_json::to_vec_pretty(&sidecar)?)
        .context("write metadata JSON cache")?;
    Ok(path)
}

#[derive(Debug, Serialize)]
struct MetadataJson<'a> {
    provider: &'static str,
    imported_at: String,
    metadata: &'a ImportedMediaMetadata,
}

#[derive(Debug, Clone)]
struct ScoredCandidate {
    score: i32,
    candidate: MediaMetadataCandidate,
    imported: Option<ImportedMediaMetadata>,
}

fn select_candidate(candidates: Vec<ScoredCandidate>) -> MetadataSearchOutcome {
    let Some(best) = candidates.first() else {
        return MetadataSearchOutcome::NotFound;
    };
    let runner_up = candidates
        .get(1)
        .map(|candidate| candidate.score)
        .unwrap_or(0);
    let confident = (best.score >= 78 && best.score - runner_up >= 12)
        || (best.score >= 62 && candidates.len() == 1);
    if confident {
        if let Some(imported) = best.imported.clone() {
            return MetadataSearchOutcome::Imported(imported);
        }
    }
    let visible = candidates
        .into_iter()
        .take(5)
        .map(|candidate| candidate.candidate)
        .collect::<Vec<_>>();
    if visible.is_empty() {
        MetadataSearchOutcome::NotFound
    } else {
        MetadataSearchOutcome::Ambiguous(visible)
    }
}

fn api_url(language: &str, params: &[(&str, &str)]) -> Result<String> {
    let query = params
        .iter()
        .map(|(key, value)| format!("{}={}", url_encode(key), url_encode(value)))
        .collect::<Vec<_>>()
        .join("&");
    Ok(format!(
        "https://{language}.wikipedia.org/w/api.php?{query}"
    ))
}

fn url_encode(value: &str) -> String {
    let mut output = String::new();
    for byte in value.as_bytes() {
        match *byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                output.push(*byte as char)
            }
            b' ' => output.push('+'),
            other => output.push_str(&format!("%{other:02X}")),
        }
    }
    output
}

fn page_to_candidate(
    language: &str,
    page: &WikiPage,
    hit: Option<&SearchHit>,
    requested_year: Option<i32>,
) -> Option<MediaMetadataCandidate> {
    let description = first_valid_paragraph(page.extract.as_deref().unwrap_or_default());
    let text = format!(
        "{}\n{}\n{}",
        page.title,
        page.extract.as_deref().unwrap_or_default(),
        hit.and_then(|hit| hit.snippet.as_deref())
            .unwrap_or_default()
    );
    let year = requested_year.or_else(|| extract_year(&text));
    Some(MediaMetadataCandidate {
        id: format!("{language}:{}", page.pageid),
        language: language.to_owned(),
        page_id: page.pageid,
        title: clean_page_title(&page.title),
        year,
        description,
        source_url: source_url(language, &page.title),
    })
}

fn page_to_imported(
    language: &str,
    page: &WikiPage,
    candidate: Option<&MediaMetadataCandidate>,
) -> Option<ImportedMediaMetadata> {
    let overview = first_valid_paragraph(page.extract.as_deref().unwrap_or_default());
    let wikitext = page.wikitext();
    let cast = extract_cast(wikitext.as_deref().unwrap_or_default());
    if overview.is_none() && cast.is_empty() {
        return None;
    }
    Some(ImportedMediaMetadata {
        title: candidate
            .map(|candidate| candidate.title.clone())
            .unwrap_or_else(|| clean_page_title(&page.title)),
        year: candidate.and_then(|candidate| candidate.year).or_else(|| {
            extract_year(&format!(
                "{}\n{}",
                page.title,
                page.extract.as_deref().unwrap_or_default()
            ))
        }),
        overview,
        cast,
        source_url: candidate
            .map(|candidate| candidate.source_url.clone())
            .unwrap_or_else(|| source_url(language, &page.title)),
        source_language: language.to_owned(),
    })
}

fn score_page(
    requested_title: &str,
    requested_year: Option<i32>,
    language: &str,
    page: &WikiPage,
    hit: Option<&SearchHit>,
) -> i32 {
    let normalized_request = normalize_text(requested_title);
    let normalized_title = normalize_text(&clean_page_title(&page.title));
    let body = normalize_text(&format!(
        "{} {} {}",
        page.title,
        page.extract.as_deref().unwrap_or_default(),
        hit.and_then(|hit| hit.snippet.as_deref())
            .unwrap_or_default()
    ));
    let mut score = 0;
    if normalized_title == normalized_request {
        score += 48;
    } else if normalized_title.contains(&normalized_request)
        || normalized_request.contains(&normalized_title)
    {
        score += 28;
    } else {
        for token in normalized_request
            .split_whitespace()
            .filter(|token| token.len() > 2)
        {
            if normalized_title.contains(token) {
                score += 6;
            }
        }
    }
    if let Some(year) = requested_year {
        let year = year.to_string();
        if page.title.contains(&year) {
            score += 24;
        } else if body.contains(&year) {
            score += 16;
        } else {
            score -= 10;
        }
    }
    let film_terms = if language == "es" {
        [
            "pelicula",
            "filme",
            "largometraje",
            "dirigida",
            "protagonizada",
        ]
    } else {
        ["film", "movie", "directed", "starring", "feature"]
    };
    if film_terms.iter().any(|term| body.contains(term)) {
        score += 18;
    }
    if body.contains("desambiguacion") || body.contains("disambiguation") {
        score -= 50;
    }
    if first_valid_paragraph(page.extract.as_deref().unwrap_or_default()).is_some() {
        score += 8;
    }
    score
}

fn first_valid_paragraph(extract: &str) -> Option<String> {
    extract
        .split("\n\n")
        .map(|part| part.trim().replace('\n', " "))
        .map(|part| collapse_spaces(&part))
        .find(|part| {
            part.chars().count() >= 80
                && !normalize_text(part).contains("puede referirse")
                && !normalize_text(part).contains("may refer to")
        })
}

fn extract_cast(wikitext: &str) -> Vec<String> {
    let mut values = extract_infobox_cast(wikitext);
    if values.is_empty() {
        values = extract_section_cast(wikitext);
    }
    normalize_people(values, 24)
}

fn extract_infobox_cast(wikitext: &str) -> Vec<String> {
    let keys = ["protagonistas", "reparto", "starring", "elenco", "actores"];
    let mut values = Vec::new();
    let mut current_key = None::<String>;
    let mut current_value = String::new();
    for raw_line in wikitext.lines() {
        let line = raw_line.trim_end();
        if line.trim_start().starts_with('|') {
            flush_infobox_field(&keys, &current_key, &current_value, &mut values);
            let field = line.trim_start().trim_start_matches('|');
            if let Some((key, value)) = field.split_once('=') {
                current_key = Some(normalize_text(key));
                current_value = value.trim().to_owned();
            } else {
                current_key = None;
                current_value.clear();
            }
        } else if current_key.is_some() {
            current_value.push('\n');
            current_value.push_str(line);
        }
    }
    flush_infobox_field(&keys, &current_key, &current_value, &mut values);
    values
}

fn flush_infobox_field(
    keys: &[&str],
    current_key: &Option<String>,
    current_value: &str,
    values: &mut Vec<String>,
) {
    let Some(key) = current_key else {
        return;
    };
    if keys.iter().any(|candidate| key.contains(candidate)) {
        values.extend(split_people(current_value));
    }
}

fn extract_section_cast(wikitext: &str) -> Vec<String> {
    let mut in_section = false;
    let mut values = Vec::new();
    for raw_line in wikitext.lines() {
        let line = raw_line.trim();
        if line.starts_with("==") && line.ends_with("==") {
            let heading = normalize_text(line.trim_matches('=').trim());
            if in_section {
                break;
            }
            in_section = matches!(
                heading.as_str(),
                "reparto" | "elenco" | "protagonistas" | "cast" | "starring"
            );
            continue;
        }
        if in_section && line.starts_with('*') {
            values.push(line.trim_start_matches('*').trim().to_owned());
        }
    }
    values
}

fn split_people(value: &str) -> Vec<String> {
    let separators = Regex::new(r"(?i)<br\s*/?>|\n|\*|;").expect("valid separator regex");
    separators
        .split(value)
        .map(clean_wiki_markup)
        .flat_map(|part| {
            if part.contains(',') && part.matches(',').count() <= 8 {
                part.split(',').map(str::to_owned).collect::<Vec<_>>()
            } else {
                vec![part.to_owned()]
            }
        })
        .collect()
}

fn normalize_people(values: Vec<String>, limit: usize) -> Vec<String> {
    let mut output = Vec::new();
    for value in values {
        let person = clean_person(&value);
        if person.is_empty() || person.chars().count() < 3 {
            continue;
        }
        if !output
            .iter()
            .any(|existing: &String| normalize_text(existing) == normalize_text(&person))
        {
            output.push(person);
        }
        if output.len() >= limit {
            break;
        }
    }
    output
}

fn clean_person(value: &str) -> String {
    let mut value = clean_wiki_markup(value);
    for separator in [" como ", " as ", " - ", " – ", " — ", " interpreta "] {
        if let Some((before, _)) = value.split_once(separator) {
            value = before.to_owned();
        }
    }
    collapse_spaces(
        value
            .trim_matches(|ch: char| ch == '-' || ch == '–' || ch == ',' || ch.is_whitespace())
            .trim(),
    )
}

fn clean_wiki_markup(value: &str) -> String {
    let mut text = value.to_owned();
    for pattern in [
        r"(?s)<ref[^>]*>.*?</ref>",
        r"(?s)<ref[^/]*/>",
        r"(?s)<!--.*?-->",
        r"(?s)<[^>]+>",
    ] {
        text = Regex::new(pattern)
            .expect("valid cleanup regex")
            .replace_all(&text, " ")
            .into_owned();
    }
    for _ in 0..5 {
        let next = Regex::new(r"\{\{[^{}]*\}\}")
            .expect("valid template regex")
            .replace_all(&text, " ")
            .into_owned();
        if next == text {
            break;
        }
        text = next;
    }
    text = Regex::new(r"\[\[[^\]|]+\|([^\]]+)\]\]")
        .expect("valid pipe link regex")
        .replace_all(&text, "$1")
        .into_owned();
    text = Regex::new(r"\[\[([^\]]+)\]\]")
        .expect("valid link regex")
        .replace_all(&text, "$1")
        .into_owned();
    text = Regex::new(r"\[https?://[^\s\]]+\s+([^\]]+)\]")
        .expect("valid external link regex")
        .replace_all(&text, "$1")
        .into_owned();
    text.replace("'''", "").replace("''", "")
}

fn clean_page_title(title: &str) -> String {
    let cleaned = Regex::new(r"\s*\((película|film|movie|[0-9]{4})\)\s*")
        .expect("valid page title regex")
        .replace_all(title, " ")
        .into_owned();
    collapse_spaces(cleaned.trim())
}

fn source_url(language: &str, title: &str) -> String {
    format!(
        "https://{language}.wikipedia.org/wiki/{}",
        title.trim().replace(' ', "_")
    )
}

fn extract_year(value: &str) -> Option<i32> {
    Regex::new(r"\b(19|20)\d{2}\b")
        .expect("valid year regex")
        .find(value)
        .and_then(|year| year.as_str().parse().ok())
}

fn normalize_text(value: &str) -> String {
    value
        .to_lowercase()
        .chars()
        .map(|ch| match ch {
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

fn collapse_spaces(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    query: Option<SearchQuery>,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    search: Vec<SearchHit>,
}

#[derive(Debug, Deserialize)]
struct SearchHit {
    pageid: i64,
    snippet: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PageResponse {
    query: Option<PageQuery>,
}

#[derive(Debug, Deserialize)]
struct PageQuery {
    pages: Vec<WikiPage>,
}

#[derive(Debug, Deserialize)]
struct WikiPage {
    pageid: i64,
    title: String,
    extract: Option<String>,
    revisions: Option<Vec<WikiRevision>>,
}

impl WikiPage {
    fn wikitext(&self) -> Option<String> {
        self.revisions
            .as_ref()
            .and_then(|revisions| revisions.first())
            .and_then(|revision| {
                revision
                    .slots
                    .as_ref()
                    .and_then(|slots| slots.main.as_ref())
                    .and_then(|slot| slot.content.clone())
                    .or_else(|| revision.content.clone())
            })
    }
}

#[derive(Debug, Deserialize)]
struct WikiRevision {
    slots: Option<WikiSlots>,
    #[serde(rename = "*")]
    content: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WikiSlots {
    main: Option<WikiSlot>,
}

#[derive(Debug, Deserialize)]
struct WikiSlot {
    content: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_cast_from_infobox_fields() {
        let text = r#"
{{Ficha de película
| título = Matrix
| protagonistas = [[Keanu Reeves]]<br />[[Laurence Fishburne]]<br />[[Carrie-Anne Moss]]
}}
"#;
        assert_eq!(
            extract_cast(text),
            vec!["Keanu Reeves", "Laurence Fishburne", "Carrie-Anne Moss"]
        );
    }

    #[test]
    fn extracts_cast_from_section_without_roles() {
        let text = r#"
== Reparto ==
* [[Keanu Reeves]] como [[Neo]]
* [[Carrie-Anne Moss]] como Trinity
== Producción ==
"#;
        assert_eq!(extract_cast(text), vec!["Keanu Reeves", "Carrie-Anne Moss"]);
    }

    #[test]
    fn rejects_disambiguation_extracts() {
        assert!(first_valid_paragraph("Matrix puede referirse a varias obras.").is_none());
    }
}
