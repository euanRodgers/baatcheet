//! Turning a phrase into an exercise card, including the wrong options.

use crate::content::{Content, Direction, Phrase, normalize_word, roman_words};
use crate::progress::PhraseProgress;
use crate::rng::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardKind {
    /// First sight of a phrase. Not graded.
    Meet,
    /// Hear it, pick the English.
    ListenMeaning,
    /// Hear it, pick the romanised phrase from lookalikes.
    PickHindi,
    /// English prompt, arrange word tiles.
    Build,
    /// English prompt, self-graded.
    Recall,
    /// Hear what someone says to you, pick the right reply.
    Respond,
}

impl CardKind {
    /// Cards where the learner is listening to someone else. Only these show the
    /// phrase's scene ("An aunty holds out the serving spoon").
    pub fn is_listening(self) -> bool {
        matches!(self, CardKind::Meet | CardKind::ListenMeaning | CardKind::PickHindi | CardKind::Respond)
    }
}

/// One answer option, or one word tile on a Build card.
#[derive(Clone, Debug, PartialEq)]
pub struct Choice {
    pub roman: String,
    pub deva: String,
    pub english: String,
}

/// What counts as right on a card.
#[derive(Clone, Debug, PartialEq)]
pub enum Answer {
    /// Meet and Recall: nothing to pick.
    None,
    /// Index of the right option.
    One(usize),
    /// Build: tile indices in the right order.
    Tiles(Vec<usize>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Card {
    pub phrase_id: String,
    pub kind: CardKind,
    /// Options (already shuffled), or the tiles for Build.
    pub choices: Vec<Choice>,
    pub answer: Answer,
    /// For Respond: the phrase id of the right reply.
    pub reply_id: Option<String>,
    /// Shown again after a wrong answer in the same session.
    pub retry: bool,
    /// Part of introducing a new phrase (the Meet card and its first Listen).
    pub intro: bool,
}

impl Card {
    fn plain(phrase_id: &str, kind: CardKind) -> Card {
        Card {
            phrase_id: phrase_id.to_string(),
            kind,
            choices: vec![],
            answer: Answer::None,
            reply_id: None,
            retry: false,
            intro: false,
        }
    }

    /// Index of the right option on a multiple-choice card.
    pub fn right_option(&self) -> Option<usize> {
        match self.answer {
            Answer::One(i) => Some(i),
            _ => None,
        }
    }

    /// Number of tiles in the right answer on a Build card.
    pub fn tiles_needed(&self) -> usize {
        match &self.answer {
            Answer::Tiles(t) => t.len(),
            _ => 0,
        }
    }

    /// Check a Build answer: the chosen tiles must spell the phrase.
    /// Compared by text, so two identical tiles are interchangeable.
    pub fn build_is_right(&self, picked: &[usize]) -> bool {
        let Answer::Tiles(order) = &self.answer else { return false };
        let words = |idx: &[usize]| -> Vec<String> {
            idx.iter().filter_map(|&i| self.choices.get(i)).map(|c| normalize_word(&c.roman)).collect()
        };
        words(order) == words(picked)
    }

    /// The same card with its options in a new order, for showing it again.
    pub fn reshuffled(&self, rng: &mut Rng) -> Card {
        let mut order: Vec<usize> = (0..self.choices.len()).collect();
        rng.shuffle(&mut order);
        // new_pos[old index] = where that option now sits.
        let mut new_pos = vec![0; order.len()];
        for (pos, &old) in order.iter().enumerate() {
            new_pos[old] = pos;
        }
        let answer = match &self.answer {
            Answer::None => Answer::None,
            Answer::One(i) => Answer::One(new_pos[*i]),
            Answer::Tiles(t) => Answer::Tiles(t.iter().map(|&i| new_pos[i]).collect()),
        };
        Card {
            choices: order.iter().map(|&o| self.choices[o].clone()).collect(),
            answer,
            ..self.clone()
        }
    }
}

/// Which exercise a phrase gets, from its box. The box picks the exercise, so a
/// phrase only reaches box 4 ("can use") by passing a Build card.
pub fn exercise_for(phrase: &Phrase, progress: Option<&PhraseProgress>) -> CardKind {
    let Some(p) = progress else { return CardKind::Meet };
    let alternate = p.seen % 2 == 1;
    let has_replies = !phrase.replies.is_empty();
    match phrase.direction {
        Direction::Hear => match p.box_ {
            0 | 1 => CardKind::ListenMeaning,
            _ if alternate && has_replies => CardKind::Respond,
            _ => CardKind::PickHindi,
        },
        Direction::Say | Direction::Both => {
            let can_build = phrase.tiles.len() >= 2;
            match p.box_ {
                0 | 1 => CardKind::ListenMeaning,
                // A "both" phrase is also said to the learner, so it can be the prompt on a Respond card.
                2 if alternate && has_replies && phrase.direction == Direction::Both => CardKind::Respond,
                2 => CardKind::PickHindi,
                3 if can_build => CardKind::Build,
                5 if can_build && !alternate => CardKind::Build,
                _ => CardKind::Recall,
            }
        }
    }
}

pub fn make_card(content: &Content, phrase: &Phrase, kind: CardKind, rng: &mut Rng) -> Card {
    let mut card = Card::plain(&phrase.id, kind);
    let same_pack = |c: &Phrase| usize::from(c.pack == phrase.pack) * 5;
    match kind {
        CardKind::Meet | CardKind::Recall => {}
        CardKind::ListenMeaning => {
            let others = pick_distractors(content, phrase, rng, |c| c.english != phrase.english, same_pack);
            fill_options(&mut card, phrase, others, rng);
        }
        CardKind::PickHindi => {
            let mine = roman_words(&phrase.roman);
            let others = pick_distractors(
                content,
                phrase,
                rng,
                |c| normalize_word(&c.roman) != normalize_word(&phrase.roman),
                |c| {
                    // Lookalikes: phrases sharing words, then the same length, then the same pack.
                    let theirs = roman_words(&c.roman);
                    let shared = theirs.iter().filter(|w| mine.contains(w)).count();
                    shared * 10 + usize::from(theirs.len() == mine.len()) * 3 + same_pack(c)
                },
            );
            fill_options(&mut card, phrase, others, rng);
        }
        CardKind::Respond => {
            let reply = phrase.replies.iter().find_map(|id| content.phrase(id));
            let Some(reply) = reply else {
                // No usable reply: fall back to a plain listening card.
                return make_card(content, phrase, CardKind::ListenMeaning, rng);
            };
            // Wrong replies come from other packs: within a pack (say, food) many
            // replies are plausible, and a right answer must never be marked wrong.
            let others = pick_distractors(
                content,
                reply,
                rng,
                |c| c.direction.is_said() && c.id != phrase.id && !phrase.replies.contains(&c.id),
                |c| usize::from(c.pack != phrase.pack) * 5,
            );
            fill_options(&mut card, reply, others, rng);
            card.reply_id = Some(reply.id.clone());
        }
        CardKind::Build => {
            let mut tiles: Vec<Choice> = phrase
                .tiles
                .iter()
                .zip(&phrase.deva_tiles)
                .map(|(r, d)| Choice { roman: r.clone(), deva: d.clone(), english: String::new() })
                .collect();
            let n = tiles.len();
            if let Some(extra) = distractor_tile(content, phrase, rng) {
                tiles.push(extra);
            }
            card.choices = tiles;
            card.answer = Answer::Tiles((0..n).collect());
            card = card.reshuffled(rng);
        }
    }
    card
}

fn choice_of(p: &Phrase) -> Choice {
    Choice { roman: p.roman.clone(), deva: p.deva.clone(), english: p.english.clone() }
}

fn fill_options(card: &mut Card, right: &Phrase, others: Vec<&Phrase>, rng: &mut Rng) {
    card.choices = std::iter::once(right).chain(others).map(choice_of).collect();
    card.answer = Answer::One(0);
    *card = card.reshuffled(rng);
}

/// Up to three other phrases that pass `keep`, best `score` first, with ties
/// broken at random.
fn pick_distractors<'a>(
    content: &'a Content,
    phrase: &Phrase,
    rng: &mut Rng,
    keep: impl Fn(&Phrase) -> bool,
    score: impl Fn(&Phrase) -> usize,
) -> Vec<&'a Phrase> {
    let mut ranked: Vec<(usize, u64, &Phrase)> = content
        .phrases
        .iter()
        .filter(|c| c.id != phrase.id && keep(c))
        .map(|c| (score(c), rng.next_u64(), c))
        .collect();
    ranked.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let mut out: Vec<&Phrase> = Vec::new();
    for (_, _, c) in ranked {
        // No two options with the same text.
        if out.iter().all(|o| o.english != c.english && o.roman != c.roman) {
            out.push(c);
        }
        if out.len() == 3 {
            break;
        }
    }
    out
}

