//! Settings and backups.

use crate::platform::{self, BackupResult};
use crate::ui::TabBar;
use crate::Ctx;
use baatcheet_core::content::Status;
use baatcheet_core::Progress;
use dioxus::prelude::*;

/// "12 phrases, 340 XP"
fn describe(p: &Progress) -> String {
    format!("{} phrase{}, {} XP", p.phrases.len(), if p.phrases.len() == 1 { "" } else { "s" }, p.xp)
}

/// Replace progress, keeping the old copy so it can be undone.
fn replace_progress(ctx: Ctx, mut previous: Signal<Option<Progress>>, new: Progress) {
    let mut progress = ctx.progress;
    let old = progress.read().clone();
    platform::keep_previous(&old);
    previous.set(Some(old));
    progress.set(new);
    ctx.save();
}

#[component]
pub fn SettingsScreen() -> Element {
    let ctx = use_context::<Ctx>();
    let mut confirm_reset = use_signal(|| false);
    // A backup file that's been read and checked, waiting for "Replace".
    let mut pending_restore: Signal<Option<Progress>> = use_signal(|| None);
    let mut previous = use_signal(platform::previous_progress);
    let settings = ctx.settings.read().clone();
    let today = platform::today();
    let backup_label = match settings.last_backup {
        None => "Never backed up".to_string(),
        Some(d) => match today.days_since(d) {
            0 => "Backed up today".to_string(),
            1 => "Backed up yesterday".to_string(),
            n => format!("Last backup {n} days ago"),
        },
    };
    let voice = platform::hindi_voice_name();
    let content = ctx.content.read();
    let total = content.phrases.len();
    let drafts = content.phrases.iter().filter(|p| p.status == Status::Draft).count();
    let with_audio = content.phrases.iter().filter(|p| p.audio).count();
    let current = describe(&ctx.progress.read());

    rsx! {
        div { class: "scr",
            div { class: "hello", div { h2 { "Settings" } } }
            div { class: "set",
                div { class: "set-row",
                    div { "Show Devanagari" small { "Under the romanised text" } }
                    button {
                        class: if settings.show_deva { "sw on" } else { "sw" },
                        role: "switch",
                        "aria-checked": "{settings.show_deva}",
                        "aria-label": "Show Devanagari",
                        onclick: move |_| ctx.update_settings(|s| s.show_deva = !s.show_deva),
                    }
                }
                div { class: "set-row",
                    div { "Play slowly" small { "Every clip at 0.75× speed" } }
                    button {
                        class: if settings.slow { "sw on" } else { "sw" },
                        role: "switch",
                        "aria-checked": "{settings.slow}",
                        "aria-label": "Play slowly",
                        onclick: move |_| ctx.update_settings(|s| s.slow = !s.slow),
                    }
                }
                div { class: "set-row",
                    div { "Speaking as" small { "Sets forms like samajh gaya" } }
                    b { "Man" }
                }
            }

            div { class: "sec-l", "Backup" }
            div { class: "set",
                div { class: "set-row",
                    div { "Save a backup" small { "{backup_label}. Save it to Files or send it to yourself." } }
                    button {
                        class: "mini",
                        onclick: move |_| async move {
                            let json = ctx.progress.read().to_json();
                            let name = format!("baatcheet-backup-{}.json", platform::today());
                            match platform::save_backup(&name, &json).await {
                                BackupResult::Saved | BackupResult::Downloaded => {
                                    ctx.update_settings(|s| s.last_backup = Some(platform::today()));
                                    ctx.show_toast("Backup saved.");
                                }
                                BackupResult::Cancelled => {}
                                BackupResult::Failed => ctx.show_toast("Couldn't save the backup. Try again."),
                            }
                        },
                        "Save"
                    }
                }
                match pending_restore() {
                    Some(backup) => rsx! {
                        div { class: "set-row",
                            div { "Replace your progress?"
                                small { "Now: {current}. Backup: {describe(&backup)}. You can undo this." }
                            }
                            div { class: "pair",
                                button { class: "mini", onclick: move |_| pending_restore.set(None), "Cancel" }
                                button {
                                    class: "mini danger",
                                    onclick: move |_| {
                                        if let Some(b) = pending_restore.take() {
                                            replace_progress(ctx, previous, b);
                                            ctx.show_toast("Progress restored.");
                                        }
                                    },
                                    "Replace"
                                }
                            }
                        }
                    },
                    None => rsx! {
                        div { class: "set-row",
                            div { "Restore from a backup" small { "You'll see what it contains first" } }
                            label { class: "mini", r#for: "import-file", "Choose file" }
                            input {
                                class: "file",
                                id: "import-file",
                                r#type: "file",
                                accept: "application/json,.json",
                                onchange: move |_| async move {
                                    let Some(text) = platform::read_chosen_file("import-file").await else {
                                        ctx.show_toast("Couldn't read that file.");
                                        return;
                                    };
                                    match Progress::from_json(&text) {
                                        Ok(p) => pending_restore.set(Some(p)),
                                        Err(_) => ctx.show_toast("That isn't a Baatcheet backup file."),
                                    }
                                },
                            }
                        }
                    },
                }
                if let Some(prev) = previous() {
                    div { class: "set-row",
                        div { "Undo last restore or reset" small { "Go back to {describe(&prev)}" } }
                        button {
                            class: "mini",
                            onclick: move |_| {
                                if let Some(p) = previous.take() {
                                    let mut progress = ctx.progress;
                                    progress.set(p);
                                    ctx.save();
                                    platform::forget_previous_progress();
                                    ctx.show_toast("Undone.");
                                }
                            },
                            "Undo"
                        }
                    }
                }
            }

            div { class: "sec-l", "About" }
            div { class: "set",
                div { class: "set-row",
                    div { "Phrases" small { "{drafts} of {total} still drafts awaiting review" } }
                    b { "{total}" }
                }
                div { class: "set-row",
                    div { "Audio"
                        small {
                            if with_audio > 0 { "{with_audio} recorded clips. " }
                            match &voice {
                                Some(v) if with_audio > 0 => rsx! { "Anything else uses the device voice, {v}." },
                                Some(v) => rsx! { "Using the device's Hindi voice, {v}." },
                                None => rsx! { "No Hindi voice found on this device yet." },
                            }
                        }
                    }
                }
            }

            div { class: "set",
                div { class: "set-row",
                    div { "Start again" small { if confirm_reset() { "This clears all progress. You can undo it here." } else { "Clears all progress on this phone" } } }
                    if confirm_reset() {
                        button {
                            class: "mini danger",
                            onclick: move |_| {
                                replace_progress(ctx, previous, Progress::default());
                                confirm_reset.set(false);
                                ctx.show_toast("Progress cleared.");
                            },
                            "Clear"
                        }
                    } else {
                        button { class: "mini danger", onclick: move |_| confirm_reset.set(true), "Reset" }
                    }
                }
            }
        }
        TabBar {}
    }
}
