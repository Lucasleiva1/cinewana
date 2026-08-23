//! Canonical genre vocabulary shared by the scanner, the metadata importers and the catalog.
//!
//! TMDB answers in several shapes for the same idea: `Suspense` for movies, `Action & Adventure`
//! and `Sci-Fi & Fantasy` for series, and English names whenever the Spanish catalog is missing.
//! Every consumer funnels raw provider strings through [`canonical_genres`] so the library groups
//! titles under one stable Spanish label instead of several near-duplicates.

/// Stable identifier for the row that holds every title without a usable genre.
pub const UNCATEGORIZED_ID: &str = "sin-categoria";
/// Visible label for [`UNCATEGORIZED_ID`].
pub const UNCATEGORIZED_LABEL: &str = "Sin categoría";
/// Stable identifier for the row that holds movie collections.
pub const SAGAS_ID: &str = "sagas";
/// Visible label for [`SAGAS_ID`].
pub const SAGAS_LABEL: &str = "Sagas";
/// Prefix used to build the identifier of a series-only genre row.
pub const SERIES_PREFIX: &str = "series-";

/// Canonical genres in the order used whenever no account preference exists yet.
pub const CANONICAL_GENRES: &[(&str, &str)] = &[
    ("ciencia-ficcion", "Ciencia ficción"),
    ("accion", "Acción"),
    ("aventura", "Aventura"),
    ("animacion", "Animación"),
    ("belica", "Bélica"),
    ("comedia", "Comedia"),
    ("crimen", "Crimen"),
    ("documental", "Documental"),
    ("drama", "Drama"),
    ("familia", "Familia"),
    ("fantasia", "Fantasía"),
    ("historia", "Historia"),
    ("misterio", "Misterio"),
    ("musica", "Música"),
    ("romance", "Romance"),
    ("suspenso", "Suspenso"),
    ("terror", "Terror"),
    ("western", "Western"),
];

/// Raw provider spellings mapped onto one or more canonical genres.
///
/// Keys are compared after [`fold`], so accents and casing are irrelevant here.
const SYNONYMS: &[(&str, &[&str])] = &[
    ("accion", &["accion"]),
    ("action", &["accion"]),
    ("action & adventure", &["accion", "aventura"]),
    ("adventure", &["aventura"]),
    ("aventura", &["aventura"]),
    ("animation", &["animacion"]),
    ("animacion", &["animacion"]),
    ("anime", &["animacion"]),
    ("belica", &["belica"]),
    ("war", &["belica"]),
    ("war & politics", &["belica"]),
    ("guerra", &["belica"]),
    ("comedy", &["comedia"]),
    ("comedia", &["comedia"]),
    ("crime", &["crimen"]),
    ("crimen", &["crimen"]),
    ("documentary", &["documental"]),
    ("documental", &["documental"]),
    ("drama", &["drama"]),
    ("family", &["familia"]),
    ("familia", &["familia"]),
    ("kids", &["familia"]),
    ("infantil", &["familia"]),
    ("fantasy", &["fantasia"]),
    ("fantasia", &["fantasia"]),
    ("history", &["historia"]),
    ("historia", &["historia"]),
    ("music", &["musica"]),
    ("musica", &["musica"]),
    ("musical", &["musica"]),
    ("mystery", &["misterio"]),
    ("misterio", &["misterio"]),
    ("romance", &["romance"]),
    ("romantica", &["romance"]),
    ("science fiction", &["ciencia-ficcion"]),
    ("ciencia ficcion", &["ciencia-ficcion"]),
    ("sci-fi", &["ciencia-ficcion"]),
    ("scifi", &["ciencia-ficcion"]),
    ("sci-fi & fantasy", &["ciencia-ficcion", "fantasia"]),
    ("suspense", &["suspenso"]),
    ("suspenso", &["suspenso"]),
    ("thriller", &["suspenso"]),
    ("horror", &["terror"]),
    ("terror", &["terror"]),
    ("western", &["western"]),
    ("soap", &["drama"]),
    ("telenovela", &["drama"]),
];

/// Provider values that carry no shelf meaning and are dropped instead of becoming a row.
const DISCARDED: &[&str] = &[
    "tv movie",
    "pelicula de tv",
    "reality",
    "news",
    "noticias",
    "talk",
    "sin genero",
    "sin categoria",
];

