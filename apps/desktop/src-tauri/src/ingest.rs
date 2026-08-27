//! La bandeja «peliculas nuevas».
//!
//! Es el único lugar que la aplicación mira al abrir. Se deja ahí una película recién bajada, se
//! procesa, y se muda sola a la carpeta PELICULAS. Las carpetas que ya están terminadas no se
//! vuelven a recorrer: por eso abrir el programa dejó de costar un escaneo entero.
//!
//! Una película con problemas se muda igual y queda marcada dentro del programa para corregirla a
//! mano. La bandeja tiene que terminar vacía siempre, porque lo que queda adentro se reprocesa en
//! el próximo arranque.

use anyhow::{Context, Result};
use cinewana_core::is_supported_video;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Carpeta donde se dejan las películas nuevas. En minúscula a propósito: al lado de PELICULAS y
/// SERIES en mayúscula se lee como una instrucción y no como una parte más de la biblioteca.
pub const TRAY_DIR: &str = "peliculas nuevas";

/// Nombre normalizado de la carpeta que guarda las películas ya terminadas.
const LIBRARY_DIR_KEY: &str = "peliculas";

/// Nombre por defecto si la biblioteca todavía no tiene su carpeta de películas.
const LIBRARY_DIR_FALLBACK: &str = "PELICULAS";

const TRAY_NOTE_FILE: &str = "LEEME.txt";
const TRAY_NOTE: &str = "\
Dejá acá las películas nuevas.

Podés soltar la carpeta entera de la película o el archivo de video suelto.

Cuando abras CINE WANA cada película se procesa sola (datos tecnicos, portada, ficha y actores) y
se muda a la carpeta PELICULAS. Esta carpeta vuelve a quedar vacía.

Si una película queda con algun problema igual se muda, y aparece marcada dentro del programa para
que la corrijas a mano.

Las series no van acá: esas se ponen directamente en SERIES y se usa el botón de reescanear.
";

/// Una película esperando en la bandeja, con los videos que trae adentro.
#[derive(Debug, Clone)]
pub struct PendingMovie {
    /// Lo que se soltó en la bandeja: una carpeta de película o un archivo de video suelto.
    pub entry: PathBuf,
    /// Nombre que se le va a mostrar mientras se procesa.
    pub label: String,
    /// Videos encontrados adentro, en el orden en que los descubre el escáner.
    pub videos: Vec<PathBuf>,
}

/// Una película ya mudada, con los videos apuntando a su ubicación definitiva.
#[derive(Debug, Clone)]
pub struct MovedMovie {
    pub destination: PathBuf,
    pub videos: Vec<PathBuf>,
}

/// Compara nombres de carpeta sin distinguir mayúsculas ni acentos.
///
/// La biblioteca fue armada a mano durante años, así que `PELICULAS`, `Peliculas` y `películas`
/// tienen que resolver todos al mismo lugar.
fn normalize(value: &str) -> String {
    value
        .trim()
        .to_lowercase()
        .chars()
        .map(|character| match character {
            'á' => 'a',
            'é' => 'e',
            'í' => 'i',
            'ó' => 'o',
            'ú' => 'u',
            'ü' => 'u',
            other => other,
        })
        .collect()
}

/// Ruta de la bandeja dentro de una biblioteca.
pub fn tray_dir(root: &Path) -> PathBuf {
    root.join(TRAY_DIR)
}

/// Crea la bandeja si no existe y deja adentro la nota que la explica.
///
/// Se llama en cada arranque porque la carpeta puede haber sido borrada desde el Explorador, y sin
/// ella no hay dónde dejar las películas nuevas.
pub fn ensure_tray(root: &Path) -> Result<PathBuf> {
    let tray = tray_dir(root);
    fs::create_dir_all(&tray)
        .with_context(|| format!("crear la bandeja {}", tray.display()))?;
    let note = tray.join(TRAY_NOTE_FILE);
    if !note.exists() {
        /* Una nota que no se puede escribir no es motivo para frenar el arranque. */
        let _ = fs::write(&note, TRAY_NOTE);
    }
    Ok(tray)
}

/// Encuentra la carpeta de películas terminadas dentro de la biblioteca.
///
/// Busca por nombre en vez de fijarlo, porque la carpeta ya existe con la forma que le dio el
/// usuario y renombrarla desde el programa movería 285 películas de lugar.
pub fn library_dir(root: &Path) -> PathBuf {
    if let Ok(entries) = fs::read_dir(root) {
        for entry in entries.filter_map(std::result::Result::ok) {
            if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else { continue };
            if normalize(name) == LIBRARY_DIR_KEY {
                return entry.path();
            }
        }
    }
    root.join(LIBRARY_DIR_FALLBACK)
}

