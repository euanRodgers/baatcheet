//! Everything that touches the browser: time, storage, audio, speech, files.

use baatcheet_core::content::word_clip_name;
use baatcheet_core::{Content, Date, Gender, Learner, Progress};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen_futures::JsFuture;
use web_sys::{HtmlAudioElement, SpeechSynthesisUtterance};

const PROGRESS_KEY: &str = "baatcheet.progress";
const SETTINGS_KEY: &str = "baatcheet.settings";
/// Progress that couldn't be read, kept so it can still be saved to a file.
const UNREADABLE_KEY: &str = "baatcheet.progress.unreadable";
/// The progress replaced by the last restore or reset, so it can be undone.
const PREVIOUS_KEY: &str = "baatcheet.progress.previous";

fn window() -> web_sys::Window {
    web_sys::window().expect("running in a browser")
}

// ---------- time ----------

pub fn now_ms() -> f64 {
    js_sys::Date::now()
}

pub fn today() -> Date {
    // getTimezoneOffset is minutes *behind* UTC, so flip the sign.
    let offset = -(js_sys::Date::new_0().get_timezone_offset() as i32);
    Date::from_unix_ms(now_ms(), offset)
}

pub fn seed() -> u64 {
    (js_sys::Math::random() * 9_007_199_254_740_991.0) as u64
}

pub async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let _ = window().set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = JsFuture::from(promise).await;
}

// ---------- content ----------

pub async fn load_content() -> Result<Content, String> {
    let text = fetch_text("phrases.json").await?;
    Content::from_json(&text).map_err(|e| format!("phrases.json is invalid: {e}"))
}

async fn fetch_text(url: &str) -> Result<String, String> {
    let resp = JsFuture::from(window().fetch_with_str(url)).await.map_err(|_| format!("Couldn't load {url}"))?;
    let resp: web_sys::Response = resp.dyn_into().map_err(|_| "Unexpected fetch result".to_string())?;
    if !resp.ok() {
        return Err(format!("Couldn't load {url} ({})", resp.status()));
    }
    let text = JsFuture::from(resp.text().map_err(|_| "No body")?).await.map_err(|_| "Couldn't read body")?;
    text.as_string().ok_or_else(|| "Body isn't text".to_string())
}

// ---------- storage ----------

fn storage() -> Option<web_sys::Storage> {
    window().local_storage().ok().flatten()
}

fn get(key: &str) -> Option<String> {
    storage()?.get_item(key).ok().flatten()
}

fn remove(key: &str) {
    if let Some(s) = storage() {
        let _ = s.remove_item(key);
    }
}

fn set(key: &str, value: &str) {
    if let Some(s) = storage() {
        let _ = s.set_item(key, value);
    }
}

pub fn load_progress() -> Progress {
    let Some(raw) = get(PROGRESS_KEY) else { return Progress::default() };
    match Progress::from_json(&raw) {
        Ok(p) => p,
        Err(_) => {
            // Keep the unreadable copy rather than silently losing it. The home
            // screen offers to save it to a file.
            set(UNREADABLE_KEY, &raw);
            Progress::default()
        }
    }
}

pub fn unreadable_progress() -> Option<String> {
    get(UNREADABLE_KEY)
}

pub fn forget_unreadable_progress() {
    remove(UNREADABLE_KEY);
}

/// Keep a copy of the current progress before a restore or reset replaces it.
pub fn keep_previous(progress: &Progress) {
    set(PREVIOUS_KEY, &progress.to_json());
}

pub fn previous_progress() -> Option<Progress> {
    get(PREVIOUS_KEY).and_then(|raw| Progress::from_json(&raw).ok())
}

pub fn forget_previous_progress() {
    remove(PREVIOUS_KEY);
}

pub fn save_progress(progress: &Progress) {
    set(PROGRESS_KEY, &progress.to_json());
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub show_deva: bool,
    pub slow: bool,
    pub last_backup: Option<Date>,
    /// Set on the welcome screen.
    pub name: String,
    pub gender: Option<Gender>,
}

impl Default for Settings {
    fn default() -> Self {
        Settings { show_deva: true, slow: false, last_backup: None, name: String::new(), gender: None }
    }
}

impl Settings {
    /// The learner, once the welcome screen has been filled in.
    pub fn learner(&self) -> Option<Learner> {
        let name = self.name.trim();
        match (name.is_empty(), self.gender) {
            (false, Some(gender)) => Some(Learner { name: name.to_string(), gender }),
            _ => None,
        }
    }
}

