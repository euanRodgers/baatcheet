//! Baatcheet: everyday spoken Hindi, ten minutes a day.

mod book;
mod home;
mod platform;
mod session_view;
mod settings;
mod ui;

use baatcheet_core::{CardKind, Content, Feedback, Progress, Session};
use dioxus::prelude::*;
use platform::Settings;

fn main() {
    platform::register_service_worker();
    platform::request_persistent_storage();
    dioxus::launch(Root);
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Screen {
    Home,
    Book,
    Settings,
    Session,
    Summary,
}

/// A session in progress, plus what's on screen for the current card.
pub struct Live {
    pub session: Session,
    pub feedback: Option<Feedback>,
    /// The option tapped on a multiple-choice card.
    pub picked: Option<usize>,
    /// Tiles placed so far on a Build card.
    pub built: Vec<usize>,
    /// Recall card answer shown.
    pub revealed: bool,
    /// Furthest the progress bar has reached, in percent. A requeued card
    /// lengthens the session, but the bar never goes backwards.
    pub bar: usize,
}

impl Live {
    fn new(session: Session) -> Live {
        Live { session, feedback: None, picked: None, built: vec![], revealed: false, bar: 0 }
    }

    fn reset_card(&mut self) {
        let total = self.session.cards.len().max(1);
        self.bar = self.bar.max(self.session.pos.min(total) * 100 / total);
        self.feedback = None;
        self.picked = None;
        self.built.clear();
        self.revealed = false;
    }
}

/// App state shared by every screen. Signals are cheap to copy.
#[derive(Clone, Copy)]
pub struct Ctx {
    pub content: Signal<Content>,
    pub progress: Signal<Progress>,
    pub settings: Signal<Settings>,
    pub screen: Signal<Screen>,
    pub live: Signal<Option<Live>>,
    pub toast: Signal<Option<String>>,
}

impl Ctx {
    pub fn go(mut self, screen: Screen) {
        self.screen.set(screen);
    }

    pub fn save(self) {
        platform::save_progress(&self.progress.read());
    }

    pub fn update_settings(mut self, f: impl FnOnce(&mut Settings)) {
        f(&mut self.settings.write());
        platform::save_settings(&self.settings.read());
    }

    pub fn show_toast(mut self, msg: impl Into<String>) {
        self.toast.set(Some(msg.into()));
        spawn(async move {
            platform::sleep(2800).await;
            self.toast.set(None);
        });
    }

    pub fn slow(self) -> bool {
        self.settings.read().slow
    }

    pub fn play_phrase(self, id: &str) {
        platform::play_phrase(&self.content.read(), id, self.slow());
    }

    /// Cards that start with audio play it straight away. This always runs
    /// inside a tap handler, which is what lets iOS play it.
    fn play_prompt(self) {
        let id = {
            let live = self.live.read();
            let Some(card) = live.as_ref().and_then(|l| l.session.current()) else { return };
            match card.kind {
                CardKind::Meet | CardKind::ListenMeaning | CardKind::PickHindi | CardKind::Respond => card.phrase_id.clone(),
                CardKind::Build | CardKind::Recall => return,
            }
        };
        self.play_phrase(&id);
    }

    /// Start today's session, or with `bonus`, an extra practice round.
    pub fn start_session(mut self, bonus: bool) {
        let session = {
            let (content, progress) = (self.content.read(), self.progress.read());
            if bonus {
                Session::build_bonus(&content, &progress, platform::now_ms(), platform::seed())
            } else {
                Session::build(&content, &progress, platform::today(), platform::now_ms(), platform::seed())
            }
        };
        if session.is_empty() {
            self.show_toast("Nothing to practise right now.");
            return;
        }
        self.live.set(Some(Live::new(session)));
        self.go(Screen::Session);
        self.play_prompt();
    }

    /// Move on from the current card, to the summary if that was the last.
    pub fn next_card(mut self) {
        let done = {
            let mut live = self.live.write();
            let Some(l) = live.as_mut() else { return };
            l.session.advance(&self.progress.read(), platform::now_ms());
            l.reset_card();
            l.session.is_done()
        };
        if done {
            self.go(Screen::Summary);
        } else {
            self.play_prompt();
        }
    }

    pub fn quit_session(mut self) {
        let answered = self.live.read().as_ref().map(|l| l.session.graded).unwrap_or(0);
        self.live.set(None);
        self.go(Screen::Home);
        self.show_toast(if answered == 0 {
            "No problem. Come back any time.".to_string()
        } else {
            format!("Progress saved. {answered} card{} counted.", if answered == 1 { "" } else { "s" })
        });
    }
}

#[component]
fn Root() -> Element {
    let content = use_resource(platform::load_content);
    match &*content.read() {
        None => rsx! { div { class: "splash", "Baatcheet" } },
        Some(Err(e)) => rsx! {
            div { class: "error",
                h2 { "Couldn't load the phrases" }
                p { "{e}" }
                p { class: "muted", "Check your connection and reopen the app." }
            }
        },
        Some(Ok(c)) => rsx! { App { content: c.clone() } },
    }
}

#[component]
fn App(content: Content) -> Element {
    let ctx = Ctx {
        content: use_signal(|| content.clone()),
        progress: use_signal(platform::load_progress),
        settings: use_signal(platform::load_settings),
        screen: use_signal(|| Screen::Home),
        live: use_signal(|| None),
        toast: use_signal(|| None),
    };
    use_context_provider(|| ctx);
    let screen = *ctx.screen.read();
    let toast = ctx.toast.read().clone();

    rsx! {
        div { class: "app",
            match screen {
                Screen::Home => rsx! { home::Home {} },
                Screen::Book => rsx! { book::Book {} },
                Screen::Settings => rsx! { settings::SettingsScreen {} },
                Screen::Session => rsx! { session_view::SessionScreen {} },
                Screen::Summary => rsx! { session_view::Summary {} },
            }
            if let Some(msg) = toast {
                div { class: "toast", role: "status", "{msg}" }
            }
        }
    }
}
