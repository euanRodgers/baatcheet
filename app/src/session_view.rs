//! A practice session: one card at a time, then the summary.

use crate::platform;
use crate::ui::{CloseIcon, Hindi, PlayButtons, interval_label, tile_text};
use crate::{Ctx, Screen};
use baatcheet_core::content::pack_name;
use baatcheet_core::progress::{INTERVALS, stats};
use baatcheet_core::{Card, CardKind, Outcome};
use dioxus::prelude::*;

#[component]
pub fn SessionScreen() -> Element {
    let ctx = use_context::<Ctx>();
    let live = ctx.live.read();
    let Some(l) = live.as_ref() else {
        return rsx! { div { class: "error", p { "No session running." } button { class: "btn", onclick: move |_| ctx.go(Screen::Home), "Back to today" } } };
    };
    let Some(card) = l.session.current().cloned() else {
        return rsx! {};
    };
    let total = l.session.cards.len().max(1);
    let pct = l.bar.max(l.session.pos * 100 / total);
    let bonus = l.session.bonus;
    let xp = l.session.xp;
    let has_sheet = l.feedback.is_some();

    rsx! {
        div { class: "sess",
            div { class: "sh",
                button { class: "x", "aria-label": "Stop and save", onclick: move |_| ctx.quit_session(), CloseIcon {} }
                div { class: "prog", b { style: "width: {pct}%" } }
                div { class: "xpb", "+{xp}" }
            }
            div { class: if has_sheet { "scr with-sheet" } else { "scr" },
                if bonus {
                    div { class: "kind", span { class: "chip", "Bonus round: practice only" } }
                }
                if card.retry {
                    div { class: "kind again", "Back again" }
                }
                CardBody { card: card.clone() }
            }
            if has_sheet {
                AnswerSheet { card: card.clone() }
            }
        }
    }
}

