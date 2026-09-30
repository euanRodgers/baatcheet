//! What Euan has learned, saved on the device.

use crate::content::{Content, PACKS};
use crate::date::Date;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Days until a phrase is due again, indexed by Leitner box.
pub const INTERVALS: [i32; 6] = [0, 1, 3, 7, 14, 30];
pub const MAX_BOX: u8 = 5;
/// A `say` phrase at this box or above counts as "can use".
pub const CAN_USE_BOX: u8 = 4;
/// A `hear` phrase at this box or above counts as "understand".
pub const UNDERSTAND_BOX: u8 = 2;

pub const XP_RIGHT: u32 = 10;
pub const XP_WRONG: u32 = 5;
pub const XP_MEET: u32 = 5;
pub const XP_USED_FOR_REAL: u32 = 25;
/// Real uses needed for the "Survived dinner" badge.
pub const DINNER_USES: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Right,
    /// Only from self-graded Recall.
    Nearly,
    Wrong,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PhraseProgress {
    #[serde(rename = "box")]
    pub box_: u8,
    pub due: Date,
    #[serde(default)]
    pub seen: u32,
    #[serde(default)]
    pub wrong: u32,
    /// The day Euan first met the phrase. Used for the daily limit on new phrases.
    #[serde(default)]
    pub met: Option<Date>,
}

impl PhraseProgress {
    /// A phrase Euan has just met. It's due again in the same session.
    pub fn met(today: Date) -> PhraseProgress {
        PhraseProgress { box_: 0, due: today, seen: 0, wrong: 0, met: Some(today) }
    }

