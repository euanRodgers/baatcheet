//! Checking a typed answer, forgivingly.
//!
//! Romanised Hindi has no single spelling (theek, thik, theekh), so a typed
//! answer is compared after "folding" away the differences that don't change
//! the word: long vowels, aspiration, doubled letters and nasal endings. A
//! word that's still one letter out counts as close, which the learner can
//! accept or not when grading themselves. Devanagari answers work too.

use crate::content::normalize_word;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mark {
    /// The same word, perhaps spelled differently.
    Right,
    /// Nearly the same: probably a slip.
    Close,
    /// A word of the answer that wasn't typed.
    Missing,
    /// A typed word that isn't in the answer.
    Extra,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarkedWord {
    /// The answer's word, or the typed word for `Extra`.
    pub text: String,
    pub mark: Mark,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checked {
    /// The answer's words in order, with typed extras where they appeared.
    pub words: Vec<MarkedWord>,
}

impl Checked {
    /// Every word right and nothing extra.
    pub fn all_right(&self) -> bool {
        self.words.iter().all(|w| w.mark == Mark::Right)
    }
}

/// Compare what the learner typed with a phrase, given as its word tiles.
pub fn check_typed(typed: &str, roman_tiles: &[String], deva_tiles: &[String]) -> Checked {
    let devanagari = typed.chars().any(|c| ('\u{0900}'..='\u{097F}').contains(&c));
    let (expected, fold): (&[String], fn(&str) -> String) =
        if devanagari { (deva_tiles, fold_deva) } else { (roman_tiles, fold_roman) };

    let want: Vec<(String, String)> = expected
        .iter()
        .map(|w| (w.trim_matches(|c: char| c.is_ascii_punctuation() || c == '।').to_string(), fold(w)))
        .filter(|(_, f)| !f.is_empty())
        .collect();
    let got: Vec<(String, String)> = typed
        .split_whitespace()
        .map(|w| (w.to_string(), fold(w)))
        .filter(|(_, f)| !f.is_empty())
        .collect();

    // Line the words up on exact (folded) matches, longest common subsequence.
    let (n, m) = (want.len(), got.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if want[i].1 == got[j].1 { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }

    let mut words = Vec::new();
    let (mut i, mut j) = (0, 0);
    // Words between two matches are paired off in order, as close or wrong.
    let mut pending_want: Vec<usize> = Vec::new();
    let mut pending_got: Vec<usize> = Vec::new();
    let flush = |pw: &mut Vec<usize>, pg: &mut Vec<usize>, words: &mut Vec<MarkedWord>| {
        let mut extras = pg.iter().copied().peekable();
        for &wi in pw.iter() {
            match extras.peek() {
                Some(&gj) if is_close(&want[wi].1, &got[gj].1) => {
                    extras.next();
                    words.push(MarkedWord { text: want[wi].0.clone(), mark: Mark::Close });
                }
                _ => words.push(MarkedWord { text: want[wi].0.clone(), mark: Mark::Missing }),
            }
        }
        for gj in extras {
            words.push(MarkedWord { text: got[gj].0.clone(), mark: Mark::Extra });
        }
        pw.clear();
        pg.clear();
    };
    while i < n || j < m {
        if i < n && j < m && want[i].1 == got[j].1 {
            flush(&mut pending_want, &mut pending_got, &mut words);
            words.push(MarkedWord { text: want[i].0.clone(), mark: Mark::Right });
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            pending_got.push(j);
            j += 1;
        } else {
            pending_want.push(i);
            i += 1;
        }
    }
    flush(&mut pending_want, &mut pending_got, &mut words);
    Checked { words }
}

/// Romanised spelling with the variations that don't matter removed.
pub fn fold_roman(word: &str) -> String {
    let lower: String = normalize_word(word).chars().filter(|c| c.is_ascii_alphabetic()).collect();
    // Letters people swap freely: zaroor/jaroor, phir/fir, vahan/wahan.
    let swapped: String = lower
        .chars()
        .map(|c| match c {
            'w' => 'v',
            'z' => 'j',
            'q' => 'k',
            'f' => 'p',
            _ => c,
        })
        .collect();
    // Long vowels: theek/thik, hoon/hun, mein/me.
    let vowels = swapped.replace("ee", "i").replace("oo", "u").replace("ei", "e");
    // Aspiration: khaana/kaana, theek/teek, accha/acha. Keep a leading h (haan).
    let mut out = String::new();
    for c in vowels.chars() {
        let after_consonant = out.chars().last().is_some_and(|p| !"aeiou".contains(p));
        if c == 'h' && after_consonant {
            continue;
        }
        // Doubled letters: aap/ap, accha/acha.
        if out.ends_with(c) {
            continue;
        }
        out.push(c);
    }
    // Nasal endings: nahin/nahi, main/mai, hoon/hu. Except hain (are), which
    // is a different word from hai (is).
    if out != "hain" && out.len() > 2 && out.ends_with('n') && out[..out.len() - 1].ends_with(|c: char| "aeiou".contains(c)) {
        out.pop();
    }
    out
}

/// Devanagari with nukta and the two nasal marks treated alike.
pub fn fold_deva(word: &str) -> String {
    normalize_word(word).chars().filter(|&c| c != '\u{093C}').map(|c| if c == 'ँ' { 'ं' } else { c }).collect()
}

/// One letter out, or two in a long word.
fn is_close(a: &str, b: &str) -> bool {
    let allowed = if a.chars().count() >= 7 { 2 } else { 1 };
    a.chars().count() >= 3 && edit_distance(a, b) <= allowed
}

fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i; b.len() + 1];
        for j in 1..=b.len() {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        prev = cur;
    }
    prev[b.len()]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tiles(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    fn check(typed: &str, roman: &str) -> Checked {
        check_typed(typed, &tiles(roman), &[])
    }

    fn marks(c: &Checked) -> Vec<(String, Mark)> {
        c.words.iter().map(|w| (w.text.clone(), w.mark)).collect()
    }

    #[test]
    fn spelling_variants_fold_together() {
        for (a, b) in [
            ("theek", "thik"),
            ("hoon", "hun"),
            ("nahin", "nahi"),
            ("accha", "acha"),
            ("accha", "achha"),
            ("aap", "ap"),
            ("zaroor", "jaroor"),
            ("phir", "fir"),
            ("mein", "me"),
            ("khaana", "khana"),
            ("Dhanyavaad", "dhanyawad"),
        ] {
            assert_eq!(fold_roman(a), fold_roman(b), "{a} vs {b}");
        }
        assert_ne!(fold_roman("kaise"), fold_roman("kaisi"), "gender endings still count");
        assert_ne!(fold_roman("gaya"), fold_roman("gayi"));
        assert_ne!(fold_roman("hain"), fold_roman("hai"), "are and is are different words");
        assert_eq!(fold_roman("main"), fold_roman("mai"));
    }

    #[test]
    fn a_differently_spelled_answer_is_right() {
        let c = check("main thik hun", "Main theek hoon");
        assert!(c.all_right(), "{:?}", marks(&c));
        assert!(check("bas pet bhar gaya!", "Bas, pet bhar gaya").all_right());
    }

    #[test]
    fn a_wrong_ending_is_close_not_right() {
        let c = check("aap kaisi hain", "Aap kaise hain?");
        assert!(!c.all_right());
        assert_eq!(marks(&c)[1], ("kaise".into(), Mark::Close));
    }

    #[test]
    fn missing_and_extra_words_are_marked() {
        let c = check("bahut swadisht", "Khaana bahut swaadisht hai");
        assert_eq!(
            marks(&c),
            vec![
                ("Khaana".into(), Mark::Missing),
                ("bahut".into(), Mark::Right),
                ("swaadisht".into(), Mark::Right),
                ("hai".into(), Mark::Missing),
            ]
        );
        let c = check("haan ji", "Haan");
        assert_eq!(marks(&c), vec![("Haan".into(), Mark::Right), ("ji".into(), Mark::Extra)]);
        assert!(!c.all_right());
    }

    #[test]
    fn devanagari_answers_work() {
        let c = check_typed("बस पेट भर गया", &tiles("Bas, pet bhar gaya"), &tiles("बस, पेट भर गया"));
        assert!(c.all_right());
        // Nukta and chandrabindu are forgiven.
        assert_eq!(fold_deva("ज़रूर"), fold_deva("जरूर"));
        assert_eq!(fold_deva("हाँ"), fold_deva("हां"));
    }

    #[test]
    fn empty_answer_is_all_missing() {
        let c = check("", "Phir milenge");
        assert!(c.words.iter().all(|w| w.mark == Mark::Missing));
    }
}
