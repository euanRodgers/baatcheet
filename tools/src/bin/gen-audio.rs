//! Generate audio for phrases and words that don't have a clip yet.
//!
//!     cargo run -p baatcheet-tools --bin gen-audio -- --engine say [--dry-run]
//!
//! Two engines, both always given Devanagari, never romanised text:
//!   say         the Mac's own Hindi voice (Lekha), free and offline. Writes
//!               `audio/tts/…m4a`, used until a better recording exists.
//!   elevenlabs  ElevenLabs (the default). Writes `audio/…mp3`, which replaces
//!               any `tts` clip. Reads ELEVENLABS_API_KEY and ELEVENLABS_VOICE_ID
//!               from the environment or `.env` in the workspace root.
//!
//! Options:
//!   --dry-run          list what would be generated and the character count
//!   --only id,id       just these phrases (regenerates them even if they exist)
//!   --no-words         skip word clips
//!   --voice <name>     for `say`, default Lekha
//!   --model <id>       for ElevenLabs, default eleven_multilingual_v2
//!
//! Run build-content afterwards so the app knows the clips exist.

use baatcheet_core::content::{Content, NAME, normalize_word, word_clip_name};
use baatcheet_tools::{audio_files, find_clip, flag_value, has_flag, path_arg, workspace_root};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

fn env_value(name: &str) -> Option<String> {
    if let Ok(v) = std::env::var(name) {
        return Some(v);
    }
    let dotenv = std::fs::read_to_string(workspace_root().join(".env")).ok()?;
    dotenv.lines().find_map(|line| {
        let (k, v) = line.split_once('=')?;
        (k.trim() == name).then(|| v.trim().trim_matches('"').to_string())
    })
}

#[derive(Clone, Copy, PartialEq)]
enum Engine {
    Say,
    ElevenLabs,
}

impl Engine {
    /// Where this engine writes a clip, relative to the audio folder.
    fn path(self, sub: &str, stem: &str) -> String {
        match self {
            Engine::Say => format!("tts/{sub}{stem}.m4a"),
            Engine::ElevenLabs => format!("{sub}{stem}.mp3"),
        }
    }

    /// Whether a clip already exists that's at least as good as this engine's.
    fn has_clip(self, files: &std::collections::BTreeSet<String>, sub: &str, stem: &str) -> bool {
        let best = find_clip(files, sub, stem);
        match self {
            // Any clip at all is as good as the Mac's voice.
            Engine::Say => !best.is_empty(),
            // A tts clip should be replaced by ElevenLabs.
            Engine::ElevenLabs => !best.is_empty() && !best.starts_with("tts/"),
        }
    }
}

