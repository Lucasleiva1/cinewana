//! Fallback saga detection for movies the metadata provider could not identify.
//!
//! TMDB collections are the authoritative source and win whenever they exist. This module only
//! covers the remainder, where the file name is all we have. It is deliberately conservative:
//! grouping merely similar titles produces nonsense sagas such as `El día después del mañana`
//! plus `El día que la tierra se detuvo`, so a candidate is accepted only when the titles share a
//! full base *and* at least one of them carries an explicit part marker.

use crate::genres::fold;

/// Tokens carrying release or encoding noise rather than title words.
const NOISE: &[&str] = &[
    "1080p",
    "1080",
    "720p",
    "720",
    "2160p",
    "2160",
    "480p",
    "4k",
    "uhd",
    "hd",
    "fullhd",
    "bluray",
    "brrip",
    "bdrip",
    "hdrip",
    "dvdrip",
    "web",
    "webrip",
    "webdl",
    "x264",
    "x265",
    "h264",
    "h265",
    "hevc",
    "aac",
    "ac3",
    "dts",
    "latino",
    "castellano",
    "espanol",
    "ingles",
    "dual",
    "subtitulado",
    "sub",
    "subs",
    "vose",
    "extended",
    "remastered",
    "remasterizada",
    "uncut",
    "unrated",
];

/// A title parsed into the saga it may belong to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SagaGuess {
    /// Folded base used to match siblings.
    pub key: String,
    /// Human-readable base, taken from the original title.
    pub label: String,
    /// Position inside the saga; titles with no marker are treated as the first part.
    pub part: u32,
    /// Whether the position came from an explicit marker instead of the default.
    pub explicit: bool,
}

/// A group of titles accepted as one saga.
#[derive(Debug, Clone)]
pub struct SagaCandidate {
    /// Folded base shared by every member.
    pub key: String,
    /// Human-readable saga name.
    pub label: String,
    /// Index into the input slice plus the detected part, ordered by part.
    pub members: Vec<(usize, u32)>,
}

/// Reads a standalone part marker, accepting arabic numerals and roman numerals from `ii` up.
///
/// Bare `i` is rejected on purpose: it appears inside ordinary English titles far more often than
/// it marks a first installment.
fn part_marker(token: &str) -> Option<u32> {
    match token {
        "ii" => return Some(2),
        "iii" => return Some(3),
        "iv" => return Some(4),
        "v" => return Some(5),
        "vi" => return Some(6),
        "vii" => return Some(7),
        "viii" => return Some(8),
        "ix" => return Some(9),
        "x" => return Some(10),
        _ => {}
    }
    token
        .parse::<u32>()
        .ok()
        .filter(|value| (1..=12).contains(value))
}

/// Splits a title into significant tokens, dropping years and release noise.
fn tokenize(title: &str) -> Vec<(String, String)> {
    title
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| (token.to_owned(), fold(token)))
        .filter(|(_, folded)| !NOISE.contains(&folded.as_str()))
        .filter(|(_, folded)| {
            !(folded.len() == 4
                && folded.chars().all(|character| character.is_ascii_digit())
                && folded
                    .parse::<u32>()
                    .map(|year| (1900..=2100).contains(&year))
                    .unwrap_or(false))
        })
        .collect()
}

