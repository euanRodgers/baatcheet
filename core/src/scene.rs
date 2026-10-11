//! Playing a scene: other people's lines play in turn, and on the learner's
//! turns they pick what to say. It's practice, so it earns XP but never moves
//! a phrase between boxes.

use crate::cards::{Card, scene_turn};
use crate::content::{Content, Scene};
use crate::date::Date;
use crate::progress::{Progress, XP_RIGHT, XP_WRONG};
use crate::rng::Rng;

#[derive(Clone, Debug)]
pub struct ScenePlay {
    pub scene: Scene,
    /// Lines shown so far. The last one is the current line.
    pub shown: usize,
    /// Options for the current line, when it's the learner's turn.
    pub turn: Option<Card>,
    /// The option picked on the current turn, once answered.
    pub picked: Option<usize>,
    pub turns: u32,
    pub right: u32,
    pub xp: u32,
    rng: Rng,
}

impl ScenePlay {
    pub fn start(content: &Content, scene: &Scene, seed: u64) -> ScenePlay {
        let mut play = ScenePlay {
            scene: scene.clone(),
            shown: 0,
            turn: None,
            picked: None,
            turns: 0,
            right: 0,
            xp: 0,
            rng: Rng::new(seed),
        };
        play.next(content);
        play
    }

    /// Show the next line. Returns false at the end of the scene.
    pub fn next(&mut self, content: &Content) -> bool {
        if self.shown >= self.scene.lines.len() {
            return false;
        }
        self.shown += 1;
        self.picked = None;
        let line = &self.scene.lines[self.shown - 1];
        self.turn = match content.phrase(&line.phrase) {
            Some(p) if line.is_yours() => Some(scene_turn(content, p, &mut self.rng)),
            _ => None,
        };
        true
    }

    /// The learner picked an option on their turn.
    pub fn answer(&mut self, choice: usize, progress: &mut Progress, today: Date) -> bool {
        let Some(card) = &self.turn else { return false };
        if self.picked.is_some() {
            return false;
        }
        let right = card.right_option() == Some(choice);
        self.picked = Some(choice);
        self.turns += 1;
        let xp = if right { XP_RIGHT } else { XP_WRONG };
        if right {
            self.right += 1;
        }
        self.xp += xp;
        progress.xp += xp;
        progress.practised(today);
        right
    }

    pub fn is_last(&self) -> bool {
        self.shown >= self.scene.lines.len()
    }

    /// Whether the current line is waiting for an answer.
    pub fn waiting(&self) -> bool {
        self.turn.is_some() && self.picked.is_none()
    }

    /// How many of the scene's phrases the learner has met.
    pub fn phrases_met(scene: &Scene, progress: &Progress) -> (usize, usize) {
        let mut ids: Vec<&str> = scene.lines.iter().map(|l| l.phrase.as_str()).collect();
        ids.sort_unstable();
        ids.dedup();
        let met = ids.iter().filter(|id| progress.phrases.contains_key(**id)).count();
        (met, ids.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::content::Line;
    use crate::test_content;

    fn scene() -> Scene {
        let line = |speaker: &str, phrase: &str| Line { speaker: speaker.into(), phrase: phrase.into() };
        Scene {
            id: "dinner".into(),
            title: "Dinner".into(),
            intro: "Everyone is eating.".into(),
            lines: vec![line("Aunty", "food-003"), line("you", "food-002"), line("you", "surv-001")],
        }
    }

    #[test]
    fn plays_through_turns_without_moving_boxes() {
        let c = test_content();
        let mut prog = Progress::default();
        let today = Date::parse("2026-10-11").unwrap();
        let mut play = ScenePlay::start(&c, &scene(), 1);
        assert_eq!(play.shown, 1);
        assert!(!play.waiting(), "the aunty's line needs no answer");
        assert!(play.next(&c));
        assert!(play.waiting());
        let card = play.turn.clone().unwrap();
        assert_eq!(card.choices[card.right_option().unwrap()].roman, "Bas, pet bhar gaya");
        assert_eq!(card.choices.len(), 4);
        assert!(play.answer(card.right_option().unwrap(), &mut prog, today));
        assert!(!play.answer(0, &mut prog, today), "can't answer twice");
        assert!(play.next(&c));
        let wrong = (play.turn.as_ref().unwrap().right_option().unwrap() + 1) % 4;
        assert!(!play.answer(wrong, &mut prog, today));
        assert!(play.is_last());
        assert!(!play.next(&c));
        assert_eq!((play.turns, play.right, play.xp), (2, 1, XP_RIGHT + XP_WRONG));
        assert_eq!(prog.xp, XP_RIGHT + XP_WRONG);
        assert!(prog.phrases.is_empty(), "scenes are practice only");
    }

    #[test]
    fn counts_phrases_met() {
        let mut prog = Progress::default();
        prog.phrases.insert("food-002".into(), crate::progress::PhraseProgress::met(Date::parse("2026-10-11").unwrap()));
        assert_eq!(ScenePlay::phrases_met(&scene(), &prog), (1, 3));
    }
}