struct Job {
    label: String,
    text: String,
    path: PathBuf,
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let json_path = path_arg(&args, "--content", "app/public/phrases.json");
    let audio_dir = path_arg(&args, "--audio", "app/public/audio");
    let dry_run = has_flag(&args, "--dry-run");
    let words = !has_flag(&args, "--no-words");
    let engine = match flag_value(&args, "--engine").as_deref() {
        None | Some("elevenlabs") => Engine::ElevenLabs,
        Some("say") => Engine::Say,
        Some(other) => {
            eprintln!("Unknown engine `{other}`. Use `say` or `elevenlabs`.");
            return ExitCode::FAILURE;
        }
    };
    let only: Option<Vec<String>> =
        flag_value(&args, "--only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());

    let content = match std::fs::read_to_string(&json_path)
        .map_err(|e| e.to_string())
        .and_then(|s| Content::from_json(&s).map_err(|e| e.to_string()))
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Can't load {}: {e}\nRun build-content first.", json_path.display());
            return ExitCode::FAILURE;
        }
    };
    let files = audio_files(&audio_dir);

    let mut jobs: Vec<Job> = Vec::new();
    // Phrases with the learner's name can't be pre-recorded; the app speaks them.
    for p in content.phrases.iter().filter(|p| !p.is_personal()) {
        let wanted = |stem: &str| match &only {
            Some(ids) => ids.contains(&p.id),
            None => !engine.has_clip(&files, "", stem),
        };
        if wanted(&p.id) {
            let path = audio_dir.join(engine.path("", &p.id));
            jobs.push(Job { label: format!("{} {}", p.id, p.roman), text: p.deva.clone(), path });
        }
        if let Some(f) = &p.female {
            let stem = format!("{}-f", p.id);
            if wanted(&stem) {
                let path = audio_dir.join(engine.path("", &stem));
                jobs.push(Job { label: format!("{stem} {}", f.roman), text: f.deva.clone(), path });
            }
        }
    }
    if words && only.is_none() {
        // One clip per distinct Devanagari word, shared by every phrase that uses it.
        let mut distinct: BTreeMap<String, String> = BTreeMap::new();
        for p in &content.phrases {
            let female = p.female.iter().flat_map(|f| f.tiles.iter().zip(&f.deva_tiles));
            for (r, d) in p.tiles.iter().zip(&p.deva_tiles).chain(female) {
                if !d.contains(NAME) {
                    distinct.entry(normalize_word(d)).or_insert_with(|| normalize_word(r));
                }
            }
        }
        for (deva, roman) in distinct {
            let stem = word_clip_name(&deva);
            if !engine.has_clip(&files, "w/", &stem) {
                let path = audio_dir.join(engine.path("w/", &stem));
                jobs.push(Job { label: format!("word {roman}"), text: deva, path });
            }
        }
    }

    let chars: usize = jobs.iter().map(|j| j.text.chars().count()).sum();
    println!("{} clips to generate, {chars} characters.", jobs.len());
    if dry_run || jobs.is_empty() {
        for j in &jobs {
            println!("  {}  ({})", j.label, j.text);
        }
        return ExitCode::SUCCESS;
    }

    let generate: Box<dyn Fn(&Job) -> Result<(), String>> = match engine {
        Engine::Say => {
            let voice = flag_value(&args, "--voice").unwrap_or_else(|| "Lekha".into());
            Box::new(move |j: &Job| say(&voice, &j.text, &j.path))
        }
        Engine::ElevenLabs => {
            let (Some(key), Some(voice)) = (env_value("ELEVENLABS_API_KEY"), env_value("ELEVENLABS_VOICE_ID")) else {
                eprintln!("Set ELEVENLABS_API_KEY and ELEVENLABS_VOICE_ID in .env (see .env.example),");
                eprintln!("or use `--engine say` for the Mac's own Hindi voice.");
                return ExitCode::FAILURE;
            };
            let model = flag_value(&args, "--model").unwrap_or_else(|| "eleven_multilingual_v2".into());
            Box::new(move |j: &Job| elevenlabs(&key, &voice, &model, &j.text, &j.path))
        }
    };

    let mut failed = 0;
    for j in &jobs {
        if let Some(dir) = j.path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        match generate(j) {
            Ok(()) => println!("  ok    {}", j.label),
            Err(e) => {
                failed += 1;
                eprintln!("  fail  {}: {e}", j.label);
            }
        }
    }
    println!("Done. {} generated, {failed} failed. Now run build-content.", jobs.len() - failed);
    if failed > 0 { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}

/// The Mac's `say` writes AIFF; `afconvert` turns it into a small mono AAC file.
fn say(voice: &str, text: &str, out: &Path) -> Result<(), String> {
    let aiff = out.with_extension("aiff");
    let status = Command::new("say")
        .args(["-v", voice, "-r", "165", "-o"])
        .arg(&aiff)
        .arg("--")
        .arg(text)
        .status()
        .map_err(|e| format!("couldn't run say: {e}"))?;
    if !status.success() {
        return Err(format!("say failed. Is the {voice} voice installed?"));
    }
    let status = Command::new("afconvert")
        .args(["-f", "m4af", "-d", "aac", "-b", "48000", "-c", "1"])
        .arg(&aiff)
        .arg(out)
        .status()
        .map_err(|e| format!("couldn't run afconvert: {e}"));
    let _ = std::fs::remove_file(&aiff);
    if status?.success() { Ok(()) } else { Err("afconvert failed".into()) }
}

fn elevenlabs(key: &str, voice: &str, model: &str, text: &str, out: &Path) -> Result<(), String> {
    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=mp3_44100_128");
    let body = serde_json::json!({ "text": text, "model_id": model }).to_string();
    let bytes = ureq::post(&url)
        .header("xi-api-key", key)
        .header("content-type", "application/json")
        .header("accept", "audio/mpeg")
        .send(body)
        .map_err(|e| e.to_string())?
        .body_mut()
        .read_to_vec()
        .map_err(|e| e.to_string())?;
    std::fs::write(out, bytes).map_err(|e| e.to_string())
}
