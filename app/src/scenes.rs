//! Scenes: short conversations played as chat bubbles. On the learner's turns
//! they pick what to say. Practice only, so mistakes cost nothing.

use crate::platform;
use crate::ui::{CloseIcon, TabBar};
use crate::{Ctx, Screen};
use baatcheet_core::scene::ScenePlay;
use dioxus::prelude::*;

#[component]
pub fn SceneList() -> Element {
    let ctx = use_context::<Ctx>();
    let content = ctx.content.read();
    let progress = ctx.progress.read();
    let scenes: Vec<(String, String, String, usize, usize, usize)> = content
        .scenes
        .iter()
        .map(|s| {
            let (met, total) = ScenePlay::phrases_met(s, &progress);
            let turns = s.lines.iter().filter(|l| l.is_yours()).count();
            (s.id.clone(), s.title.clone(), s.intro.clone(), turns, met, total)
        })
        .collect();

    rsx! {
        div { class: "scr",
            div { class: "hello", div { h2 { "Scenes" } } }
            p { class: "muted", "Short conversations from family life. Listen to each line, then choose what to say on your turns. It's practice, so nothing here counts against you." }
            div { class: "scene-list",
                for (id, title, intro, turns, met, total) in scenes {
                    button {
                        key: "{id}",
                        class: "scene-card",
                        onclick: {
                            let id = id.clone();
                            move |_| ctx.start_scene(&id)
                        },
                        div { class: "st", "{title}" }
                        div { class: "si", "{intro}" }
                        div { class: "sm",
                            span { "{turns} turns for you" }
                            span { if met == total { "All phrases met" } else { "{met} of {total} phrases met" } }
                        }
                    }
                }
            }
        }
        TabBar {}
    }
}

#[component]
pub fn ScenePlayer() -> Element {
    let ctx = use_context::<Ctx>();
    let mut show_english = use_signal(|| false);
    let mut finished = use_signal(|| false);
    let scene = ctx.scene.read();
    let Some(play) = scene.as_ref() else {
        return rsx! { div { class: "error", button { class: "btn", onclick: move |_| ctx.go(Screen::Scenes), "Back to scenes" } } };
    };
    let content = ctx.content.read();
    let show_deva = ctx.settings.read().show_deva;
    let english = show_english();

    // Scroll the newest line into view whenever one appears.
    let shown = play.shown;
    let answered = play.picked.is_some();
    use_effect(use_reactive!(|(shown, answered)| {
        let _ = (shown, answered);
        spawn(async move {
            platform::sleep(30).await;
            platform::scroll_to_bottom("scene-scroll");
        });
    }));

    if finished() {
        let (turns, right, xp) = (play.turns, play.right, play.xp);
        let id = play.scene.id.clone();
        return rsx! {
            div { class: "scr",
                div { class: "sum",
                    div { class: "sec-l", "Scene done" }
                    h2 { "{play.scene.title}" }
                    div { class: "rows",
                        div { class: "row", span { "Your turns" } b { "{right} of {turns} right" } }
                        div { class: "row", span { "XP earned" } b { "+{xp}" } }
                    }
                    p { class: "muted", "Scenes are practice only. Your reviews are unchanged." }
                    button {
                        class: "btn",
                        onclick: move |_| {
                            finished.set(false);
                            ctx.start_scene(&id);
                        },
                        "Play again"
                    }
                    button {
                        class: "btn ghost",
                        onclick: move |_| {
                            finished.set(false);
                            let mut scene = ctx.scene;
                            scene.set(None);
                            ctx.go(Screen::Scenes);
                        },
                        "Back to scenes"
                    }
                }
            }
        };
    }

    let lines: Vec<_> = play.scene.lines[..play.shown]
        .iter()
        .enumerate()
        .filter_map(|(i, l)| content.phrase(&l.phrase).map(|p| (i, l.clone(), p.clone())))
        .collect();
    let current = play.shown - 1;
    let waiting = play.waiting();
    let turn = play.turn.clone();
    let picked = play.picked;
    let is_last = play.is_last();
    let right_option = turn.as_ref().and_then(|t| t.right_option());

    rsx! {
        div { class: "sess",
            div { class: "sh",
                button {
                    class: "x",
                    "aria-label": "Leave scene",
                    onclick: move |_| {
                        let mut scene = ctx.scene;
                        scene.set(None);
                        ctx.go(Screen::Scenes);
                    },
                    CloseIcon {}
                }
                div { class: "scene-title", "{play.scene.title}" }
                button {
                    class: if english { "mini on" } else { "mini" },
                    "aria-pressed": "{english}",
                    onclick: move |_| show_english.set(!english),
                    "English"
                }
            }
            div { class: "scr chat", id: "scene-scroll",
                p { class: "ctx", "{play.scene.intro}" }
                for (i, line, p) in lines {
                    if !(line.is_yours() && i == current && waiting) {
                        {
                            let yours = line.is_yours();
                            let id = p.id.clone();
                            let missed = yours && i == current && picked.is_some() && picked != right_option;
                            rsx! {
                                button {
                                    key: "{i}",
                                    class: if yours { "bubble you" } else { "bubble" },
                                    onclick: move |_| ctx.play_phrase(&id),
                                    div { class: "who", if yours { "You" } else { "{line.speaker}" } }
                                    div { class: "br", "{p.roman}" }
                                    if show_deva {
                                        div { class: "bd", lang: "hi", "{p.deva}" }
                                    }
                                    if english || missed {
                                        div { class: "be", "{p.english}" }
                                    }
                                }
                                if missed {
                                    div { class: "nudge", "Not quite. This is what you'd say. Tap it to hear it again." }
                                }
                            }
                        }
                    }
                }
                if waiting {
                    if let Some(t) = turn {
                        div { class: "your-turn",
                            div { class: "sec-l", "Your turn. What do you say?" }
                            div { class: "opts",
                                for (k, c) in t.choices.iter().enumerate() {
                                    button {
                                        key: "{k}",
                                        class: "opt",
                                        onclick: move |_| ctx.scene_answer(k),
                                        "{c.roman}"
                                        if show_deva {
                                            span { class: "d", lang: "hi", "{c.deva}" }
                                        }
                                        if english {
                                            span { class: "d", "{c.english}" }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            if !waiting {
                div { class: "scene-foot",
                    if is_last {
                        button { class: "btn", onclick: move |_| finished.set(true), "Finish" }
                    } else {
                        button { class: "btn", onclick: move |_| ctx.scene_next(), "Next" }
                    }
                }
            }
        }
    }
}
