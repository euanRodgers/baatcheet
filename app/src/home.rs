//! The Today screen.

use crate::platform;
use crate::ui::TabBar;
use crate::{Ctx, Screen};
use baatcheet_core::content::PACKS;
use baatcheet_core::progress::{badges, pack_progress, stats};
use baatcheet_core::session::MAX_NEW_PER_DAY;
use baatcheet_core::{CardKind, Session};
use dioxus::prelude::*;

/// Remind about backups when the last one is older than this.
const BACKUP_REMINDER_DAYS: i32 = 30;

#[component]
pub fn Home() -> Element {
    let ctx = use_context::<Ctx>();
    let today = platform::today();
    let content = ctx.content.read();
    let progress = ctx.progress.read();
    let settings = ctx.settings.read();

    // Preview today's session to describe it. It's cheap to build.
    let preview = Session::build(&content, &progress, today, 0.0, 1);
    let new_count = preview.cards.iter().filter(|c| c.kind == CardKind::Meet).count();
    let review_count = preview.cards.iter().filter(|c| !c.intro).count();
    let minutes = ((preview.cards.len() as f64 * 20.0) / 60.0).ceil().max(1.0) as usize;
    let practised_today = progress.days.contains(&today);
    let has_progress = !progress.phrases.is_empty();
    let unmet_left = content.phrases.iter().any(|p| !progress.phrases.contains_key(&p.id));
    let new_limit_reached = unmet_left && progress.met_on(today) >= MAX_NEW_PER_DAY;
    let mut unreadable = use_signal(platform::unreadable_progress);

    let (headline, detail) = if preview.is_empty() {
        (
            "All caught up".to_string(),
            if new_limit_reached {
                format!("You've met {MAX_NEW_PER_DAY} new phrases today. More tomorrow.")
            } else if practised_today {
                "Nothing else is due today. Nice work.".to_string()
            } else {
                "Nothing is due today.".to_string()
            },
        )
    } else {
        let plan = match (review_count, new_count) {
            (0, n) => format!("{n} new phrase{} to meet.", plural(n)),
            (r, 0) => format!("{r} review{}.", plural(r)),
            (r, n) => format!("{r} review{} first, then {n} new phrase{}.", plural(r), plural(n)),
        };
        (format!("{} cards, about {minutes} min", preview.cards.len()), plan)
    };

    let s = stats(&content, &progress);
    let days_this_month = progress.days_in_month(today);
    let (_, _, day_of_month) = today.ymd();
    let month_len = today.month_len();
    let month_name = today.month_name();

    let needs_backup = has_progress
        && progress.days.len() >= 3
        && settings.last_backup.is_none_or(|d| today.days_since(d) >= BACKUP_REMINDER_DAYS);

    // The first pack with phrases not yet met is "up next".
    let next_pack = PACKS
        .iter()
        .find(|p| content.in_pack(p.id).any(|ph| !progress.phrases.contains_key(&ph.id)))
        .map(|p| p.id);
    let packs: Vec<(&str, &str, usize, usize)> = PACKS
        .iter()
        .map(|p| {
            let (learned, total) = pack_progress(&content, &progress, p.id);
            (p.id, p.name, learned, total)
        })
        .filter(|(_, _, _, total)| *total > 0)
        .collect();
    let badge_list = badges(&content, &progress);
    let xp = progress.xp;

    rsx! {
        div { class: "scr",
            div { class: "hello",
                div {
                    div { class: "date", "{today.long_label()}" }
                    h2 { "Namaste, Euan" }
                }
                div { class: "xpb", "{xp} XP" }
            }
            div { class: "today",
                div { class: "l", "Today's practice" }
                div { class: "big", "{headline}" }
                div { class: "sub", "{detail}" }
                if !preview.is_empty() {
                    button { onclick: move |_| ctx.start_session(false), if practised_today { "Keep going" } else { "Start" } }
                } else if has_progress {
                    button { class: "quiet", onclick: move |_| ctx.start_session(true), "Practise anyway" }
                }
            }
            if let Some(raw) = unreadable() {
                div { class: "banner",
                    span { "Your saved progress couldn't be read, so the app started fresh. Save the old copy so it isn't lost." }
                    button {
                        onclick: move |_| {
                            let raw = raw.clone();
                            async move {
                                let name = format!("baatcheet-unreadable-{}.json", platform::today());
                                if matches!(platform::save_backup(&name, &raw).await, platform::BackupResult::Saved | platform::BackupResult::Downloaded) {
                                    platform::forget_unreadable_progress();
                                    unreadable.set(None);
                                }
                            }
                        },
                        "Save copy"
                    }
                }
            }
            if needs_backup {
                div { class: "banner",
                    span { "Back up your progress now and then, in case the phone clears it." }
                    button { onclick: move |_| ctx.go(Screen::Settings), "Back up" }
                }
            }
            div { class: "stats",
                div { class: "stat",
                    div { class: "v", "{s.can_use}" small { " / {s.say_total}" } }
                    div { class: "k", "Phrases I can use" }
                    div { class: "k2", "and {s.understand} of {s.hear_total} I understand" }
                }
                div { class: "stat",
                    div { class: "v", "{days_this_month}" }
                    div { class: "k", "Days practised in {month_name}" }
                    div { class: "cal",
                        for d in 1..=month_len {
                            i {
                                key: "{d}",
                                class: if progress.days.contains(&today.add_days(d as i32 - day_of_month as i32)) { "on" } else if d > day_of_month { "fut" } else { "" },
                                class: if d == day_of_month { "now" } else { "" },
                            }
                        }
                    }
                }
            }
            div { class: "sec-l", "Packs" }
            div { class: "packs",
                for (id, name, learned, total) in packs {
                    div { key: "{id}", class: if next_pack == Some(id) { "pk next" } else { "pk" },
                        div { class: "nm", "{name}" }
                        div { class: "ct",
                            if next_pack == Some(id) { "Up next · " }
                            "{learned}/{total}"
                        }
                        div { class: "bar", b { style: "width: {learned * 100 / total.max(1)}%" } }
                    }
                }
            }
            div { class: "sec-l", "Badges" }
            div { class: "badges",
                for b in badge_list {
                    span { key: "{b.name}", class: if b.earned { "bdg got" } else { "bdg" }, "{b.name}" }
                }
            }
        }
        TabBar {}
    }
}

fn plural(n: usize) -> &'static str {
    if n == 1 { "" } else { "s" }
}
