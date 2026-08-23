use anyhow::{Context, Result};
use cinewana_core::{MediaPerson, PortableMediaMetadata};
use cinewana_database::Database;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use uuid::Uuid;

const DIRECTORY_NAME: &str = ".cinewana";
const ITEMS_DIRECTORY: &str = "items";
const METADATA_FILE: &str = "metadata.json";
const INDEX_FILE: &str = "index.json";
const CAST_DIRECTORY: &str = "cast";

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PortableIndex {
    schema_version: u32,
    entries: BTreeMap<String, String>,
}

pub fn sync_media(db: &Database, media_id: &str) -> Result<()> {
    let Some(mut entry) = db.portable_media_export(media_id)? else {
        return Ok(());
    };
    let video_path = PathBuf::from(&entry.video_path);
    let folder = video_path
        .parent()
        .context("the video does not have a containing folder")?;
    let portable_id = safe_portable_id(&entry.metadata.portable_id)?;
    let item_directory = folder
        .join(DIRECTORY_NAME)
        .join(ITEMS_DIRECTORY)
        .join(&portable_id);
    fs::create_dir_all(&item_directory).with_context(|| {
        format!(
            "create portable metadata folder {}",
            item_directory.display()
        )
    })?;

    entry.metadata.poster_file =
        copy_artwork(entry.poster_path.as_deref(), &item_directory, "poster")?;
    entry.metadata.backdrop_file =
        copy_artwork(entry.backdrop_path.as_deref(), &item_directory, "backdrop")?;
    copy_people_photos(&mut entry.metadata.people, &item_directory)?;
    let metadata_path = item_directory.join(METADATA_FILE);
    write_json_atomic(&metadata_path, &entry.metadata)?;
    update_index(folder, &entry.metadata.video_file_name, &portable_id)?;
    Ok(())
}

pub fn restore_media(
    db: &Database,
    cache_dir: &Path,
    media_id: &str,
    video_path: &Path,
    file_size: i64,
    fingerprint: Option<&str>,
) -> Result<bool> {
    let Some((metadata, metadata_path)) = load_metadata(video_path, file_size, fingerprint)? else {
        return Ok(false);
    };
    let item_directory = metadata_path
        .parent()
        .context("portable metadata file does not have a parent")?;
    let poster_path = cache_portable_artwork(
        item_directory,
        metadata.poster_file.as_deref(),
        cache_dir,
        &metadata.portable_id,
        "posters",
    )?;
    let backdrop_path = cache_portable_artwork(
        item_directory,
        metadata.backdrop_file.as_deref(),
        cache_dir,
        &metadata.portable_id,
        "backdrops",
    )?;
    let mut metadata = metadata;
    restore_people_photos(&mut metadata.people, item_directory, cache_dir)?;
    db.apply_portable_metadata(
        media_id,
        &metadata,
        &metadata_path.to_string_lossy(),
        poster_path
            .as_deref()
            .map(|path| path.to_string_lossy())
            .as_deref(),
        backdrop_path
            .as_deref()
            .map(|path| path.to_string_lossy())
            .as_deref(),
    )?;
    Ok(true)
}

fn load_metadata(
    video_path: &Path,
    file_size: i64,
    fingerprint: Option<&str>,
) -> Result<Option<(PortableMediaMetadata, PathBuf)>> {
    let Some(folder) = video_path.parent() else {
        return Ok(None);
    };
    let items_directory = folder.join(DIRECTORY_NAME).join(ITEMS_DIRECTORY);
    if !items_directory.is_dir() {
        return Ok(None);
    }
    let file_name = video_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    let index_path = folder.join(DIRECTORY_NAME).join(INDEX_FILE);
    if let Ok(bytes) = fs::read(&index_path)
        && let Ok(index) = serde_json::from_slice::<PortableIndex>(&bytes)
        && let Some(portable_id) = index.entries.get(&file_name.to_lowercase())
        && let Some(found) = read_candidate(
            &items_directory.join(portable_id).join(METADATA_FILE),
            file_name,
            file_size,
            fingerprint,
        )?
    {
        return Ok(Some(found));
    }

    for item in fs::read_dir(&items_directory)
        .with_context(|| {
            format!(
                "read portable metadata folder {}",
                items_directory.display()
            )
        })?
        .filter_map(std::result::Result::ok)
    {
        let metadata_path = item.path().join(METADATA_FILE);
        if let Some(found) = read_candidate(&metadata_path, file_name, file_size, fingerprint)? {
            return Ok(Some(found));
        }
    }
    Ok(None)
}

