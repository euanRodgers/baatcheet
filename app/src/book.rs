//! The Phrasebook: every phrase, searchable and playable, with a detail view
//! showing the words, replies, progress and "Used it for real".

use crate::platform;
use crate::ui::{CloseIcon, Hindi, PlayButtons, TabBar, WordBreakdown, interval_label};
use crate::Ctx;
use baatcheet_core::content::{PACKS, pack_name};
use baatcheet_core::progress::stats;
use baatcheet_core::{Direction, Phrase};
use dioxus::prelude::*;

#[derive(Clone, Copy, PartialEq)]
enum Filter {
    All,
    Say,
    Hear,
}

fn matches(p: &Phrase, query: &str, filter: Filter) -> bool {
    let by_direction = match filter {
        Filter::All => true,
        Filter::Say => p.direction.is_said(),
        Filter::Hear => p.direction.is_heard(),
    };
    let q = query.trim().to_lowercase();
    by_direction
        && (q.is_empty()
            || p.roman.to_lowercase().contains(&q)
            || p.english.to_lowercase().contains(&q)
            || p.deva.contains(query.trim()))
}

#[component]
pub fn Book() -> Element {
    let ctx = use_context::<Ctx>();
    let mut query = use_signal(String::new);
    let mut filter = use_signal(|| Filter::All);
    let mut open = use_signal(|| None::<String>);
    let content = ctx.content.read();
    let progress = ctx.progress.read();
    let show_deva = ctx.settings.read().show_deva;
    let s = stats(&content, &progress);
    let q = query();
    let f = filter();
    let total_shown = content.phrases.iter().filter(|p| matches(p, &q, f)).count();

    rsx! {
        div { class: "scr",
            div { class: "hello",
                div {
                    div { class: "date", "{s.can_use} I can use · {s.understand} I understand" }
                    h2 { "Phrasebook" }
                }
            }
            input {
                class: "search",
                r#type: "search",
                placeholder: "Search in English or Hindi",
                "aria-label": "Search phrases",
                autocomplete: "off",
                autocapitalize: "none",
                spellcheck: "false",
                value: "{q}",
                oninput: move |e| query.set(e.value()),
            }
            div { class: "filters", role: "radiogroup",
                for (value, label) in [(Filter::All, "All"), (Filter::Say, "I say"), (Filter::Hear, "I hear")] {
                    button {
                        key: "{label}",
                        class: if f == value { "chip-btn on" } else { "chip-btn" },
                        role: "radio",
                        "aria-checked": "{f == value}",
                        onclick: move |_| filter.set(value),
                        "{label}"
                    }
                }
            }
            if total_shown == 0 {
                p { class: "muted", "No phrases match \"{q}\"." }
            }
            for pack in PACKS.iter().filter(|p| content.in_pack(p.id).any(|ph| matches(ph, &q, f))) {
                div { key: "{pack.id}", class: "pb-pack",
                    div { class: "sec-l", "{pack.name}" }
                    for p in content.in_pack(pack.id).filter(|ph| matches(ph, &q, f)) {
                        {
                            let entry = progress.phrases.get(&p.id);
                            let id = p.id.clone();
                            rsx! {
                                div { key: "{p.id}", class: "pbr",
                                    PlayButtons { phrase_id: p.id.clone(), small: true }
                                    button {
                                        class: "pbr-main",
                                        "aria-label": "Details for {p.roman}",
                                        onclick: move |_| open.set(Some(id.clone())),
                                        div { class: "r",
                                            "{p.roman} "
                                            if p.direction == Direction::Hear {
                                                span { class: "chip hear", "Hear" }
                                            }
                                        }
                                        if show_deva {
                                            div { class: "d", lang: "hi", "{p.deva}" }
                                        }
                                        div { class: "e", "{p.english}" }
                                        div { class: "boxes",
                                            match entry {
                                                Some(e) => rsx! {
                                                    for b in 1..=5u8 {
                                                        i { key: "{b}", class: if e.box_ >= b { "on" } else { "" } }
                                                    }
                                                },
                                                None => rsx! { span { "Not met yet" } },
                                            }
                                        }
                                    }
                                    span { class: "chev", "aria-hidden": "true", "›" }
                                }
                            }
                        }
                    }
                }
            }
        }
        TabBar {}
        if let Some(id) = open() {
            PhraseDetail { phrase_id: id, on_close: move |_| open.set(None) }
        }
    }
}

