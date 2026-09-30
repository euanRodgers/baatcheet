//! Building a session and stepping through it.

use crate::cards::{Card, CardKind, exercise_for, make_card};
use crate::content::{Content, Phrase, pack_order};
use crate::date::Date;
use crate::progress::{Outcome, PhraseProgress, Progress, XP_MEET, XP_RIGHT, XP_WRONG, stats};
use crate::rng::Rng;
use std::collections::BTreeSet;

pub const MAX_REVIEWS: usize = 20;
/// New phrases per session.
pub const MAX_NEW: usize = 5;
/// New phrases per day, so "Keep going" can't race through the whole list.
pub const MAX_NEW_PER_DAY: usize = 10;
/// No new phrases while more reviews than this are due.
pub const BACKLOG_LIMIT: usize = 15;
/// Stop introducing new phrases after this long.
pub const NEW_CUTOFF_MS: f64 = 8.0 * 60_000.0;
/// A wrong card comes back this many cards later.
pub const RETRY_GAP: usize = 3;
/// Cards in a bonus round.
pub const BONUS_CARDS: usize = 10;

#[derive(Clone, Debug)]
pub struct Session {
    pub cards: Vec<Card>,
    pub pos: usize,
    pub xp: u32,
    /// Graded cards seen for the first time this session (not retries).
    pub first_attempts: u32,
    pub right_first_time: u32,
    /// Graded cards, including retries.
    pub graded: u32,
    /// Phrases whose box changed, with the new box, in the order first touched.
    pub moved: Vec<(String, u8)>,
    pub can_use_start: usize,
    pub understand_start: usize,
    /// A bonus round: practice only, so boxes and due dates don't change.
    pub bonus: bool,
    missed: BTreeSet<String>,
    started_ms: f64,
    rng: Rng,
}

/// What the answer sheet shows after a graded card.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub right: bool,
    pub xp: u32,
    /// The card will come back later in this session.
    pub requeued: bool,
}

impl Session {
    /// Today's session: due reviews first (oldest overdue first), then new
    /// phrases from the highest-priority unfinished pack, within the limits above.
    pub fn build(content: &Content, progress: &Progress, today: Date, now_ms: f64, seed: u64) -> Session {
        let mut rng = Rng::new(seed);

        let mut reviews = with_progress(content, progress);
        reviews.retain(|(_, pp)| pp.due <= today);
        let backlog = reviews.len();
        reviews.truncate(MAX_REVIEWS);
        let mut cards = review_cards(content, &reviews, &mut rng);

        let slots = if backlog > BACKLOG_LIMIT {
            0
        } else {
            MAX_NEW
                .min(MAX_NEW_PER_DAY.saturating_sub(progress.met_on(today)))
                .min(MAX_REVIEWS.saturating_sub(cards.len()) / 2)
        };
        let mut fresh: Vec<(usize, &Phrase)> = content
            .phrases
            .iter()
            .enumerate()
            .filter(|(_, p)| !progress.phrases.contains_key(&p.id))
            .collect();
        fresh.sort_by_key(|(i, p)| (pack_order(&p.pack), p.priority, *i));
        let fresh: Vec<&Phrase> = fresh.into_iter().take(slots).map(|(_, p)| p).collect();

        // Meet A, Meet B, Listen A, Meet C, Listen B, ... Listen last.
        let intro = |kind, p, rng: &mut Rng| Card { intro: true, ..make_card(content, p, kind, rng) };
        for (i, p) in fresh.iter().enumerate() {
            cards.push(intro(CardKind::Meet, p, &mut rng));
            if i > 0 {
                cards.push(intro(CardKind::ListenMeaning, fresh[i - 1], &mut rng));
            }
        }
        if let Some(last) = fresh.last() {
            cards.push(intro(CardKind::ListenMeaning, last, &mut rng));
        }

        Session::new(content, progress, cards, now_ms, rng, false)
    }

    /// Extra practice when nothing is due: the phrases due soonest. It earns XP
    /// but doesn't move boxes, so practising early can't skip the spacing.
    pub fn build_bonus(content: &Content, progress: &Progress, now_ms: f64, seed: u64) -> Session {
        let mut rng = Rng::new(seed);
        let mut reviews = with_progress(content, progress);
        reviews.truncate(BONUS_CARDS);
        let cards = review_cards(content, &reviews, &mut rng);
        Session::new(content, progress, cards, now_ms, rng, true)
    }