/// Lista lo que está esperando en la bandeja.
///
/// Devuelve vacío cuando no hay nada, que es el caso normal: por eso abrir el programa no cuesta
/// nada cuando no agregaste películas.
pub fn pending(tray: &Path) -> Result<Vec<PendingMovie>> {
    if !tray.is_dir() {
        return Ok(Vec::new());
    }
    let mut pending = Vec::new();
    let entries = fs::read_dir(tray)
        .with_context(|| format!("leer la bandeja {}", tray.display()))?;
    for entry in entries.filter_map(std::result::Result::ok) {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        /* La nota y las carpetas de datos de la propia aplicación no son películas. */
        if name.starts_with('.') || name == TRAY_NOTE_FILE {
            continue;
        }
        let Ok(kind) = entry.file_type() else { continue };
        let videos = if kind.is_dir() {
            cinewana_scanner::discover(&path, true).unwrap_or_default()
        } else if is_supported_video(&path) {
            vec![path.clone()]
        } else {
            continue;
        };
        if videos.is_empty() {
            continue;
        }
        pending.push(PendingMovie {
            entry: path,
            label: name,
            videos,
        });
    }
    pending.sort_by(|a, b| a.label.to_lowercase().cmp(&b.label.to_lowercase()));
    Ok(pending)
}

/// Muda una película de la bandeja a la biblioteca y devuelve las rutas definitivas.
///
/// Un video suelto se envuelve en su propia carpeta, porque las 285 películas que ya están usan esa
/// forma y una mezcla rompería el orden del Explorador.
pub fn move_into_library(movie: &PendingMovie, library: &Path) -> Result<MovedMovie> {
    fs::create_dir_all(library)
        .with_context(|| format!("crear la carpeta de películas {}", library.display()))?;
    let is_directory = movie.entry.is_dir();
    let stem = if is_directory {
        movie.label.clone()
    } else {
        Path::new(&movie.label)
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or(&movie.label)
            .to_owned()
    };
    let destination = unique_destination(library, &stem);

    if is_directory {
        move_path(&movie.entry, &destination)?;
        let videos = movie
            .videos
            .iter()
            .map(|video| rebase(video, &movie.entry, &destination))
            .collect::<Result<Vec<_>>>()?;
        return Ok(MovedMovie {
            destination,
            videos,
        });
    }

    /* Archivo suelto: se le arma la carpeta y se lleva también lo que lo acompaña, como los
       subtítulos externos, que comparten el nombre y quedarían huérfanos. */
    fs::create_dir_all(&destination)
        .with_context(|| format!("crear la carpeta {}", destination.display()))?;
    let mut videos = Vec::new();
    for companion in companions(&movie.entry)? {
        let Some(name) = companion.file_name() else {
            continue;
        };
        let target = destination.join(name);
        move_path(&companion, &target)?;
        if companion == movie.entry {
            videos.push(target);
        }
    }
    Ok(MovedMovie {
        destination,
        videos,
    })
}

/// El archivo pedido más los que comparten su nombre, como `pelicula.es.srt` junto a `pelicula.mkv`.
fn companions(video: &Path) -> Result<Vec<PathBuf>> {
    let Some(parent) = video.parent() else {
        return Ok(vec![video.to_path_buf()]);
    };
    let Some(stem) = video.file_stem().and_then(|value| value.to_str()) else {
        return Ok(vec![video.to_path_buf()]);
    };
    let stem = stem.to_lowercase();
    let mut found = vec![video.to_path_buf()];
    for entry in fs::read_dir(parent)
        .with_context(|| format!("leer {}", parent.display()))?
        .filter_map(std::result::Result::ok)
    {
        let path = entry.path();
        if path == video || !path.is_file() {
            continue;
        }
        let matches = path
            .file_name()
            .and_then(|value| value.to_str())
            .is_some_and(|name| name.to_lowercase().starts_with(&stem));
        if matches {
            found.push(path);
        }
    }
    Ok(found)
}

/// Traduce la ruta de un video al lugar donde quedó después de la mudanza.
fn rebase(video: &Path, from: &Path, to: &Path) -> Result<PathBuf> {
    let relative = video
        .strip_prefix(from)
        .with_context(|| format!("{} no está dentro de {}", video.display(), from.display()))?;
    Ok(to.join(relative))
}

/// Busca un nombre libre en la biblioteca para no pisar nunca una película existente.
fn unique_destination(library: &Path, stem: &str) -> PathBuf {
    let candidate = library.join(stem);
    if !candidate.exists() {
        return candidate;
    }
    for suffix in 2..1000u32 {
        let candidate = library.join(format!("{stem} ({suffix})"));
        if !candidate.exists() {
            return candidate;
        }
    }
    library.join(format!("{stem} ({})", uuid::Uuid::new_v4()))
}

