//! The phrase content, as stored in `phrases.json`.

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Core,
    Later,
}

/// Who says the phrase. Euan only produces `Say` and `Both` phrases.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Say,
    Hear,
    Both,
}

impl Direction {
    pub fn is_said(self) -> bool {
        matches!(self, Direction::Say | Direction::Both)
    }
    pub fn is_heard(self) -> bool {
        matches!(self, Direction::Hear | Direction::Both)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Draft,
    Approved,
    AudioOk,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Phrase {
    pub id: String,
    pub pack: String,
    pub priority: Priority,
    pub direction: Direction,
    #[serde(default)]
    pub context: String,
    pub roman: String,
    pub deva: String,
    /// For gendered phrases this names the listener, e.g. "How are you? (to a woman)".
    pub english: String,
    #[serde(default)]
    pub notes: String,
    pub tiles: Vec<String>,
    pub deva_tiles: Vec<String>,
    #[serde(default)]
    pub replies: Vec<String>,
    pub status: Status,
    /// Set by `build-content` when `audio/{id}.mp3` exists.
    #[serde(default)]
    pub audio: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Content {
    pub version: u32,
    pub phrases: Vec<Phrase>,
    /// Names of word clips that exist in `audio/w/`, from [`word_clip_name`].
    #[serde(default)]
    pub word_audio: Vec<String>,
}

impl Content {
    pub fn from_json(s: &str) -> Result<Content, serde_json::Error> {
        serde_json::from_str(s)
    }

    pub fn phrase(&self, id: &str) -> Option<&Phrase> {
        self.phrases.iter().find(|p| p.id == id)
    }

    pub fn in_pack<'a>(&'a self, pack: &'a str) -> impl Iterator<Item = &'a Phrase> + 'a {
        self.phrases.iter().filter(move |p| p.pack == pack)
    }

    pub fn has_word_audio(&self, deva_word: &str) -> bool {
        let name = word_clip_name(deva_word);
        self.word_audio.contains(&name)
    }
}

pub struct Pack {
    pub id: &'static str,
    pub name: &'static str,
    pub badge: &'static str,
}

/// Packs in teaching order.
pub const PACKS: [Pack; 7] = [
    Pack { id: "greet", name: "Greetings & basics", badge: "Namaste!" },
    Pack { id: "surv", name: "Learner survival", badge: "Getting by" },
    Pack { id: "family", name: "Family & kinship", badge: "Knows the whole family" },
    Pack { id: "food", name: "Food & meals", badge: "Second helpings" },
    Pack { id: "talk", name: "Small talk", badge: "Chatterbox" },
    Pack { id: "warm", name: "Compliments & warmth", badge: "Charmer" },
    Pack { id: "occ", name: "Occasions", badge: "Festival ready" },
];

pub fn pack_order(pack: &str) -> usize {
    PACKS.iter().position(|p| p.id == pack).unwrap_or(PACKS.len())
}

pub fn pack_name(pack: &str) -> &'static str {
    PACKS.iter().find(|p| p.id == pack).map(|p| p.name).unwrap_or("Other")
}

const PUNCT: &[char] = &['?', '!', '.', ',', '।', ';', ':', '"', '\'', '(', ')', '…'];

/// Lowercased tile text with punctuation removed, for comparing answers.
pub fn normalize_word(s: &str) -> String {
    s.trim().trim_matches(PUNCT).trim().to_lowercase()
}

/// `roman` with punctuation removed, split into words, for checking tiles.
pub fn roman_words(s: &str) -> Vec<String> {
    s.split_whitespace().map(normalize_word).filter(|w| !w.is_empty()).collect()
}

/// File stem for a word clip: FNV-1a hash of the Devanagari word without punctuation.
/// Shared by `gen-audio` (which writes the files) and the app (which plays them),
/// so a word like है is generated once and reused by every phrase.
pub fn word_clip_name(deva_word: &str) -> String {
    let word = normalize_word(deva_word);
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in word.bytes() {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{hash:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes() {
        assert_eq!(normalize_word("hain?"), "hain");
        assert_eq!(normalize_word("Bas,"), "bas");
        assert_eq!(roman_words("Bas, pet bhar gaya"), ["bas", "pet", "bhar", "gaya"]);
    }

    #[test]
    fn clip_names_ignore_punctuation() {
        assert_eq!(word_clip_name("हैं?"), word_clip_name("हैं"));
        assert_ne!(word_clip_name("है"), word_clip_name("हैं"));
        assert_eq!(word_clip_name("है").len(), 16);
    }
}