fn read_candidate(
    metadata_path: &Path,
    video_file_name: &str,
    file_size: i64,
    fingerprint: Option<&str>,
) -> Result<Option<(PortableMediaMetadata, PathBuf)>> {
    if !metadata_path.is_file() {
        return Ok(None);
    }
    let metadata: PortableMediaMetadata = serde_json::from_slice(
        &fs::read(metadata_path)
            .with_context(|| format!("read portable metadata {}", metadata_path.display()))?,
    )
    .with_context(|| format!("parse portable metadata {}", metadata_path.display()))?;
    if metadata.schema_version != 1 {
        return Ok(None);
    }
    let fingerprint_matches = fingerprint.is_some_and(|value| value == metadata.fingerprint);
    let file_matches = metadata.file_size == file_size
        && metadata
            .video_file_name
            .eq_ignore_ascii_case(video_file_name);
    Ok((fingerprint_matches || file_matches).then(|| (metadata, metadata_path.to_path_buf())))
}

fn update_index(folder: &Path, video_file_name: &str, portable_id: &str) -> Result<()> {
    let directory = folder.join(DIRECTORY_NAME);
    fs::create_dir_all(&directory)?;
    let path = directory.join(INDEX_FILE);
    let mut index = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<PortableIndex>(&bytes).ok())
        .unwrap_or_default();
    index.schema_version = 1;
    index
        .entries
        .insert(video_file_name.to_lowercase(), portable_id.to_owned());
    write_json_atomic(&path, &index)
}

/// Copies every credited photo next to the video, inside the item's own `cast` folder.
///
/// The same actor is duplicated across the movies that credit them, on purpose: each folder has to
/// stand on its own so the library still shows faces after the drive is plugged into a computer
/// that never saw this metadata.
fn copy_people_photos(people: &mut [MediaPerson], item_directory: &Path) -> Result<()> {
    if people.is_empty() {
        return Ok(());
    }
    let cast_directory = item_directory.join(CAST_DIRECTORY);
    fs::create_dir_all(&cast_directory)
        .with_context(|| format!("create portable cast folder {}", cast_directory.display()))?;
    for person in people.iter_mut() {
        let Some(source) = person.photo_url.as_deref().map(Path::new) else {
            continue;
        };
        let Some(file_name) = source.file_name().map(|name| name.to_string_lossy().into_owned())
        else {
            continue;
        };
        let destination = cast_directory.join(&file_name);
        if !destination_is_current(source, &destination) {
            if let Err(error) = fs::copy(source, &destination) {
                /* Una foto que falta no puede romper el guardado del resto de la ficha. */
                eprintln!("no se pudo copiar la foto {}: {error}", source.display());
                continue;
            }
        }
        person.photo_file = Some(file_name);
    }
    Ok(())
}

/// Brings the photos stored beside the video into the local cache and points the people at them.
fn restore_people_photos(
    people: &mut [MediaPerson],
    item_directory: &Path,
    cache_dir: &Path,
) -> Result<()> {
    if people.is_empty() {
        return Ok(());
    }
    let cast_directory = item_directory.join(CAST_DIRECTORY);
    let target_directory = cache_dir.join("tmdb").join("profiles");
    fs::create_dir_all(&target_directory)?;
    for person in people.iter_mut() {
        let Some(file_name) = person.photo_file.clone() else {
            continue;
        };
        let source = cast_directory.join(&file_name);
        if !source.is_file() {
            person.photo_url = None;
            continue;
        }
        let destination = target_directory.join(&file_name);
        if !destination_is_current(&source, &destination) {
            fs::copy(&source, &destination).with_context(|| {
                format!("copy portable cast photo {}", source.display())
            })?;
        }
        person.photo_url = Some(destination.to_string_lossy().into_owned());
    }
    Ok(())
}