#[component]
fn CardBody(card: Card) -> Element {
    let mut ctx = use_context::<Ctx>();
    let content = ctx.content.read();
    let Some(p) = content.phrase(&card.phrase_id).cloned() else {
        return rsx! { p { "Missing phrase {card.phrase_id}" } };
    };
    let live = ctx.live.read();
    let l = live.as_ref().unwrap();
    let answered = l.feedback.is_some();
    let picked = l.picked;
    let built = l.built.clone();
    let revealed = l.revealed;
    let quiet_hints = card.retry || l.session.bonus;
    let show_deva = ctx.settings.read().show_deva;
    let id = p.id.clone();

    match card.kind {
        CardKind::Meet => rsx! {
            div { class: "kind",
                "New phrase"
                span { class: "chip", "{pack_name(&p.pack)}" }
                if p.direction == baatcheet_core::Direction::Hear {
                    span { class: "chip hear", "You'll hear this" }
                }
            }
            // The scene only makes sense when someone is saying this to Euan.
            if !p.context.is_empty() && p.direction.is_heard() {
                div { class: "ctx", "{p.context}" }
            }
            PlayButtons { phrase_id: id.clone() }
            Hindi { roman: p.roman.clone(), deva: p.deva.clone(), big: true }
            div { class: "eng", "{p.english}" }
            if !p.notes.is_empty() {
                div { class: "note", "{p.notes}" }
            }
            div { class: "spacer" }
            button {
                class: "btn",
                onclick: move |_| {
                    if let Some(l) = ctx.live.write().as_mut() {
                        l.session.meet(&mut ctx.progress.write(), platform::today());
                    }
                    ctx.save();
                    ctx.next_card();
                },
                "Got it"
            }
        },

        CardKind::ListenMeaning | CardKind::PickHindi | CardKind::Respond => {
            let right = card.right_option().unwrap_or(0);
            rsx! {
                match card.kind {
                    CardKind::ListenMeaning => rsx! {
                        div { class: "kind", "Listen" }
                        h3 { class: "q", "What does this mean?" }
                        PlayButtons { phrase_id: id.clone() }
                    },
                    CardKind::PickHindi => rsx! {
                        div { class: "kind", "Listen" }
                        h3 { class: "q", "Which one did you hear?" }
                        PlayButtons { phrase_id: id.clone() }
                    },
                    _ => rsx! {
                        div { class: "kind", "Respond" }
                        div { class: "ctx", if p.context.is_empty() { "Someone says:" } else { "{p.context}, and says:" } }
                        PlayButtons { phrase_id: id.clone() }
                        if answered {
                            Hindi { roman: p.roman.clone(), deva: p.deva.clone() }
                        }
                        h3 { class: "q", "What do you say?" }
                    },
                }
                div { class: "opts",
                    for (i, choice) in card.choices.iter().enumerate() {
                        button {
                            key: "{i}",
                            class: if answered && i == right { "opt right" } else if answered && picked == Some(i) { "opt wrong" } else { "opt" },
                            disabled: answered,
                            onclick: move |_| grade_choice(ctx, i, i == right),
                            if card.kind == CardKind::ListenMeaning {
                                "{choice.english}"
                            } else {
                                "{choice.roman}"
                                if show_deva {
                                    span { class: "d", lang: "hi", "{choice.deva}" }
                                }
                            }
                        }
                    }
                }
            }
        }

        CardKind::Build => {
            let tiles = card.choices.clone();
            let tiles_for_placed = tiles.clone();
            let card_for_check = card.clone();
            rsx! {
                div { class: "kind", "Build" }
                h3 { class: "q", "Say it in Hindi" }
                div { class: "eng big", "{p.english}" }
                div { class: "slots",
                    if built.is_empty() {
                        span { class: "ph", "Tap the words in order" }
                    }
                    for (k, &ti) in built.iter().enumerate() {
                        button {
                            key: "p{k}",
                            class: "tile",
                            disabled: answered,
                            onclick: move |_| {
                                if let Some(l) = ctx.live.write().as_mut() {
                                    l.built.retain(|&x| x != ti);
                                }
                            },
                            "{tile_text(&tiles_for_placed[ti].roman)}"
                            if show_deva {
                                span { class: "d", lang: "hi", "{tile_text(&tiles_for_placed[ti].deva)}" }
                            }
                        }
                    }
                }
                div { class: "bank",
                    for (i, t) in tiles.iter().enumerate() {
                        button {
                            key: "b{i}",
                            class: if built.contains(&i) { "tile used" } else { "tile" },
                            disabled: answered || built.contains(&i),
                            onclick: {
                                let deva = t.deva.clone();
                                move |_| {
                                    platform::play_word(&ctx.content.read(), &deva, false);
                                    if let Some(l) = ctx.live.write().as_mut() {
                                        l.built.push(i);
                                    }
                                }
                            },
                            "{tile_text(&t.roman)}"
                            if show_deva {
                                span { class: "d", lang: "hi", "{tile_text(&t.deva)}" }
                            }
                        }
                    }
                }
                div { class: "spacer" }
                if !answered {
                    button {
                        class: "btn",
                        // Only once every word is placed, so a stray tap can't mark it wrong.
                        disabled: built.len() != card.tiles_needed(),
                        onclick: move |_| {
                            let picked = ctx.live.read().as_ref().map(|l| l.built.clone()).unwrap_or_default();
                            grade(ctx, if card_for_check.build_is_right(&picked) { Outcome::Right } else { Outcome::Wrong });
                        },
                        "Check"
                    }
                }
            }
        }

        CardKind::Recall => rsx! {
            div { class: "kind", "Recall" }
            h3 { class: "q", "How would you say…" }
            div { class: "eng big", "{p.english}" }
            if revealed {
                Hindi { roman: p.roman.clone(), deva: p.deva.clone(), big: true }
                PlayButtons { phrase_id: id.clone() }
                div { class: "spacer" }
                div { class: "sec-l", "How did it go?" }
                div { class: "grade",
                    // On a retry or in a bonus round nothing moves, so the hints would be wrong.
                    button { class: "g1", onclick: move |_| grade_recall(ctx, Outcome::Right), "Got it" if !quiet_hints { small { "moves up" } } }
                    button { class: "g2", onclick: move |_| grade_recall(ctx, Outcome::Nearly), "Nearly" if !quiet_hints { small { "stays put" } } }
                    button { class: "g3", onclick: move |_| grade_recall(ctx, Outcome::Wrong), "Missed" if !quiet_hints { small { "see it again" } } }
                }
            } else {
                p { class: "muted", "Say it out loud, or in your head." }
                div { class: "spacer" }
                button {
                    class: "btn",
                    onclick: move |_| {
                        if let Some(l) = ctx.live.write().as_mut() {
                            l.revealed = true;
                        }
                        ctx.play_phrase(&id);
                    },
                    "Show answer"
                }
            }
        },
    }
}

fn grade_choice(mut ctx: Ctx, index: usize, right: bool) {
    if let Some(l) = ctx.live.write().as_mut() {
        l.picked = Some(index);
    }
    grade(ctx, if right { Outcome::Right } else { Outcome::Wrong });
}