#[component]
fn PhraseDetail(phrase_id: String, on_close: EventHandler<()>) -> Element {
    let mut ctx = use_context::<Ctx>();
    let content = ctx.content.read();
    let progress = ctx.progress.read();
    let Some(p) = content.phrase(&phrase_id).cloned() else { return rsx! {} };
    let entry = progress.phrases.get(&p.id).cloned();
    let used = progress.used_for_real(&p.id);
    let today = platform::today();
    let replies: Vec<Phrase> = p.replies.iter().filter_map(|r| content.phrase(r).cloned()).collect();
    let id = p.id.clone();

    rsx! {
        div { class: "overlay", onclick: move |_| on_close.call(()),
            div {
                class: "detail",
                role: "dialog",
                "aria-label": "{p.roman}",
                onclick: move |e| e.stop_propagation(),
                div { class: "detail-top",
                    div { class: "kind",
                        span { class: "chip", "{pack_name(&p.pack)}" }
                        match p.direction {
                            Direction::Say => rsx! { span { class: "chip hear", "You say this" } },
                            Direction::Hear => rsx! { span { class: "chip hear", "You'll hear this" } },
                            Direction::Both => rsx! { span { class: "chip hear", "Said both ways" } },
                        }
                    }
                    button { class: "x", "aria-label": "Close", onclick: move |_| on_close.call(()), CloseIcon {} }
                }
                PlayButtons { phrase_id: p.id.clone() }
                Hindi { roman: p.roman.clone(), deva: p.deva.clone(), big: true }
                div { class: "eng", "{p.english}" }
                if !p.context.is_empty() && p.direction.is_heard() {
                    div { class: "ctx", "{p.context}" }
                }
                WordBreakdown { phrase_id: p.id.clone() }
                if !p.notes.is_empty() {
                    div { class: "note", "{p.notes}" }
                }
                if !replies.is_empty() {
                    div { class: "sec-l", "Ways to reply" }
                    div { class: "replies",
                        for r in replies {
                            {
                                let rid = r.id.clone();
                                rsx! {
                                    button { key: "{r.id}", class: "reply", onclick: move |_| ctx.play_phrase(&rid),
                                        b { "{r.roman}" }
                                        span { "{r.english}" }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "sec-l", "Your progress" }
                div { class: "set",
                    div { class: "set-row",
                        match &entry {
                            Some(e) => rsx! {
                                div { "Level {e.box_} of 5"
                                    small {
                                        if e.due <= today { "Due for review now" } else { "Next review {interval_label(e.due.days_since(today))}" }
                                    }
                                }
                                div { class: "boxes",
                                    for b in 1..=5u8 {
                                        i { key: "{b}", class: if e.box_ >= b { "on" } else { "" } }
                                    }
                                }
                            },
                            None => rsx! { div { "Not met yet" small { "It'll come up in a session soon." } } },
                        }
                    }
                    if p.direction.is_said() {
                        div { class: "set-row",
                            div { "Used it for real" small { "Said it at a real family meal? Worth 25 XP." } }
                            button {
                                class: if used { "used on" } else { "used" },
                                onclick: move |_| {
                                    let now_used = ctx.progress.write().toggle_used_for_real(&id);
                                    ctx.save();
                                    if now_used {
                                        ctx.show_toast("+25 XP. Nice one.");
                                    }
                                },
                                if used { "Used ✓" } else { "Used it" }
                            }
                        }
                    }
                }
            }
        }
    }
}