/// A word from another phrase, preferably in the same pack, that isn't in this one.
fn distractor_tile(content: &Content, phrase: &Phrase, rng: &mut Rng) -> Option<Choice> {
    let mine: Vec<String> = phrase.tiles.iter().map(|t| normalize_word(t)).collect();
    let mut candidates: Vec<(bool, Choice)> = Vec::new();
    for other in content.phrases.iter().filter(|p| p.id != phrase.id) {
        for (r, d) in other.tiles.iter().zip(&other.deva_tiles) {
            let word = normalize_word(r);
            if !word.is_empty() && !mine.contains(&word) && !candidates.iter().any(|(_, c)| normalize_word(&c.roman) == word) {
                let roman = r.trim_matches(|c: char| c.is_ascii_punctuation()).to_string();
                let deva = d.trim_matches(|c: char| c.is_ascii_punctuation() || c == '।').to_string();
                candidates.push((other.pack == phrase.pack, Choice { roman, deva, english: String::new() }));
            }
        }
    }
    let same_pack: Vec<&Choice> = candidates.iter().filter(|(s, _)| *s).map(|(_, c)| c).collect();
    let pool: Vec<&Choice> = if same_pack.is_empty() { candidates.iter().map(|(_, c)| c).collect() } else { same_pack };
    if pool.is_empty() { None } else { Some(pool[rng.below(pool.len())].clone()) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content;

    fn prog(b: u8, seen: u32) -> PhraseProgress {
        PhraseProgress { box_: b, seen, ..PhraseProgress::met(crate::Date::parse("2026-09-30").unwrap()) }
    }

    #[test]
    fn box_picks_exercise() {
        let c = test_content();
        let say = c.phrase("greet-002").unwrap();
        assert_eq!(exercise_for(say, None), CardKind::Meet);
        assert_eq!(exercise_for(say, Some(&prog(1, 0))), CardKind::ListenMeaning);
        assert_eq!(exercise_for(say, Some(&prog(2, 0))), CardKind::PickHindi);
        assert_eq!(exercise_for(say, Some(&prog(2, 1))), CardKind::PickHindi, "say phrases never prompt Respond");
        assert_eq!(exercise_for(say, Some(&prog(3, 0))), CardKind::Build);
        assert_eq!(exercise_for(say, Some(&prog(4, 0))), CardKind::Recall);
        assert_eq!(exercise_for(say, Some(&prog(5, 0))), CardKind::Build);
        assert_eq!(exercise_for(say, Some(&prog(5, 1))), CardKind::Recall);

        let hear = c.phrase("food-003").unwrap();
        assert_eq!(exercise_for(hear, Some(&prog(3, 0))), CardKind::PickHindi);
        assert_eq!(exercise_for(hear, Some(&prog(3, 1))), CardKind::Respond);
        assert_eq!(exercise_for(hear, Some(&prog(5, 0))), CardKind::PickHindi, "hear phrases never get Build");

        let both = c.phrase("talk-001").unwrap();
        assert_eq!(exercise_for(both, Some(&prog(2, 1))), CardKind::Respond);
        assert_eq!(exercise_for(both, Some(&prog(3, 1))), CardKind::Build);
    }

    #[test]
    fn pick_hindi_prefers_lookalikes() {
        let c = test_content();
        let mut rng = Rng::new(7);
        let card = make_card(&c, c.phrase("greet-002").unwrap(), CardKind::PickHindi, &mut rng);
        assert_eq!(card.choices.len(), 4);
        assert_eq!(card.choices[card.right_option().unwrap()].roman, "Aap kaise hain?");
        assert!(card.choices.iter().any(|ch| ch.roman == "Aap kaisi hain?"), "the other gender form is a distractor");
    }

    #[test]
    fn build_answer_spells_phrase() {
        let c = test_content();
        for seed in 0..20 {
            let mut rng = Rng::new(seed);
            let card = make_card(&c, c.phrase("food-002").unwrap(), CardKind::Build, &mut rng);
            assert_eq!(card.choices.len(), 5, "four tiles plus one distractor");
            assert_eq!(card.tiles_needed(), 4);
            let Answer::Tiles(order) = card.answer.clone() else { panic!("build has tiles") };
            assert!(card.build_is_right(&order));
            let mut wrong = order.clone();
            wrong.swap(0, 1);
            assert!(!card.build_is_right(&wrong));
            let again = card.reshuffled(&mut rng);
            let Answer::Tiles(order2) = again.answer.clone() else { panic!() };
            assert!(again.build_is_right(&order2), "still right after reshuffling");
        }
    }

    #[test]
    fn respond_offers_the_reply_and_no_other_valid_one() {
        let c = test_content();
        for seed in 0..50 {
            let mut rng = Rng::new(seed);
            let card = make_card(&c, c.phrase("food-003").unwrap(), CardKind::Respond, &mut rng);
            assert_eq!(card.reply_id.as_deref(), Some("food-002"));
            assert_eq!(card.choices[card.right_option().unwrap()].roman, "Bas, pet bhar gaya");
            assert!(card.choices.iter().all(|ch| ch.roman != "Aur lijiye!"));
            // Other food phrases could be fair replies to "have some more", so none are offered as wrong.
            let food_options = card.choices.iter().filter(|ch| c.phrases.iter().any(|p| p.pack == "food" && p.roman == ch.roman)).count();
            assert_eq!(food_options, 1, "seed {seed}");
        }
    }
}