/// Mueve una ruta, cayendo en copiar y borrar solo si el destino está en otro disco.
///
/// Dentro del mismo disco `rename` es instantáneo porque no toca los bytes, y una película de 40 GB
/// se muda en un parpadeo. El respaldo copia primero y recién borra el origen cuando la copia
/// terminó bien, así un corte de luz nunca deja a la película en la nada.
fn move_path(from: &Path, to: &Path) -> Result<()> {
    if fs::rename(from, to).is_ok() {
        return Ok(());
    }
    if from.is_dir() {
        copy_dir(from, to)?;
        fs::remove_dir_all(from)
            .with_context(|| format!("borrar el original {}", from.display()))?;
    } else {
        fs::copy(from, to)
            .with_context(|| format!("copiar {} a {}", from.display(), to.display()))?;
        fs::remove_file(from)
            .with_context(|| format!("borrar el original {}", from.display()))?;
    }
    Ok(())
}

fn copy_dir(from: &Path, to: &Path) -> Result<()> {
    fs::create_dir_all(to).with_context(|| format!("crear {}", to.display()))?;
    for entry in fs::read_dir(from)
        .with_context(|| format!("leer {}", from.display()))?
        .filter_map(std::result::Result::ok)
    {
        let source = entry.path();
        let target = to.join(entry.file_name());
        if source.is_dir() {
            copy_dir(&source, &target)?;
        } else {
            fs::copy(&source, &target)
                .with_context(|| format!("copiar {} a {}", source.display(), target.display()))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Carpeta descartable para no tocar la biblioteca real durante las pruebas.
    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("cinewana-ingest-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: &Path, contents: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, contents).unwrap();
    }

    #[test]
    fn library_folder_matches_regardless_of_case_and_accents() {
        assert_eq!(normalize("PELICULAS"), LIBRARY_DIR_KEY);
        assert_eq!(normalize("Películas"), LIBRARY_DIR_KEY);
        assert_eq!(normalize(" peliculas "), LIBRARY_DIR_KEY);
    }

    #[test]
    fn the_tray_is_never_mistaken_for_the_library() {
        assert_ne!(normalize(TRAY_DIR), LIBRARY_DIR_KEY);
    }

    #[test]
    fn the_library_folder_is_found_as_the_user_named_it() {
        let root = TempRoot::new();
        fs::create_dir_all(root.path().join("PELICULAS")).unwrap();
        assert_eq!(library_dir(root.path()), root.path().join("PELICULAS"));
    }

    #[test]
    fn an_empty_tray_has_nothing_to_do() {
        let root = TempRoot::new();
        let tray = ensure_tray(root.path()).unwrap();
        assert!(tray.is_dir());
        assert!(pending(&tray).unwrap().is_empty(), "la nota no es una película");
    }

    #[test]
    fn a_dropped_folder_moves_whole_and_the_video_keeps_being_found() {
        let root = TempRoot::new();
        let tray = ensure_tray(root.path()).unwrap();
        let library = root.path().join("PELICULAS");
        write(&tray.join("Ad.astra.2019").join("Ad.astra.2019.mkv"), "video");

        let waiting = pending(&tray).unwrap();
        assert_eq!(waiting.len(), 1);

        let moved = move_into_library(&waiting[0], &library).unwrap();
        assert_eq!(moved.destination, library.join("Ad.astra.2019"));
        assert_eq!(moved.videos.len(), 1);
        assert!(moved.videos[0].is_file(), "el video tiene que existir donde quedó");
        assert!(
            pending(&tray).unwrap().is_empty(),
            "la bandeja tiene que quedar vacía o se reprocesa en cada arranque"
        );
    }

    #[test]
    fn a_loose_video_gets_its_own_folder_and_its_subtitles_travel_with_it() {
        let root = TempRoot::new();
        let tray = ensure_tray(root.path()).unwrap();
        let library = root.path().join("PELICULAS");
        write(&tray.join("300.2007.mkv"), "video");
        write(&tray.join("300.2007.es.srt"), "subtitulos");

        let waiting = pending(&tray).unwrap();
        assert_eq!(waiting.len(), 1, "el subtítulo no es una película aparte");

        let moved = move_into_library(&waiting[0], &library).unwrap();
        assert_eq!(moved.destination, library.join("300.2007"));
        assert!(library.join("300.2007").join("300.2007.mkv").is_file());
        assert!(
            library.join("300.2007").join("300.2007.es.srt").is_file(),
            "el subtítulo quedaría huérfano si no viaja con la película"
        );
    }

    #[test]
    fn an_existing_title_is_never_overwritten() {
        let root = TempRoot::new();
        let tray = ensure_tray(root.path()).unwrap();
        let library = root.path().join("PELICULAS");
        write(&library.join("300.2007").join("300.2007.mkv"), "la que ya estaba");
        write(&tray.join("300.2007").join("300.2007.mkv"), "la nueva");

        let waiting = pending(&tray).unwrap();
        let moved = move_into_library(&waiting[0], &library).unwrap();

        assert_eq!(moved.destination, library.join("300.2007 (2)"));
        assert_eq!(
            fs::read_to_string(library.join("300.2007").join("300.2007.mkv")).unwrap(),
            "la que ya estaba",
            "una película de la biblioteca no se pisa nunca"
        );
    }
}