    /// Move the phrase between boxes. `already_missed` is true when this phrase
    /// was answered wrongly earlier in the same session: it doesn't move up for
    /// getting it right the second time, and isn't pushed down twice.
    pub fn apply(&mut self, outcome: Outcome, today: Date, already_missed: bool) {
        self.seen += 1;
        match outcome {
            Outcome::Right if !already_missed => self.box_ = (self.box_ + 1).min(MAX_BOX),
            Outcome::Wrong if !already_missed => {
                self.wrong += 1;
                // Down two boxes, never below 1. A brand new phrase stays in 0.
                if self.box_ > 0 {
                    self.box_ = self.box_.saturating_sub(2).max(1);
                }
            }
            _ => {}
        }
        self.due = today.add_days(INTERVALS[self.box_.min(MAX_BOX) as usize]);
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Progress {
    pub v: u32,
    pub xp: u32,
    pub days: BTreeSet<Date>,
    pub phrases: BTreeMap<String, PhraseProgress>,
    /// Times each phrase was used at a real family meal. Kept apart from the
    /// schedule so marking a phrase doesn't skip its introduction.
    pub used: BTreeMap<String, u32>,
}

impl Default for Progress {
    fn default() -> Self {
        Progress { v: 1, xp: 0, days: BTreeSet::new(), phrases: BTreeMap::new(), used: BTreeMap::new() }
    }
}

impl Progress {
    /// Parse and check saved progress. Rejects anything the app couldn't use,
    /// such as a box above 5, rather than crashing on it later.
    pub fn from_json(s: &str) -> Result<Progress, String> {
        let p: Progress = serde_json::from_str(s).map_err(|e| e.to_string())?;
        if p.v != 1 {
            return Err(format!("unknown progress version {}", p.v));
        }
        if let Some((id, _)) = p.phrases.iter().find(|(_, pp)| pp.box_ > MAX_BOX) {
            return Err(format!("{id} is in box {}, but the highest box is {MAX_BOX}", p.phrases[id].box_));
        }
        Ok(p)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string(self).expect("progress always serialises")
    }

    pub fn box_of(&self, id: &str) -> Option<u8> {
        self.phrases.get(id).map(|p| p.box_)
    }

    pub fn practised(&mut self, today: Date) {
        self.days.insert(today);
    }

    pub fn days_in_month(&self, today: Date) -> usize {
        let (y, m, _) = today.ymd();
        self.days.iter().filter(|d| { let (dy, dm, _) = d.ymd(); dy == y && dm == m }).count()
    }

    /// Phrases first met today.
    pub fn met_on(&self, today: Date) -> usize {
        self.phrases.values().filter(|p| p.met == Some(today)).count()
    }

    pub fn used_for_real(&self, id: &str) -> bool {
        self.used.get(id).is_some_and(|&n| n > 0)
    }

    pub fn used_for_real_total(&self) -> u32 {
        self.used.values().sum()
    }

    /// Record (or undo) using a phrase at a real family meal. Returns whether it's now marked.
    pub fn toggle_used_for_real(&mut self, id: &str) -> bool {
        if self.used.remove(id).is_some() {
            self.xp = self.xp.saturating_sub(XP_USED_FOR_REAL);
            false
        } else {
            self.used.insert(id.to_string(), 1);
            self.xp += XP_USED_FOR_REAL;
            true
        }
    }
}

/// The numbers on the home screen.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub can_use: usize,
    pub say_total: usize,
    pub understand: usize,
    pub hear_total: usize,
}

pub fn stats(content: &Content, progress: &Progress) -> Stats {
    let mut s = Stats { can_use: 0, say_total: 0, understand: 0, hear_total: 0 };
    for p in &content.phrases {
        let b = progress.box_of(&p.id);
        if p.direction.is_said() {
            s.say_total += 1;
            if b.is_some_and(|b| b >= CAN_USE_BOX) {
                s.can_use += 1;
            }
        }
        if p.direction.is_heard() {
            s.hear_total += 1;
            if b.is_some_and(|b| b >= UNDERSTAND_BOX) {
                s.understand += 1;
            }
        }
    }
    s
}

/// Whether a phrase counts as learned for pack progress.
pub fn is_learned(content: &Content, progress: &Progress, id: &str) -> bool {
    let Some(p) = content.phrase(id) else { return false };
    let Some(b) = progress.box_of(id) else { return false };
    if p.direction.is_said() { b >= CAN_USE_BOX } else { b >= UNDERSTAND_BOX }
}

/// (learned, total) for a pack.
pub fn pack_progress(content: &Content, progress: &Progress, pack: &str) -> (usize, usize) {
    let total = content.in_pack(pack).count();
    let learned = content.in_pack(pack).filter(|p| is_learned(content, progress, &p.id)).count();
    (learned, total)
}

pub struct Badge {
    pub name: &'static str,
    pub earned: bool,
}

pub fn badges(content: &Content, progress: &Progress) -> Vec<Badge> {
    let mut out = vec![
        Badge { name: "First session", earned: !progress.days.is_empty() },
        Badge { name: "Survived dinner", earned: progress.used_for_real_total() >= DINNER_USES },
    ];
    for pack in &PACKS {
        let (learned, total) = pack_progress(content, progress, pack.id);
        if total > 0 {
            out.push(Badge { name: pack.badge, earned: learned == total });
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn day(s: &str) -> Date {
        Date::parse(s).unwrap()
    }

    #[test]
    fn right_moves_up_and_sets_interval() {
        let mut p = PhraseProgress::met(day("2026-09-30"));
        p.apply(Outcome::Right, day("2026-09-30"), false);
        assert_eq!((p.box_, p.due), (1, day("2026-10-01")));
        p.apply(Outcome::Right, day("2026-10-01"), false);
        assert_eq!((p.box_, p.due), (2, day("2026-10-04")));
    }

    #[test]
    fn wrong_drops_two_boxes_but_not_below_one() {
        let mut p = PhraseProgress { box_: 5, ..PhraseProgress::met(day("2026-09-30")) };
        p.apply(Outcome::Wrong, day("2026-09-30"), false);
        assert_eq!(p.box_, 3);
        p.apply(Outcome::Wrong, day("2026-09-30"), false);
        assert_eq!(p.box_, 1);
        p.apply(Outcome::Wrong, day("2026-09-30"), false);
        assert_eq!(p.box_, 1);
        assert_eq!(p.wrong, 3);
    }

    #[test]
    fn new_phrase_wrong_stays_in_box_zero() {
        let mut p = PhraseProgress::met(day("2026-09-30"));
        p.apply(Outcome::Wrong, day("2026-09-30"), false);
        assert_eq!((p.box_, p.due), (0, day("2026-09-30")));
    }

    #[test]
    fn retry_in_same_session_does_not_move() {
        let mut p = PhraseProgress { box_: 3, ..PhraseProgress::met(day("2026-09-30")) };
        p.apply(Outcome::Wrong, day("2026-09-30"), false);
        p.apply(Outcome::Right, day("2026-09-30"), true);
        assert_eq!(p.box_, 1);
    }

    #[test]
    fn nearly_stays() {
        let mut p = PhraseProgress { box_: 4, ..PhraseProgress::met(day("2026-09-30")) };
        p.apply(Outcome::Nearly, day("2026-09-30"), false);
        assert_eq!((p.box_, p.due), (4, day("2026-10-14")));
    }

    #[test]
    fn rejects_impossible_progress() {
        assert!(Progress::from_json(r#"{"v":1,"phrases":{"x":{"box":9,"due":"2026-09-30"}}}"#).is_err());
        assert!(Progress::from_json(r#"{"v":2}"#).is_err());
        assert!(Progress::from_json("not json").is_err());
        // Missing fields fall back to defaults.
        assert_eq!(Progress::from_json(r#"{"v":1}"#).unwrap(), Progress::default());
    }

    #[test]
    fn used_for_real_does_not_touch_schedule() {
        let mut prog = Progress::default();
        assert!(prog.toggle_used_for_real("occ-003"));
        assert!(prog.phrases.is_empty());
        assert_eq!((prog.used_for_real_total(), prog.xp), (1, XP_USED_FOR_REAL));
        assert!(!prog.toggle_used_for_real("occ-003"));
        assert_eq!(prog.xp, 0);
    }

    #[test]
    fn progress_json_matches_plan() {
        let mut prog = Progress::default();
        prog.phrases.insert("greet-003".into(), PhraseProgress::met(day("2026-10-02")));
        prog.practised(day("2026-09-30"));
        let json = prog.to_json();
        assert!(json.contains(r#""box":0"#));
        assert!(json.contains(r#""met":"2026-10-02""#));
        assert!(json.contains(r#""days":["2026-09-30"]"#));
        assert_eq!(Progress::from_json(&json).unwrap(), prog);
    }
}
