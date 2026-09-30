//! First launch: the learner's name, and which gendered forms to teach.

use crate::Ctx;
use baatcheet_core::Gender;
use dioxus::prelude::*;

#[component]
pub fn Welcome() -> Element {
    let ctx = use_context::<Ctx>();
    let mut name = use_signal(String::new);
    let gender: Signal<Option<Gender>> = use_signal(|| None);
    let ready = !name().trim().is_empty() && gender().is_some();

    rsx! {
        div { class: "scr welcome",
            h1 { "Namaste!" }
            p { class: "lede", "Everyday Hindi, ten minutes a day. First, two quick questions." }
            label { class: "field", r#for: "learner-name",
                span { "What's your name?" }
                input {
                    id: "learner-name",
                    r#type: "text",
                    autocomplete: "given-name",
                    placeholder: "Your first name",
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
                }
            }
            div { class: "field",
                span { "Which forms should we teach you?" }
                p { class: "muted", "Some Hindi words change with who's speaking. A man says samajh gaya (I understood), a woman says samajh gayi." }
                GenderChoice { gender }
            }
            div { class: "spacer" }
            button {
                class: "btn",
                disabled: !ready,
                onclick: move |_| {
                    let (n, g) = (name().trim().to_string(), gender());
                    ctx.update_settings(|s| {
                        s.name = n;
                        s.gender = g;
                    });
                },
                "Start learning"
            }
            p { class: "muted small", "You can change these in Settings. Everything stays on this phone." }
        }
    }
}

/// The two-way choice, used here and in Settings.
#[component]
pub fn GenderChoice(gender: Signal<Option<Gender>>) -> Element {
    let option = move |g: Gender, title: &'static str, example: &'static str| {
        let on = gender() == Some(g);
        rsx! {
            button {
                class: if on { "choice on" } else { "choice" },
                role: "radio",
                "aria-checked": "{on}",
                onclick: move |_| gender.set(Some(g)),
                b { "{title}" }
                small { "{example}" }
            }
        }
    };
    rsx! {
        div { class: "choices", role: "radiogroup",
            {option(Gender::Man, "A man's forms", "samajh gaya")}
            {option(Gender::Woman, "A woman's forms", "samajh gayi")}
        }
    }
}
