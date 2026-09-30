//! Generate MP3s with ElevenLabs for phrases and word tiles that don't have one yet.
//!
//!     cargo run -p baatcheet-tools --bin gen-audio [-- --dry-run]
//!
//! Always sends Devanagari, never romanised text. Reads ELEVENLABS_API_KEY and
//! ELEVENLABS_VOICE_ID from the environment or from `.env` in the workspace root.
//!
//! Options:
//!   --dry-run          list what would be generated and the character count
//!   --only id,id       just these phrases (regenerates them even if they exist)
//!   --no-words         skip word tile clips
//!   --model <id>       default eleven_multilingual_v2
//!
//! Run build-content afterwards so the app knows the clips exist.

use baatcheet_core::content::{Content, normalize_word, word_clip_name};
use baatcheet_tools::{flag_value, has_flag, path_arg, workspace_root};
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::ExitCode;

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
    let model = flag_value(&args, "--model").unwrap_or_else(|| "eleven_multilingual_v2".into());
    let only: Option<Vec<String>> =
        flag_value(&args, "--only").map(|s| s.split(',').map(|x| x.trim().to_string()).collect());

    let content = match std::fs::read_to_string(&json_path).map_err(|e| e.to_string()).and_then(|s| Content::from_json(&s).map_err(|e| e.to_string())) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Can't load {}: {e}\nRun build-content first.", json_path.display());
            return ExitCode::FAILURE;
        }
    };

    let mut jobs: Vec<Job> = Vec::new();
    for p in &content.phrases {
        let path = audio_dir.join(format!("{}.mp3", p.id));
        let wanted = match &only {
            Some(ids) => ids.contains(&p.id),
            None => !path.exists(),
        };
        if wanted {
            jobs.push(Job { label: format!("{} {}", p.id, p.roman), text: p.deva.clone(), path });
        }
    }
    if words && only.is_none() {
        // One clip per distinct Devanagari word, shared by every phrase that uses it.
        let mut distinct: BTreeMap<String, String> = BTreeMap::new();
        for p in &content.phrases {
            for (r, d) in p.tiles.iter().zip(&p.deva_tiles) {
                distinct.entry(normalize_word(d)).or_insert_with(|| normalize_word(r));
            }
        }
        for (deva, roman) in distinct {
            let path = audio_dir.join("w").join(format!("{}.mp3", word_clip_name(&deva)));
            if !path.exists() {
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

    let (Some(key), Some(voice)) = (env_value("ELEVENLABS_API_KEY"), env_value("ELEVENLABS_VOICE_ID")) else {
        eprintln!("Set ELEVENLABS_API_KEY and ELEVENLABS_VOICE_ID in .env (see .env.example).");
        return ExitCode::FAILURE;
    };
    let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{voice}?output_format=mp3_44100_128");
    let _ = std::fs::create_dir_all(audio_dir.join("w"));

    let mut failed = 0;
    for j in &jobs {
        let body = serde_json::json!({ "text": j.text, "model_id": model }).to_string();
        let result = ureq::post(&url)
            .header("xi-api-key", &key)
            .header("content-type", "application/json")
            .header("accept", "audio/mpeg")
            .send(body)
            .map_err(|e| e.to_string())
            .and_then(|mut resp| resp.body_mut().read_to_vec().map_err(|e| e.to_string()))
            .and_then(|bytes| std::fs::write(&j.path, bytes).map_err(|e| e.to_string()));
        match result {
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
