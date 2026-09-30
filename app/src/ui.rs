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
    let tab = |screen: Screen| if current == screen { "tab on" } else { "tab" };
    rsx! {
        nav { class: "tabbar",
            button { class: tab(Screen::Home), onclick: move |_| ctx.go(Screen::Home),
                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                    path { d: "M3 11l9-7 9 7v9a1 1 0 0 1-1 1h-5v-6H9v6H4a1 1 0 0 1-1-1z" }
                }
                "Today"
            }
            button { class: tab(Screen::Book), onclick: move |_| ctx.go(Screen::Book),
                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                    path { d: "M4 5a2 2 0 0 1 2-2h13v16H6a2 2 0 0 0-2 2z" }
                    path { d: "M4 21V5" }
                }
                "Phrasebook"
            }
            button { class: tab(Screen::Settings), onclick: move |_| ctx.go(Screen::Settings),
                svg { view_box: "0 0 24 24", fill: "none", stroke: "currentColor", stroke_width: "2", "aria-hidden": "true",
                    circle { cx: "12", cy: "12", r: "3" }
                    path { d: "M12 2v3M12 19v3M2 12h3M19 12h3M4.9 4.9l2.1 2.1M17 17l2.1 2.1M4.9 19.1L7 17M17 7l2.1-2.1" }
                }
                "Settings"
            }
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
