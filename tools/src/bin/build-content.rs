//! Turn the spreadsheet export into `public/phrases.json`, checking it on the way.
//!
//!     cargo run -p baatcheet-tools --bin build-content [-- --approved-only --strict]
//!
//! Options: --csv <path>  --out <path>  --audio <dir>
//!   --approved-only  leave out draft phrases
//!   --strict         treat missing audio as an error, not a warning

use baatcheet_core::content::{Content, Direction, Phrase, Priority, Status, PACKS, normalize_word, roman_words};
use baatcheet_tools::{has_flag, mp3_stems, path_arg};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::process::ExitCode;

/// One row of `content/phrases.csv`. Tiles are the words of `roman` and `deva`
/// unless the optional `tiles` / `deva_tiles` columns (split on `|`) say otherwise.
#[derive(Deserialize)]
struct Row {
    id: String,
    pack: String,
    priority: Priority,
    direction: Direction,
    roman: String,
    deva: String,
    english: String,
    #[serde(default)]
    notes: String,
    #[serde(default)]
    context: String,
    #[serde(default)]
    replies: String,
    status: Status,
    #[serde(default)]
    tiles: String,
    #[serde(default)]
    deva_tiles: String,
}

fn split_tiles(explicit: &str, text: &str) -> Vec<String> {
    if explicit.trim().is_empty() {
        text.split_whitespace().map(String::from).collect()
    } else {
        explicit.split('|').map(|t| t.trim().to_string()).filter(|t| !t.is_empty()).collect()
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let csv_path = path_arg(&args, "--csv", "content/phrases.csv");
    let out_path = path_arg(&args, "--out", "app/public/phrases.json");
    let audio_dir = path_arg(&args, "--audio", "app/public/audio");
    let approved_only = has_flag(&args, "--approved-only");
    let strict = has_flag(&args, "--strict");

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let mut reader = match csv::Reader::from_path(&csv_path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("Can't read {}: {e}", csv_path.display());
            return ExitCode::FAILURE;
        }
    };

    let mut phrases: Vec<Phrase> = Vec::new();
    for (i, row) in reader.deserialize::<Row>().enumerate() {
        let line = i + 2; // header is line 1
        let row = match row {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!("line {line}: {e}"));
                continue;
            }
        };
        if approved_only && row.status == Status::Draft {
            continue;
        }
        phrases.push(Phrase {
            tiles: split_tiles(&row.tiles, &row.roman),
            deva_tiles: split_tiles(&row.deva_tiles, &row.deva),
            replies: row.replies.split(';').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect(),
            id: row.id.trim().to_string(),
            pack: row.pack.trim().to_string(),
            priority: row.priority,
            direction: row.direction,
            context: row.context.trim().to_string(),
            roman: row.roman.trim().to_string(),
            deva: row.deva.trim().to_string(),
            english: row.english.trim().to_string(),
            notes: row.notes.trim().to_string(),
            status: row.status,
            audio: false,
        });
    }

    // 1. IDs are unique, look like `pack-000`, and the pack exists.
    let mut seen = HashSet::new();
    for p in &phrases {
        if !seen.insert(p.id.as_str()) {
            errors.push(format!("{}: duplicate id", p.id));
        }
        let well_formed = p.id.split_once('-').is_some_and(|(pack, num)| {
            pack == p.pack && num.len() == 3 && num.chars().all(|c| c.is_ascii_digit())
        });
        if !well_formed {
            errors.push(format!("{}: id should be `{}-000` style", p.id, p.pack));
        }
        if !PACKS.iter().any(|k| k.id == p.pack) {
            let known: Vec<&str> = PACKS.iter().map(|k| k.id).collect();
            errors.push(format!("{}: unknown pack `{}` (known: {})", p.id, p.pack, known.join(", ")));
        }
        if p.roman.is_empty() || p.deva.is_empty() || p.english.is_empty() {
            errors.push(format!("{}: roman, deva and english are all required", p.id));
        }
    }

    // 2. Tiles spell the phrase.
    for p in &phrases {
        if roman_words(&p.tiles.join(" ")) != roman_words(&p.roman) {
            errors.push(format!("{}: tiles {:?} don't spell \"{}\"", p.id, p.tiles, p.roman));
        }
    }

    // 3. Every romanised tile has a Devanagari partner.
    for p in &phrases {
        if p.tiles.len() != p.deva_tiles.len() {
            errors.push(format!(
                "{}: {} romanised words but {} Devanagari words. Add tiles/deva_tiles columns split with |",
                p.id,
                p.tiles.len(),
                p.deva_tiles.len()
            ));
        }
    }

    // 4. Each Devanagari word has one romanised spelling across the whole file.
    let mut spellings: BTreeMap<String, BTreeMap<String, Vec<String>>> = BTreeMap::new();
    for p in &phrases {
        for (r, d) in p.tiles.iter().zip(&p.deva_tiles) {
            spellings
                .entry(normalize_word(d))
                .or_default()
                .entry(normalize_word(r))
                .or_default()
                .push(p.id.clone());
        }
    }
    for (deva, romans) in &spellings {
        if romans.len() > 1 {
            let detail: Vec<String> = romans.iter().map(|(r, ids)| format!("\"{r}\" in {}", ids.join(", "))).collect();
            errors.push(format!("{deva} is spelled more than one way: {}", detail.join("; ")));
        }
    }

    // 5. Replies exist and are things Euan says.
    for p in &phrases {
        for reply in &p.replies {
            match phrases.iter().find(|q| &q.id == reply) {
                None => errors.push(format!("{}: reply {reply} doesn't exist", p.id)),
                Some(q) if !q.direction.is_said() => {
                    errors.push(format!("{}: reply {reply} is hear-only, so Euan can't say it", p.id))
                }
                _ => {}
            }
        }
    }

    // 6. Audio: approved phrases have a clip, and no clip is orphaned.
    let clips = mp3_stems(&audio_dir);
    for p in phrases.iter_mut() {
        p.audio = clips.contains(&p.id);
        if !p.audio && p.status != Status::Draft {
            let msg = format!("{}: approved but has no audio/{}.mp3", p.id, p.id);
            if strict { errors.push(msg) } else { warnings.push(msg) }
        }
    }
    for clip in &clips {
        if !phrases.iter().any(|p| &p.id == clip) {
            warnings.push(format!("audio/{clip}.mp3 doesn't match any phrase"));
        }
    }
    let word_audio = mp3_stems(&audio_dir.join("w"));

    for w in &warnings {
        eprintln!("warning: {w}");
    }
    if !errors.is_empty() {
        for e in &errors {
            eprintln!("error: {e}");
        }
        eprintln!("\n{} problem(s) in {}. Nothing written.", errors.len(), csv_path.display());
        return ExitCode::FAILURE;
    }

    let content = Content { version: 1, phrases, word_audio };
    let json = serde_json::to_string_pretty(&content).expect("content serialises");
    if let Err(e) = std::fs::write(&out_path, json + "\n") {
        eprintln!("Can't write {}: {e}", out_path.display());
        return ExitCode::FAILURE;
    }

    let say = content.phrases.iter().filter(|p| p.direction.is_said()).count();
    let hear = content.phrases.iter().filter(|p| p.direction.is_heard()).count();
    let with_audio = content.phrases.iter().filter(|p| p.audio).count();
    println!(
        "Wrote {} phrases ({say} to say, {hear} to hear, {with_audio} with audio, {} word clips) to {}",
        content.phrases.len(),
        content.word_audio.len(),
        out_path.display()
    );
    for pack in &PACKS {
        let n = content.in_pack(pack.id).count();
        if n > 0 {
            println!("  {:<22} {n}", pack.name);
        }
    }
    ExitCode::SUCCESS
}