    fn new(content: &Content, progress: &Progress, cards: Vec<Card>, now_ms: f64, rng: Rng, bonus: bool) -> Session {
        let s = stats(content, progress);
        Session {
            cards,
            pos: 0,
            xp: 0,
            first_attempts: 0,
            right_first_time: 0,
            graded: 0,
            moved: vec![],
            can_use_start: s.can_use,
            understand_start: s.understand,
            bonus,
            missed: BTreeSet::new(),
            started_ms: now_ms,
            rng,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    pub fn current(&self) -> Option<&Card> {
        self.cards.get(self.pos)
    }

    pub fn is_done(&self) -> bool {
        self.pos >= self.cards.len()
    }

    /// The learner tapped "Got it" on a Meet card.
    pub fn meet(&mut self, progress: &mut Progress, today: Date) {
        let Some(card) = self.current() else { return };
        let id = card.phrase_id.clone();
        progress.phrases.entry(id.clone()).or_insert_with(|| PhraseProgress::met(today));
        progress.practised(today);
        progress.xp += XP_MEET;
        self.xp += XP_MEET;
        self.note_move(&id, 0);
    }

    /// Grade the current card. Updates progress straight away, so quitting
    /// mid-session keeps everything answered so far.
    pub fn grade(&mut self, outcome: Outcome, progress: &mut Progress, today: Date) -> Feedback {
        let Some(card) = self.current().cloned() else {
            return Feedback { right: false, xp: 0, requeued: false };
        };
        let id = card.phrase_id.clone();
        let already_missed = self.missed.contains(&id);
        if !self.bonus {
            let entry = progress.phrases.entry(id.clone()).or_insert_with(|| PhraseProgress::met(today));
            entry.apply(outcome, today, already_missed);
            let new_box = entry.box_;
            self.note_move(&id, new_box);
        }
        progress.practised(today);
        self.graded += 1;
        if !card.retry {
            self.first_attempts += 1;
        }

        let right = outcome != Outcome::Wrong;
        let xp = if outcome == Outcome::Right { XP_RIGHT } else { XP_WRONG };
        progress.xp += xp;
        self.xp += xp;
        if outcome == Outcome::Right && !already_missed {
            self.right_first_time += 1;
        }

        let mut requeued = false;
        if outcome == Outcome::Wrong && !already_missed {
            self.missed.insert(id);
            // Shuffle the options so the position isn't memorised.
            let again = Card { retry: true, ..card.reshuffled(&mut self.rng) };
            let at = (self.pos + RETRY_GAP).min(self.cards.len());
            self.cards.insert(at, again);
            requeued = true;
        }
        Feedback { right, xp, requeued }
    }

    /// Move to the next card. After eight minutes, cards introducing new phrases
    /// are skipped (unless the phrase was already met, so its first Listen still shows).
    pub fn advance(&mut self, progress: &Progress, now_ms: f64) {
        self.pos += 1;
        if now_ms - self.started_ms > NEW_CUTOFF_MS {
            while let Some(card) = self.cards.get(self.pos) {
                let unmet = !progress.phrases.contains_key(&card.phrase_id);
                if card.intro && (card.kind == CardKind::Meet || unmet) {
                    self.pos += 1;
                } else {
                    break;
                }
            }
        }
    }

    fn note_move(&mut self, id: &str, new_box: u8) {
        match self.moved.iter_mut().find(|(m, _)| m == id) {
            Some(entry) => entry.1 = new_box,
            None => self.moved.push((id.to_string(), new_box)),
        }
    }
}

/// Phrases the learner has met, soonest due first.
fn with_progress<'a>(content: &'a Content, progress: &'a Progress) -> Vec<(&'a Phrase, &'a PhraseProgress)> {
    let mut out: Vec<_> = content
        .phrases
        .iter()
        .filter_map(|p| progress.phrases.get(&p.id).map(|pp| (p, pp)))
        .collect();
    out.sort_by_key(|(_, pp)| (pp.due, pp.box_));
    out
}

fn review_cards(content: &Content, reviews: &[(&Phrase, &PhraseProgress)], rng: &mut Rng) -> Vec<Card> {
    reviews.iter().map(|(p, pp)| make_card(content, p, exercise_for(p, Some(pp)), rng)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_content;

    fn today() -> Date {
        Date::parse("2026-09-30").unwrap()
    }

    #[test]
    fn first_session_introduces_new_phrases_in_pack_order() {
        let c = test_content();
        let s = Session::build(&c, &Progress::default(), today(), 0.0, 1);
        let kinds: Vec<CardKind> = s.cards.iter().map(|c| c.kind).collect();
        assert_eq!(kinds[0], CardKind::Meet);
        assert_eq!(s.cards[0].phrase_id, "greet-001");
        assert_eq!(s.cards.iter().filter(|c| c.kind == CardKind::Meet).count(), 5);
        assert_eq!(s.cards.iter().filter(|c| c.kind == CardKind::ListenMeaning).count(), 5);
        // Each Listen comes after its Meet.
        for (i, card) in s.cards.iter().enumerate().filter(|(_, c)| c.kind == CardKind::ListenMeaning) {
            assert!(s.cards[..i].iter().any(|m| m.kind == CardKind::Meet && m.phrase_id == card.phrase_id));
        }
    }

    #[test]
    fn reviews_come_first_and_backlog_blocks_new() {
        let c = test_content();
        let mut prog = Progress::default();
        let old = today().add_days(-5);
        prog.phrases.insert("food-002".into(), PhraseProgress { box_: 3, ..PhraseProgress::met(old) });
        let s = Session::build(&c, &prog, today(), 0.0, 1);
        assert_eq!(s.cards[0].phrase_id, "food-002");
        assert_eq!(s.cards[0].kind, CardKind::Build);

        // Test content has fewer phrases than the backlog limit, so fake a big backlog.
        let big = Content { phrases: (0..20).flat_map(|_| c.phrases.clone()).enumerate().map(|(i, mut p)| { p.id = format!("x-{i}"); p }).collect(), ..c.clone() };
        let mut big_prog = Progress::default();
        for p in big.phrases.iter().take(16) {
            big_prog.phrases.insert(p.id.clone(), PhraseProgress { box_: 1, ..PhraseProgress::met(old) });
        }
        let s = Session::build(&big, &big_prog, today(), 0.0, 1);
        assert!(s.cards.iter().all(|c| c.kind != CardKind::Meet), "no new phrases with 16 due");
        assert_eq!(s.cards.len(), 16);
    }

    #[test]
    fn wrong_answer_comes_back_and_quitting_keeps_progress() {
        let c = test_content();
        let mut prog = Progress::default();
        let mut s = Session::build(&c, &prog, today(), 0.0, 1);
        s.meet(&mut prog, today());
        s.advance(&prog, 0.0);
        s.meet(&mut prog, today());
        s.advance(&prog, 0.0);
        // Third card is the Listen for the first phrase.
        assert_eq!(s.current().unwrap().kind, CardKind::ListenMeaning);
        let before = s.cards.len();
        let fb = s.grade(Outcome::Wrong, &mut prog, today());
        assert!(fb.requeued && !fb.right);
        assert_eq!(s.cards.len(), before + 1);
        let retry = &s.cards[s.pos + RETRY_GAP];
        assert!(retry.retry);
        assert_eq!(retry.choices[retry.right_option().unwrap()].english, "Hello");
        assert_eq!(prog.box_of("greet-001"), Some(0));
        assert_eq!(prog.xp, XP_MEET * 2 + XP_WRONG);
        assert_eq!(prog.days.len(), 1);
    }

    #[test]
    fn new_phrases_stop_after_eight_minutes() {
        let c = test_content();
        let mut prog = Progress::default();
        let mut s = Session::build(&c, &prog, today(), 0.0, 1);
        s.meet(&mut prog, today());
        s.advance(&prog, NEW_CUTOFF_MS + 1.0);
        // The second Meet is skipped; the next card is greet-001's Listen.
        let card = s.current().unwrap();
        assert_eq!((card.kind, card.phrase_id.as_str()), (CardKind::ListenMeaning, "greet-001"));
        s.advance(&prog, NEW_CUTOFF_MS + 1.0);
        assert!(s.is_done(), "remaining intro cards are for unmet phrases");
    }

    #[test]
    fn new_phrases_are_capped_per_day() {
        let c = test_content();
        let big = Content { phrases: (0..3).flat_map(|_| c.phrases.clone()).enumerate().map(|(i, mut p)| { p.id = format!("x-{i}"); p }).collect(), ..c.clone() };
        let mut prog = Progress::default();
        // Eight met earlier today and not due again until tomorrow.
        for p in big.phrases.iter().take(8) {
            prog.phrases.insert(p.id.clone(), PhraseProgress { box_: 1, due: today().add_days(1), ..PhraseProgress::met(today()) });
        }
        let meets = |s: &Session| s.cards.iter().filter(|c| c.kind == CardKind::Meet).count();
        assert_eq!(meets(&Session::build(&big, &prog, today(), 0.0, 1)), MAX_NEW_PER_DAY - 8);
        // Tomorrow the allowance is back (the eight are due, but that's under the backlog limit).
        assert_eq!(meets(&Session::build(&big, &prog, today().add_days(1), 0.0, 1)), MAX_NEW);
    }

    #[test]
    fn bonus_round_earns_xp_without_moving_boxes() {
        let c = test_content();
        let mut prog = Progress::default();
        let later = today().add_days(5);
        for p in &c.phrases {
            prog.phrases.insert(p.id.clone(), PhraseProgress { box_: 3, due: later, ..PhraseProgress::met(today().add_days(-10)) });
        }
        let before = prog.phrases.clone();
        let mut s = Session::build_bonus(&c, &prog, 0.0, 1);
        assert_eq!(s.cards.len(), BONUS_CARDS.min(c.phrases.len()));
        while !s.is_done() {
            s.grade(Outcome::Right, &mut prog, today());
            s.advance(&prog, 0.0);
        }
        assert_eq!(prog.phrases, before);
        assert!(prog.xp > 0);
        assert!(s.moved.is_empty());
    }
}