/// Parses a movie title into its saga base and part.
///
/// Returns `None` when nothing usable survives tokenization, or when the title is only a part
/// marker with no base to group on.
pub fn saga_guess(title: &str) -> Option<SagaGuess> {
    let tokens = tokenize(title);
    if tokens.is_empty() {
        return None;
    }
    let marker = tokens
        .iter()
        .enumerate()
        .skip(1)
        .find_map(|(index, (_, folded))| part_marker(folded).map(|part| (index, part)));
    let (base_len, part, explicit) = match marker {
        Some((index, part)) => (index, part, true),
        None => (tokens.len(), 1, false),
    };
    let base = &tokens[..base_len];
    let key = base
        .iter()
        .map(|(_, folded)| folded.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    if key.len() < 3 {
        return None;
    }
    let label = base
        .iter()
        .map(|(original, _)| original.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    Some(SagaGuess {
        key,
        label,
        part,
        explicit,
    })
}

/// Groups titles into sagas.
///
/// A group survives only when it holds at least two titles, spans at least two distinct parts and
/// carries at least one explicit marker. That last condition is what keeps duplicate files and
/// unrelated same-prefix titles from inventing a saga.
pub fn group_saga_candidates<S: AsRef<str>>(titles: &[S]) -> Vec<SagaCandidate> {
    let mut groups: Vec<(String, String, Vec<(usize, u32)>, bool)> = Vec::new();
    for (index, title) in titles.iter().enumerate() {
        let Some(guess) = saga_guess(title.as_ref()) else {
            continue;
        };
        match groups.iter_mut().find(|(key, ..)| *key == guess.key) {
            Some((_, _, members, explicit)) => {
                members.push((index, guess.part));
                *explicit = *explicit || guess.explicit;
            }
            None => groups.push((
                guess.key,
                guess.label,
                vec![(index, guess.part)],
                guess.explicit,
            )),
        }
    }
    groups
        .into_iter()
        .filter(|(_, _, members, explicit)| {
            if !*explicit || members.len() < 2 {
                return false;
            }
            let mut parts = members.iter().map(|(_, part)| *part).collect::<Vec<_>>();
            parts.sort_unstable();
            parts.dedup();
            parts.len() >= 2
        })
        .map(|(key, label, mut members, _)| {
            members.sort_by_key(|(index, part)| (*part, *index));
            SagaCandidate {
                key,
                label,
                members,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(titles: &[&str]) -> Vec<String> {
        let mut found = group_saga_candidates(titles)
            .into_iter()
            .map(|candidate| candidate.key)
            .collect::<Vec<_>>();
        found.sort();
        found
    }

    #[test]
    fn groups_numbered_installments() {
        let candidates = group_saga_candidates(&[
            "Chucky El Muñeco Diabólico",
            "Chucky El Muñeco Diabólico 2",
            "Chucky El Muñeco Diabólico 3",
        ]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].label, "Chucky El Muñeco Diabólico");
        assert_eq!(
            candidates[0]
                .members
                .iter()
                .map(|(_, part)| *part)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn reads_markers_placed_before_a_subtitle() {
        let candidates = group_saga_candidates(&[
            "Jeepers Creepers",
            "Jeepers creepers ii",
            "Jeepers Creepers 3 El Regreso Del Demonio",
        ]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].label, "Jeepers Creepers");
        assert_eq!(candidates[0].members.len(), 3);
    }

    #[test]
    fn ignores_release_noise_and_years() {
        let candidates =
            group_saga_candidates(&["Especies 1080", "Especies 2 1080p latino", "Especies 3 2004"]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].members.len(), 3);
    }

    #[test]
    fn refuses_unrelated_titles_that_merely_share_a_prefix() {
        assert!(keys(&[
            "El Día Después Del Mañana",
            "El Día Que La Tierra Se Detuvo",
            "El Dia De La Revelacion",
        ])
        .is_empty());
        assert!(keys(&[
            "El Exorcista",
            "El Exorcista Creyentes",
            "El Exorcista Del Papa",
        ])
        .is_empty());
    }

    #[test]
    fn refuses_duplicate_files_of_a_single_movie() {
        assert!(keys(&["Avatar Fuego Y Ceniza", "Avatar Fuego Y Ceniza"]).is_empty());
    }

    #[test]
    fn accepts_a_pair_when_only_the_sequel_is_numbered() {
        let candidates = group_saga_candidates(&["Deadpool", "Deadpool 2"]);
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].members.len(), 2);
    }
}
