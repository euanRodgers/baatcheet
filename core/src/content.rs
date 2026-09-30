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
    /// The version for a woman learner, when the words change with who's
    /// speaking or being spoken to (samajh gaya / samajh gayi).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub female: Option<Form>,
    /// True once [`Content::for_learner`] has swapped in the woman's version.
    #[serde(skip)]
    pub female_form: bool,
}

/// Another form of a phrase: its words, tiles and audio.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Form {
    pub roman: String,
    pub deva: String,
    pub english: String,
    /// Empty means "use the phrase's own notes".
    #[serde(default)]
    pub notes: String,
    pub tiles: Vec<String>,
    pub deva_tiles: Vec<String>,
    /// Set by `build-content` when `audio/{id}-f.mp3` exists.
    #[serde(default)]
    pub audio: bool,
}

/// Placeholder in phrases for the learner's own name.
pub const NAME: &str = "{name}";

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Gender {
    Man,
    Woman,
}

/// Who is learning: sets the name in phrases and which gendered forms to teach.
#[derive(Clone, Debug, PartialEq)]
pub struct Learner {
    pub name: String,
    pub gender: Gender,
}

impl Phrase {
    /// Whether the phrase contains the learner's name, so it can't have a recorded clip.
    pub fn is_personal(&self) -> bool {
        self.roman.contains(NAME)
    }

    /// File stem of this phrase's recorded clip in `audio/`.
    pub fn clip_name(&self) -> String {
        if self.female_form { format!("{}-f", self.id) } else { self.id.clone() }
    }
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

    /// The phrases as this learner should see them: the woman's forms for a
    /// woman, and their own name in place of `{name}`.
    pub fn for_learner(&self, learner: &Learner) -> Content {
        let mut out = self.clone();
        let name = learner.name.trim();
        for p in &mut out.phrases {
            if learner.gender == Gender::Woman
                && let Some(f) = p.female.take()
            {
                p.roman = f.roman;
                p.deva = f.deva;
                p.english = f.english;
                if !f.notes.is_empty() {
                    p.notes = f.notes;
                }
                p.tiles = f.tiles;
                p.deva_tiles = f.deva_tiles;
                p.audio = f.audio;
                p.female_form = true;
            }
            if p.is_personal() {
                // A recording can't say every name, so these use the device voice.
                p.audio = false;
                for text in [&mut p.roman, &mut p.deva, &mut p.english, &mut p.notes] {
                    *text = text.replace(NAME, name);
                }
                for tile in p.tiles.iter_mut().chain(p.deva_tiles.iter_mut()) {
                    *tile = tile.replace(NAME, name);
                }
            }
        }
        out
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

    fn phrase(id: &str, roman: &str, deva: &str) -> Phrase {
        Phrase {
            id: id.into(),
            pack: "surv".into(),
            priority: Priority::Core,
            direction: Direction::Say,
            context: String::new(),
            roman: roman.into(),
            deva: deva.into(),
            english: "x".into(),
            notes: "base".into(),
            tiles: roman.split_whitespace().map(String::from).collect(),
            deva_tiles: deva.split_whitespace().map(String::from).collect(),
            replies: vec![],
            status: Status::Draft,
            audio: true,
            female: None,
            female_form: false,
        }
    }

    #[test]
    fn adapts_to_the_learner() {
        let mut gaya = phrase("surv-008", "Haan, samajh gaya", "हाँ, समझ गया");
        gaya.female = Some(Form {
            roman: "Haan, samajh gayi".into(),
            deva: "हाँ, समझ गई".into(),
            english: "Yes, I understood".into(),
            notes: String::new(),
            tiles: vec!["Haan,".into(), "samajh".into(), "gayi".into()],
            deva_tiles: vec!["हाँ,".into(), "समझ".into(), "गई".into()],
            audio: false,
        });
        let naam = phrase("greet-012", "Mera naam {name} hai", "मेरा नाम {name} है");
        let content = Content { version: 1, phrases: vec![gaya, naam], word_audio: vec![] };

        let man = content.for_learner(&Learner { name: "Euan".into(), gender: Gender::Man });
        assert_eq!(man.phrases[0].roman, "Haan, samajh gaya");
        assert_eq!(man.phrases[0].clip_name(), "surv-008");
        assert_eq!(man.phrases[1].roman, "Mera naam Euan hai");
        assert_eq!(man.phrases[1].tiles[2], "Euan");
        assert!(!man.phrases[1].audio, "a clip can't say every name");

        let woman = content.for_learner(&Learner { name: "Priya".into(), gender: Gender::Woman });
        assert_eq!(woman.phrases[0].deva, "हाँ, समझ गई");
        assert_eq!(woman.phrases[0].notes, "base", "keeps the phrase's notes when the form has none");
        assert_eq!(woman.phrases[0].clip_name(), "surv-008-f");
        assert_eq!(woman.phrases[1].deva, "मेरा नाम Priya है");
    }

    #[test]
    fn clip_names_ignore_punctuation() {
        assert_eq!(word_clip_name("हैं?"), word_clip_name("हैं"));
        assert_ne!(word_clip_name("है"), word_clip_name("हैं"));
        assert_eq!(word_clip_name("है").len(), 16);
    }
}