/// Lowercases `value` and folds Spanish accents so lookups ignore spelling noise.
pub(crate) fn fold(value: &str) -> String {
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
            'ç' => 'c',
            other => other,
        })
        .collect()
}

/// Returns the visible label of a canonical genre slug.
pub fn genre_label(slug: &str) -> Option<&'static str> {
    CANONICAL_GENRES
        .iter()
        .find(|(id, _)| *id == slug)
        .map(|(_, label)| *label)
}

/// Returns the canonical slug of a genre label, accepting any known provider spelling.
pub fn genre_slug(raw: &str) -> Option<&'static str> {
    let folded = fold(raw);
    if folded.is_empty() || DISCARDED.contains(&folded.as_str()) {
        return None;
    }
    SYNONYMS
        .iter()
        .find(|(key, _)| *key == folded)
        .and_then(|(_, slugs)| slugs.first().copied())
        .or_else(|| {
            CANONICAL_GENRES
                .iter()
                .find(|(id, _)| *id == folded.replace(' ', "-"))
                .map(|(id, _)| *id)
        })
}

/// Expands one raw provider genre into the canonical slugs it represents.
///
/// A single provider value may fan out into two shelves: `Sci-Fi & Fantasy` belongs on both the
/// science-fiction and the fantasy row.
fn expand(raw: &str) -> Vec<&'static str> {
    let folded = fold(raw);
    if folded.is_empty() || DISCARDED.contains(&folded.as_str()) {
        return Vec::new();
    }
    if let Some((_, slugs)) = SYNONYMS.iter().find(|(key, _)| *key == folded) {
        return slugs.to_vec();
    }
    CANONICAL_GENRES
        .iter()
        .find(|(id, _)| *id == folded.replace(' ', "-"))
        .map(|(id, _)| vec![*id])
        .unwrap_or_default()
}

/// Normalizes raw provider genres into canonical labels, deduplicated and ordered like
/// [`CANONICAL_GENRES`].
///
/// Unknown values are dropped rather than guessed: a title whose every genre is unknown ends up
/// with an empty list, which is what places it on the uncategorized shelf.
pub fn canonical_genres<S: AsRef<str>>(raw: &[S]) -> Vec<String> {
    let mut slugs: Vec<&'static str> = Vec::new();
    for value in raw {
        for slug in expand(value.as_ref()) {
            if !slugs.contains(&slug) {
                slugs.push(slug);
            }
        }
    }
    CANONICAL_GENRES
        .iter()
        .filter(|(id, _)| slugs.contains(id))
        .map(|(_, label)| (*label).to_owned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_tmdb_movie_and_series_spellings_onto_one_vocabulary() {
        assert_eq!(canonical_genres(&["Suspense"]), vec!["Suspenso"]);
        assert_eq!(
            canonical_genres(&["Action & Adventure"]),
            vec!["Acción", "Aventura"]
        );
        assert_eq!(
            canonical_genres(&["Sci-Fi & Fantasy"]),
            vec!["Ciencia ficción", "Fantasía"]
        );
        assert_eq!(canonical_genres(&["horror"]), vec!["Terror"]);
    }

    #[test]
    fn deduplicates_and_orders_canonically() {
        let genres = canonical_genres(&["Terror", "Suspense", "Thriller", "terror", "Acción"]);
        assert_eq!(genres, vec!["Acción", "Suspenso", "Terror"]);
    }

    #[test]
    fn drops_values_that_do_not_describe_a_shelf() {
        assert!(canonical_genres(&["Película de TV"]).is_empty());
        assert!(canonical_genres(&["Reality"]).is_empty());
        assert!(canonical_genres(&[""]).is_empty());
        assert!(canonical_genres(&["género inventado"]).is_empty());
    }

    #[test]
    fn resolves_slugs_and_labels_in_both_directions() {
        assert_eq!(genre_slug("Ciencia ficción"), Some("ciencia-ficcion"));
        assert_eq!(genre_slug("Suspense"), Some("suspenso"));
        assert_eq!(genre_label("terror"), Some("Terror"));
        assert_eq!(genre_label("inexistente"), None);
    }
}