/// Grade the current card, save, show the answer sheet and play the answer.
fn grade(mut ctx: Ctx, outcome: Outcome) {
    let answer_id = {
        let mut live = ctx.live.write();
        let Some(l) = live.as_mut() else { return };
        let card = l.session.current().cloned();
        let fb = l.session.grade(outcome, &mut ctx.progress.write(), platform::today());
        l.feedback = Some(fb);
        card.map(|c| c.reply_id.unwrap_or(c.phrase_id))
    };
    ctx.save();
    if let Some(id) = answer_id {
        ctx.play_phrase(&id);
    }
}

/// Self-graded cards skip the answer sheet: the answer is already showing.
fn grade_recall(mut ctx: Ctx, outcome: Outcome) {
    if let Some(l) = ctx.live.write().as_mut() {
        l.session.grade(outcome, &mut ctx.progress.write(), platform::today());
    }
    ctx.save();
    ctx.next_card();
}

#[component]
fn AnswerSheet(card: Card) -> Element {
    let ctx = use_context::<Ctx>();
    let live = ctx.live.read();
    let Some(fb) = live.as_ref().and_then(|l| l.feedback) else { return rsx! {} };
    let content = ctx.content.read();
    let target_id = card.reply_id.clone().unwrap_or(card.phrase_id.clone());
    let Some(target) = content.phrase(&target_id).cloned() else { return rsx! {} };
    let own = content.phrase(&card.phrase_id).cloned();
    let notes = own.map(|p| p.notes).unwrap_or_default();

    rsx! {
        div { class: if fb.right { "sheet ok" } else { "sheet no" }, role: "status",
            div { class: "h",
                if fb.right { "Sahi hai! That's right" } else { "Not quite" }
                small { "+{fb.xp} XP" }
            }
            if fb.requeued {
                div { class: "m", "Here's the answer. It'll come back in a couple of cards." }
            }
            div { class: "ans",
                PlayButtons { phrase_id: target_id.clone(), small: true }
                div {
                    Hindi { roman: target.roman.clone(), deva: target.deva.clone() }
                    div { class: "m", "{target.english}" }
                }
            }
            if !fb.right && !notes.is_empty() {
                div { class: "m", "{notes}" }
            }
            button { class: "btn", onclick: move |_| ctx.next_card(), "Continue" }
        }
    }
}

#[component]
pub fn Summary() -> Element {
    let mut ctx = use_context::<Ctx>();
    let live = ctx.live.read();
    let Some(l) = live.as_ref() else {
        return rsx! { div { class: "error", button { class: "btn", onclick: move |_| ctx.go(Screen::Home), "Back to today" } } };
    };
    let content = ctx.content.read();
    let s = stats(&content, &ctx.progress.read());
    let session = &l.session;
    let first_try_total = session.first_attempts;
    let bonus = session.bonus;
    let back: Vec<(String, String)> = session
        .moved
        .iter()
        .filter_map(|(id, b)| content.phrase(id).map(|p| (p.roman.clone(), interval_label(INTERVALS[*b as usize]))))
        .collect();
    let (xp, right_first, can_start, und_start) =
        (session.xp, session.right_first_time, session.can_use_start, session.understand_start);

    rsx! {
        div { class: "scr",
            div { class: "sum",
                div { class: "sec-l", "Session done" }
                h2 { "Shabaash, Euan!" }
                div { class: "rows",
                    div { class: "row", span { "XP earned" } b { "+{xp}" } }
                    if first_try_total > 0 {
                        div { class: "row", span { "Right first time" } b { "{right_first} of {first_try_total}" } }
                    }
                    div { class: "row", span { "Phrases I can use" } b { class: if s.can_use > can_start { "up" } else { "" }, "{can_start} → {s.can_use}" } }
                    if s.understand != und_start {
                        div { class: "row", span { "Phrases I understand" } b { class: "up", "{und_start} → {s.understand}" } }
                    }
                }
                if !back.is_empty() {
                    div { class: "sec-l", style: "justify-self: start", "Coming back" }
                    div { class: "back-list",
                        for (i, (roman, when)) in back.iter().enumerate() {
                            div { key: "{i}", b { "{roman}" } span { "{when}" } }
                        }
                    }
                }
                if bonus {
                    p { class: "muted", "Bonus rounds are practice only, so nothing moved. Your normal reviews are unchanged." }
                } else {
                    p { class: "muted", "Skipping tomorrow is fine. Nothing resets." }
                }
                button {
                    class: "btn",
                    onclick: move |_| {
                        ctx.live.set(None);
                        ctx.go(Screen::Home);
                    },
                    "Back to today"
                }
            }
        }
    }
}