fn copy_artwork(source: Option<&str>, item_directory: &Path, stem: &str) -> Result<Option<String>> {
    let Some(source) = source.map(Path::new).filter(|path| path.is_file()) else {
        return Ok(None);
    };
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| value.len() <= 8)
        .unwrap_or("jpg")
        .to_ascii_lowercase();
    let file_name = format!("{stem}.{extension}");
    let destination = item_directory.join(&file_name);
    let already_copied = destination_is_current(source, &destination);
    if source != destination && !already_copied {
        let temporary = item_directory.join(format!(".{file_name}.{}.tmp", Uuid::new_v4()));
        fs::copy(source, &temporary).with_context(|| {
            format!(
                "copy portable artwork from {} to {}",
                source.display(),
                destination.display()
            )
        })?;
        replace_file(&temporary, &destination)?;
    }
    Ok(Some(file_name))
}

fn portable_artwork(item_directory: &Path, file_name: Option<&str>) -> Option<PathBuf> {
    let file_name = file_name?;
    let safe_name = Path::new(file_name).file_name()?.to_str()?;
    if safe_name != file_name {
        return None;
    }
    let path = item_directory.join(safe_name);
    path.is_file().then_some(path)
}

fn cache_portable_artwork(
    item_directory: &Path,
    file_name: Option<&str>,
    cache_dir: &Path,
    portable_id: &str,
    kind: &str,
) -> Result<Option<PathBuf>> {
    let Some(source) = portable_artwork(item_directory, file_name) else {
        return Ok(None);
    };
    let extension = source
        .extension()
        .and_then(|value| value.to_str())
        .filter(|value| value.len() <= 8)
        .unwrap_or("jpg")
        .to_ascii_lowercase();
    let safe_id = safe_portable_id(portable_id)?;
    let directory = cache_dir.join("portable").join(kind);
    fs::create_dir_all(&directory)?;
    let destination = directory.join(format!("{safe_id}.{extension}"));
    let already_cached = destination_is_current(&source, &destination);
    if already_cached {
        return Ok(Some(destination));
    }
    let temporary = directory.join(format!(".{safe_id}.{}.tmp", Uuid::new_v4()));
    fs::copy(&source, &temporary)?;
    replace_file(&temporary, &destination)?;
    Ok(Some(destination))
}

fn destination_is_current(source: &Path, destination: &Path) -> bool {
    let (Ok(source), Ok(destination)) = (fs::metadata(source), fs::metadata(destination)) else {
        return false;
    };
    source.len() == destination.len() && destination.modified().ok() >= source.modified().ok()
}

fn safe_portable_id(value: &str) -> Result<String> {
    let safe = value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric() || *character == '-')
        .take(80)
        .collect::<String>();
    if safe.is_empty() {
        anyhow::bail!("portable media identifier is empty");
    }
    Ok(safe)
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path
        .parent()
        .context("JSON destination does not have a parent")?;
    fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("data"),
        Uuid::new_v4()
    ));
    fs::write(&temporary, serde_json::to_vec_pretty(value)?)?;
    replace_file(&temporary, path)
}

