//! Small pieces used on several screens.

use crate::{Ctx, Screen};
use dioxus::prelude::*;

/// Romanised text, with Devanagari underneath when that setting is on.
#[component]
pub fn Hindi(roman: String, deva: String, #[props(default)] big: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let show_deva = ctx.settings.read().show_deva;
    rsx! {
        div { class: if big { "hi big" } else { "hi" },
            div { class: "rom", "{roman}" }
            if show_deva {
                div { class: "dev", lang: "hi", "{deva}" }
            }
        }
    }
}

/// Play and slow-play buttons for a phrase.
#[component]
pub fn PlayButtons(phrase_id: String, #[props(default)] small: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let id_normal = phrase_id.clone();
    let id_slow = phrase_id.clone();
    rsx! {
        div { class: if small { "aud sm" } else { "aud" },
            button {
                class: "play",
                "aria-label": "Play",
                onclick: move |_| ctx.play_phrase(&id_normal),
                PlayIcon {}
            }
            if !small {
                button {
                    class: "slow",
                    "aria-label": "Play slowly",
                    onclick: move |_| crate::platform::play_phrase(&ctx.content.read(), &id_slow, true),
                    "0.75×"
                }
            }
        }
    }
}

#[component]
pub fn PlayIcon() -> Element {
    rsx! {
        svg { view_box: "0 0 24 24", "aria-hidden": "true",
            path { fill: "currentColor", d: "M7 4.5v15a1 1 0 0 0 1.5.86l12-7.5a1 1 0 0 0 0-1.72l-12-7.5A1 1 0 0 0 7 4.5z" }
        }
    }
}

#[component]
pub fn CloseIcon() -> Element {
    rsx! {
        svg { view_box: "0 0 14 14", "aria-hidden": "true",
            path { d: "M2 2l10 10M12 2L2 12", stroke: "currentColor", stroke_width: "2", stroke_linecap: "round" }
        }
    }
}

#[component]
pub fn TabBar() -> Element {
    let ctx = use_context::<Ctx>();
    let current = *ctx.screen.read();
    let tab = |screens: &[Screen]| if screens.contains(&current) { "tab on" } else { "tab" };
    rsx! {
        nav { class: "tabbar",
            button { class: tab(&[Screen::Home]), onclick: move |_| ctx.go(Screen::Home),
                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                    path { d: "M3 11l9-7 9 7v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" }
                }
                "Today"
            }
            button { class: tab(&[Screen::Scenes, Screen::Scene]), onclick: move |_| ctx.go(Screen::Scenes),
                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                    path { d: "M4 5h11a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2H9l-4 3v-3H4a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2z" }
                    path { d: "M19 9h1a2 2 0 0 1 2 2v6a2 2 0 0 1-2 2h-1v3l-4-3h-3" }
                }
                "Scenes"
            }
            button { class: tab(&[Screen::Book]), onclick: move |_| ctx.go(Screen::Book),
                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                    path { d: "M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2z" }
                    path { d: "M4 21V5" }
                }
                "Phrasebook"
            }
            button { class: tab(&[Screen::Settings]), onclick: move |_| ctx.go(Screen::Settings),
                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                    circle { cx: "12", cy: "12", r: "3" }
                    path { d: "M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1" }
                }
                "Settings"
            }
        }
    }
}

/// The phrase word by word: each word's meaning, and its sound when tapped.
#[component]
pub fn WordBreakdown(phrase_id: String, #[props(default)] compact: bool) -> Element {
    let ctx = use_context::<Ctx>();
    let mut selected = use_signal(|| None::<usize>);
    let content = ctx.content.read();
    let Some(phrase) = content.phrase(&phrase_id) else { return rsx! {} };
    let show_deva = ctx.settings.read().show_deva;
    let words: Vec<(String, String, String, String)> = phrase
        .word_pairs()
        .map(|(roman, deva)| {
            let w = content.word(deva);
            (
                tile_text(roman),
                tile_text(deva),
                w.map(|w| w.meaning.clone()).unwrap_or_default(),
                w.map(|w| w.note.clone()).unwrap_or_default(),
            )
        })
        .collect();
    // A one-word phrase gains nothing from a breakdown.
    if words.len() < 2 {
        return rsx! {};
    }
    let note = selected().and_then(|i| words.get(i)).map(|w| w.3.clone()).filter(|n| !n.is_empty());

    rsx! {
        div { class: if compact { "words compact" } else { "words" },
            for (i, (roman, deva, meaning, _)) in words.iter().cloned().enumerate() {
                button {
                    key: "{i}",
                    class: if selected() == Some(i) { "word on" } else { "word" },
                    "aria-label": "{roman}: {meaning}",
                    onclick: {
                        let deva = deva.clone();
                        move |_| {
                            crate::platform::play_word(&ctx.content.read(), &deva, ctx.slow());
                            selected.set(if selected() == Some(i) { None } else { Some(i) });
                        }
                    },
                    span { class: "wr", "{roman}" }
                    if show_deva && !compact {
                        span { class: "wd", lang: "hi", "{deva}" }
                    }
                    span { class: "wm", "{meaning}" }
                }
            }
        }
        if let Some(n) = note {
            p { class: "word-note", "{n}" }
        }
    }
}

/// "tomorrow", "in 3 days", "in 2 weeks"...
pub fn interval_label(days: i32) -> String {
    match days {
        d if d <= 0 => "later today".into(),
        1 => "tomorrow".into(),
        7 => "in a week".into(),
        14 => "in 2 weeks".into(),
        30 => "in a month".into(),
        d => format!("in {d} days"),
    }
}

/// Punctuation stripped from a word tile for display.
pub fn tile_text(s: &str) -> String {
    s.trim_matches(|c: char| c.is_ascii_punctuation() || c == '।').to_string()
}
