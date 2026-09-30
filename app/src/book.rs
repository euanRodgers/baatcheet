//! The Phrasebook: every phrase, playable, with "Used it for real".

use crate::ui::{PlayButtons, TabBar};
use crate::Ctx;
use baatcheet_core::content::PACKS;
use baatcheet_core::progress::stats;
use dioxus::prelude::*;

#[component]
pub fn Book() -> Element {
    let mut ctx = use_context::<Ctx>();
    let content = ctx.content.read();
    let progress = ctx.progress.read();
    let show_deva = ctx.settings.read().show_deva;
    let s = stats(&content, &progress);

    rsx! {
        div { class: "scr",
            div { class: "hello",
                div {
                    div { class: "date", "{s.can_use} I can use · {s.understand} I understand" }
                    h2 { "Phrasebook" }
                }
            }
            p { class: "muted", "Tap \"Used it\" when you say a phrase at a real family meal. It's worth 25 XP." }
            for pack in PACKS.iter().filter(|p| content.in_pack(p.id).next().is_some()) {
                div { key: "{pack.id}", class: "pb-pack",
                    div { class: "sec-l", "{pack.name}" }
                    for p in content.in_pack(pack.id) {
                        {
                            let entry = progress.phrases.get(&p.id);
                            let used = progress.used_for_real(&p.id);
                            let id = p.id.clone();
                            rsx! {
                                div { key: "{p.id}", class: "pbr",
                                    PlayButtons { phrase_id: p.id.clone(), small: true }
                                    div {
                                        div { class: "r",
                                            "{p.roman} "
                                            if p.direction == baatcheet_core::Direction::Hear {
                                                span { class: "chip hear", "Hear" }
                                            }
                                        }
                                        if show_deva {
                                            div { class: "d", lang: "hi", "{p.deva}" }
                                        }
                                        div { class: "e", "{p.english}" }
                                        if !p.notes.is_empty() {
                                            div { class: "n", "{p.notes}" }
                                        }
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
                                    if p.direction.is_said() {
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
                                    } else {
                                        span {}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        TabBar {}
    }
}
