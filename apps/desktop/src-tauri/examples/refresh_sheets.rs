//! Refetches every title sheet from the outside, filling data that older imports never stored.
//!
//! The application offers the same run from Configuración. This binary exists so the catch-up can
//! also happen without the window open, over the same code paths the application uses: same
//! database, same provider client, same portable package writer.
//!
//! Run it with the application closed, so the two processes do not fight over the database.

use cine_wana_desktop_lib::portable_library;
use cinewana_database::Database;
use cinewana_metadata::{MetadataSearchOutcome, TmdbMetadataClient};
use std::{path::PathBuf, sync::Arc};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let app_data = PathBuf::from(std::env::var("APPDATA")?).join("com.cinewana.app");
    let cache_dir = app_data.join("cache");
    let db = Arc::new(Database::open(app_data.join("cine-wana.db"))?);
    let metadata = TmdbMetadataClient::from_environment()?;

    let targets = db.metadata_refresh_targets()?;
    let total = targets.len();
    println!("Fichas a actualizar: {total}");

    let (mut updated, mut ambiguous, mut missing, mut failed) = (0_usize, 0, 0, 0);
    for (index, target) in targets.into_iter().enumerate() {
        let position = index + 1;
        /* Si ya sabemos cual es, se le pide por su numero: buscar por nombre algo ya identificado
           es lo que vuelve ambigua una pelicula que estaba resuelta. */
        let known = db.metadata_source_url(&target.media_id).unwrap_or(None);
        let outcome = match known {
            Some(url) => metadata
                .import_from_source(&url, target.season_number, target.episode_number)
                .await
                .map(MetadataSearchOutcome::Imported),
            None => {
                metadata
                    .search_media(
                        &target.title,
                        target.year,
                        &target.kind,
                        target.season_number,
                        target.episode_number,
                    )
                    .await
            }
        };
        match outcome {
            Ok(MetadataSearchOutcome::Imported(imported)) => {
                let json_path =
                    cinewana_metadata::write_metadata_json(&cache_dir, &target.fingerprint, &imported)?;
                /* Una pelicula que falla no puede cortar la corrida entera: se cuenta y se sigue. */
                let artwork = match metadata
                    .cache_artwork(&cache_dir, &target.fingerprint, &imported)
                    .await
                {
                    Ok(artwork) => artwork,
                    Err(error) => {
                        failed += 1;
                        eprintln!("[{position}/{total}] {} — imagenes: {error}", target.title);
                        continue;
                    }
                };
                db.apply_imported_metadata(
                    &target.media_id,
                    &imported,
                    Some(&json_path.to_string_lossy()),
                    artwork
                        .poster_path
                        .as_ref()
                        .map(|path| path.to_string_lossy())
                        .as_deref(),
                    artwork
                        .backdrop_path
                        .as_ref()
                        .map(|path| path.to_string_lossy())
                        .as_deref(),
                    false,
                    &artwork.people,
                )?;
                /* Deja la copia que viaja: las fotos quedan al lado del video, no solo en el cache. */
                if let Err(error) = portable_library::sync_media(&db, &target.media_id) {
                    eprintln!("  aviso: no se pudo escribir el paquete portatil: {error}");
                }
                updated += 1;
                let faces = artwork
                    .people
                    .iter()
                    .filter(|person| person.photo_url.is_some())
                    .count();
                println!("[{position}/{total}] {} — {faces} fotos", target.title);
            }
            Ok(MetadataSearchOutcome::Ambiguous(candidates)) => {
                db.store_metadata_candidates(&target.media_id, &candidates)?;
                ambiguous += 1;
                println!("[{position}/{total}] {} — varias posibilidades", target.title);
            }
            Ok(MetadataSearchOutcome::NotFound) => {
                db.store_metadata_candidates(&target.media_id, &[])?;
                missing += 1;
                println!("[{position}/{total}] {} — sin coincidencia", target.title);
            }
            Err(error) => {
                failed += 1;
                eprintln!("[{position}/{total}] {} — error: {error}", target.title);
            }
        }
    }

    println!("\nListo. Actualizadas {updated}, ambiguas {ambiguous}, sin coincidencia {missing}, con error {failed}.");
    Ok(())
}