fn replace_file(temporary: &Path, destination: &Path) -> Result<()> {
    if destination.exists() {
        fs::remove_file(destination)?;
    }
    fs::rename(temporary, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use cinewana_core::{MediaKind, MediaMetadataCandidate};

    /// The whole point of storing faces beside the video: another computer, with an empty cache,
    /// still shows the cast after reading the folder.
    #[test]
    fn cast_photos_travel_with_the_movie() {
        let root = std::env::temp_dir().join(format!("cinewana-cast-{}", Uuid::new_v4()));
        let item = root.join("origen").join(DIRECTORY_NAME).join(ITEMS_DIRECTORY).join("movie-1");
        let source_cache = root.join("cache-vieja").join("tmdb").join("profiles");
        fs::create_dir_all(&item).unwrap();
        fs::create_dir_all(&source_cache).unwrap();
        let photo = source_cache.join("ripley.jpg");
        fs::write(&photo, b"foto-de-prueba").unwrap();

        let mut people = vec![MediaPerson {
            name: "Sigourney Weaver".into(),
            role: cinewana_core::PersonRole::Actor,
            character: Some("Ripley".into()),
            photo_url: Some(photo.to_string_lossy().into_owned()),
            photo_file: None,
            photo_source: None,
        }];
        copy_people_photos(&mut people, &item).unwrap();
        assert_eq!(people[0].photo_file.as_deref(), Some("ripley.jpg"));
        assert!(
            item.join(CAST_DIRECTORY).join("ripley.jpg").is_file(),
            "la foto tiene que quedar al lado de la pelicula"
        );

        // Otra computadora: cache vacio, solo la carpeta que vino en el disco.
        let new_cache = root.join("cache-nueva");
        people[0].photo_url = None;
        restore_people_photos(&mut people, &item, &new_cache).unwrap();
        let restored = people[0].photo_url.clone().expect("la foto no se restauro");
        assert!(Path::new(&restored).is_file());
        assert_eq!(fs::read(&restored).unwrap(), b"foto-de-prueba");

        // Una foto borrada no puede dejar una ruta rota apuntando a la nada.
        fs::remove_file(item.join(CAST_DIRECTORY).join("ripley.jpg")).unwrap();
        let mut faltante = people.clone();
        restore_people_photos(&mut faltante, &item, &root.join("cache-tercera")).unwrap();
        assert!(faltante[0].photo_url.is_none());
        assert_eq!(faltante[0].name, "Sigourney Weaver", "el nombre sobrevive sin foto");

        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn portable_metadata_can_be_found_by_video_name_and_size() {
        let root = std::env::temp_dir().join(format!("cinewana-portable-{}", Uuid::new_v4()));
        let item = root
            .join(DIRECTORY_NAME)
            .join(ITEMS_DIRECTORY)
            .join("movie-1");
        fs::create_dir_all(&item).unwrap();
        let metadata = sample_metadata();
        write_json_atomic(&item.join(METADATA_FILE), &metadata).unwrap();
        update_index(&root, "Alien.mp4", "movie-1").unwrap();

        let loaded = load_metadata(&root.join("Alien.mp4"), 42, None)
            .unwrap()
            .unwrap()
            .0;

        assert_eq!(loaded.portable_id, "movie-1");
        assert_eq!(loaded.title, "Alien");
        fs::remove_dir_all(root).unwrap();
    }

    fn sample_metadata() -> PortableMediaMetadata {
        PortableMediaMetadata {
            schema_version: 1,
            portable_id: "movie-1".into(),
            video_file_name: "Alien.mp4".into(),
            file_size: 42,
            fingerprint: "fingerprint".into(),
            kind: MediaKind::Movie,
            title: "Alien".into(),
            year: Some(1979),
            overview: Some("En el espacio nadie puede oír tus gritos.".into()),
            genres: vec!["Terror".into()],
            cast: vec!["Sigourney Weaver".into()],
            series_title: None,
            season_number: None,
            episode_number: None,
            identification_source: "manual".into(),
            needs_review: false,
            review_reason: None,
            manual_classification: true,
            manual_metadata: true,
            metadata_status: "imported".into(),
            metadata_source_url: None,
            metadata_imported_at: None,
            metadata_candidates: Vec::<MediaMetadataCandidate>::new(),
            poster_file: Some("poster.jpg".into()),
            backdrop_file: None,
            people: vec![MediaPerson {
                name: "Sigourney Weaver".into(),
                role: cinewana_core::PersonRole::Actor,
                character: Some("Ripley".into()),
                photo_url: None,
                photo_file: Some("ripley.jpg".into()),
                photo_source: None,
            }],
            saga_id: Some("tmdb:8091".into()),
            saga_title: Some("Colección Alien".into()),
            saga_position: Some(1),
            updated_at: "2026-08-20T00:00:00Z".into(),
        }
    }
}