pub fn load_settings() -> Settings {
    get(SETTINGS_KEY).and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

pub fn save_settings(settings: &Settings) {
    if let Ok(json) = serde_json::to_string(settings) {
        set(SETTINGS_KEY, &json);
    }
}

/// Ask the browser not to clear our storage. Safari grants this to home-screen apps.
pub fn request_persistent_storage() {
    if let Ok(promise) = window().navigator().storage().persist() {
        wasm_bindgen_futures::spawn_local(async move {
            let _ = JsFuture::from(promise).await;
        });
    }
}

pub fn register_service_worker() {
    let host = window().location().hostname().unwrap_or_default();
    // Not during development: a cached build would hide your changes.
    if host == "localhost" || host == "127.0.0.1" {
        return;
    }
    let _ = window().navigator().service_worker().register("sw.js");
}

// ---------- backup files ----------

#[wasm_bindgen(inline_js = r#"
export async function saveBackup(name, json) {
  const file = new File([json], name, { type: 'application/json' });
  // The share sheet works in a home-screen app, where plain downloads often don't.
  if (navigator.canShare && navigator.canShare({ files: [file] })) {
    try {
      await navigator.share({ files: [file], title: 'Baatcheet backup' });
      return 'saved';
    } catch (e) {
      return e && e.name === 'AbortError' ? 'cancelled' : 'failed';
    }
  }
  const url = URL.createObjectURL(file);
  const a = document.createElement('a');
  a.href = url;
  a.download = name;
  document.body.appendChild(a);
  a.click();
  a.remove();
  // Revoking straight away can cancel the download in Safari.
  setTimeout(() => URL.revokeObjectURL(url), 30000);
  return 'downloaded';
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = saveBackup)]
    fn save_backup_js(name: &str, json: &str) -> js_sys::Promise;
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BackupResult {
    /// Shared to Files, Mail, etc.
    Saved,
    /// Handed to the browser as a download. We can't tell if it finished.
    Downloaded,
    Cancelled,
    Failed,
}

/// Offer a JSON file through the share sheet, or as a download where that isn't available.
pub async fn save_backup(name: &str, json: &str) -> BackupResult {
    match JsFuture::from(save_backup_js(name, json)).await.ok().and_then(|v| v.as_string()).as_deref() {
        Some("saved") => BackupResult::Saved,
        Some("downloaded") => BackupResult::Downloaded,
        Some("cancelled") => BackupResult::Cancelled,
        _ => BackupResult::Failed,
    }
}

/// Text of the first file chosen in the `<input type=file>` with this id.
pub async fn read_chosen_file(input_id: &str) -> Option<String> {
    let input: web_sys::HtmlInputElement = window().document()?.get_element_by_id(input_id)?.dyn_into().ok()?;
    let file = input.files()?.get(0)?;
    let text = JsFuture::from(file.text()).await.ok()?.as_string();
    input.set_value("");
    text
}

// ---------- audio ----------

thread_local! {
    /// One shared player. iOS only lets audio play after a tap, and reusing the
    /// element that played during a tap keeps later clips working.
    static PLAYER: RefCell<Option<HtmlAudioElement>> = const { RefCell::new(None) };
}

fn with_player(f: impl FnOnce(&HtmlAudioElement)) {
    PLAYER.with(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_none() {
            *slot = HtmlAudioElement::new().ok();
        }
        if let Some(el) = slot.as_ref() {
            f(el);
        }
    });
}

/// Play a phrase: its recorded clip if there is one, otherwise the device's Hindi voice.
pub fn play_phrase(content: &Content, id: &str, slow: bool) {
    let Some(p) = content.phrase(id) else { return };
    if p.audio {
        play_clip(&format!("audio/{}.mp3", p.clip_name()), &p.deva, slow);
    } else {
        speak(&p.deva, slow);
    }
}

/// Play one word tile.
pub fn play_word(content: &Content, deva: &str, slow: bool) {
    if content.has_word_audio(deva) {
        play_clip(&format!("audio/w/{}.mp3", word_clip_name(deva)), deva, slow);
    } else {
        speak(deva, slow);
    }
}

fn play_clip(url: &str, fallback: &str, slow: bool) {
    stop_speech();
    let fallback = fallback.to_string();
    with_player(|el| {
        el.set_src(url);
        el.set_playback_rate(if slow { 0.75 } else { 1.0 });
        if let Ok(promise) = el.play() {
            wasm_bindgen_futures::spawn_local(async move {
                if JsFuture::from(promise).await.is_err() {
                    speak(&fallback, slow);
                }
            });
        }
    });
}

fn stop_speech() {
    if let Ok(synth) = window().speech_synthesis() {
        synth.cancel();
    }
}

fn hindi_voice() -> Option<web_sys::SpeechSynthesisVoice> {
    let synth = window().speech_synthesis().ok()?;
    synth
        .get_voices()
        .iter()
        .filter_map(|v| v.dyn_into::<web_sys::SpeechSynthesisVoice>().ok())
        .find(|v| {
            let lang = v.lang().to_lowercase();
            lang == "hi" || lang.starts_with("hi-") || lang.starts_with("hi_")
        })
}

/// Name of the device's Hindi voice, if it has one. `None` can also mean the
/// voice list hasn't loaded yet.
pub fn hindi_voice_name() -> Option<String> {
    hindi_voice().map(|v| v.name())
}

pub fn speak(text: &str, slow: bool) {
    with_player(|el| {
        let _ = el.pause();
    });
    let Ok(synth) = window().speech_synthesis() else { return };
    synth.cancel();
    let Ok(u) = SpeechSynthesisUtterance::new_with_text(text) else { return };
    u.set_lang("hi-IN");
    if let Some(v) = hindi_voice() {
        u.set_voice(Some(&v));
    }
    u.set_rate(if slow { 0.6 } else { 0.9 });
    synth.speak(&u);
}
