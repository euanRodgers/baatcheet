//! Everything Baatcheet decides, with no web code: content and progress types,
//! the Leitner scheduler, card generation and the session builder.

pub mod cards;
pub mod content;
pub mod date;
pub mod progress;
pub mod rng;
pub mod session;

pub use cards::{Answer, Card, CardKind, Choice};
pub use content::{Content, Direction, Gender, Learner, Phrase};
pub use date::Date;
pub use progress::{Outcome, Progress};
pub use session::{Feedback, Session};

#[cfg(test)]
pub(crate) fn test_content() -> Content {
    use content::{Priority, Status};
    let p = |id: &str, dir: Direction, roman: &str, deva: &str, english: &str, replies: &[&str]| Phrase {
        id: id.into(),
        pack: id.split('-').next().unwrap().into(),
        priority: Priority::Core,
        direction: dir,
        context: String::new(),
        roman: roman.into(),
        deva: deva.into(),
        english: english.into(),
        notes: String::new(),
        tiles: roman.split_whitespace().map(String::from).collect(),
        deva_tiles: deva.split_whitespace().map(String::from).collect(),
        replies: replies.iter().map(|s| s.to_string()).collect(),
        status: Status::Draft,
        audio: false,
        female: None,
        female_form: false,
    };
    use Direction::*;
    Content {
        version: 1,
        word_audio: vec![],
        phrases: vec![
            p("greet-001", Say, "Namaste", "नमस्ते", "Hello", &[]),
            p("greet-002", Say, "Aap kaise hain?", "आप कैसे हैं?", "How are you? (to a man)", &[]),
            p("greet-003", Say, "Aap kaisi hain?", "आप कैसी हैं?", "How are you? (to a woman)", &[]),
            p("greet-004", Say, "Main theek hoon", "मैं ठीक हूँ", "I'm fine", &[]),
            p("food-001", Say, "Khaana bahut swaadisht hai", "खाना बहुत स्वादिष्ट है", "The food is very tasty", &[]),
            p("food-002", Say, "Bas, pet bhar gaya", "बस, पेट भर गया", "Enough, I'm full", &[]),
            p("food-003", Hear, "Aur lijiye!", "और लीजिए!", "Have some more!", &["food-002"]),
            p("surv-001", Say, "Dheere boliye", "धीरे बोलिए", "Please speak slowly", &[]),
            p("talk-001", Both, "Safar kaisa tha?", "सफ़र कैसा था?", "How was the journey?", &["talk-002"]),
            p("talk-002", Say, "Safar accha tha", "सफ़र अच्छा था", "The journey was good", &[]),
        ],
    }
}
