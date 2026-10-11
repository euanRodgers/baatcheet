//! Turn the spreadsheet export into `public/phrases.json`, checking it on the way.
//!
//!     cargo run -p baatcheet-tools --bin build-content [-- --approved-only --strict]
//!
//! Options: --csv <path>  --words <path>  --out <path>  --audio <dir>
//!   --approved-only  leave out draft phrases
//!   --strict         treat missing audio as an error, not a warning

use baatcheet_core::content::{
    Content, Direction, Form, Line, NAME, PACKS, Phrase, Priority, Scene, Status, Word, normalize_word, roman_words,
    word_clip_name,
};
use baatcheet_tools::{audio_files, find_clip, has_flag, path_arg};
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::process::ExitCode;

/// One row of `content/phrases.csv`. Tiles are the words of `roman` and `deva`
/// unless the optional `tiles` / `deva_tiles` columns (split on `|`) say otherwise.
/// The optional `*_f` columns hold the version for a woman learner.
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
    #[serde(default)]
    roman_f: String,
    #[serde(default)]
    deva_f: String,
    #[serde(default)]
    english_f: String,
    #[serde(default)]
    notes_f: String,
}

/// One row of `content/words.csv`: the glossary.
#[derive(Deserialize)]
struct WordRow {
    deva: String,
    roman: String,
    meaning: String,
    #[serde(default)]
    note: String,
}

/// One row of `content/scenes.csv`. A scene's title and intro go on its first row.
#[derive(Deserialize)]
struct SceneRow {
    scene: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    intro: String,
    speaker: String,
    phrase: String,
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
    let words_path = path_arg(&args, "--words", "content/words.csv");
    let scenes_path = path_arg(&args, "--scenes", "content/scenes.csv");
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
        let female = if row.roman_f.trim().is_empty() && row.deva_f.trim().is_empty() {
            None
        } else {
            if row.roman_f.trim().is_empty() || row.deva_f.trim().is_empty() {
                errors.push(format!("{}: a woman's form needs both roman_f and deva_f", row.id.trim()));
            }
            Some(Form {
                roman: row.roman_f.trim().to_string(),
                deva: row.deva_f.trim().to_string(),
                english: if row.english_f.trim().is_empty() { row.english.trim() } else { row.english_f.trim() }.to_string(),
                notes: row.notes_f.trim().to_string(),
                tiles: split_tiles("", &row.roman_f),
                deva_tiles: split_tiles("", &row.deva_f),
                audio: String::new(),
            })
        };
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
            audio: String::new(),
            female,
            female_form: false,
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

    // Checks 2–4 cover the woman's forms too.
    struct Words<'a> {
        id: String,
        roman: &'a str,
        tiles: &'a [String],
        deva_tiles: &'a [String],
    }
    let mut forms: Vec<Words> = Vec::new();
    for p in &phrases {
        forms.push(Words { id: p.id.clone(), roman: &p.roman, tiles: &p.tiles, deva_tiles: &p.deva_tiles });
        if let Some(f) = &p.female {
            forms.push(Words { id: format!("{} (woman's form)", p.id), roman: &f.roman, tiles: &f.tiles, deva_tiles: &f.deva_tiles });
        }
    }

    // 2. Tiles spell the phrase.
    for p in &forms {
        if roman_words(&p.tiles.join(" ")) != roman_words(p.roman) {
            errors.push(format!("{}: tiles {:?} don't spell \"{}\"", p.id, p.tiles, p.roman));
        }
    }

    // 3. Every romanised tile has a Devanagari partner.
    for p in &forms {
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
    for p in &forms {
        for (r, d) in p.tiles.iter().zip(p.deva_tiles) {
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

    // 5. Replies exist and are things the learner says.
    for p in &phrases {
        for reply in &p.replies {
            match phrases.iter().find(|q| &q.id == reply) {
                None => errors.push(format!("{}: reply {reply} doesn't exist", p.id)),
                Some(q) if !q.direction.is_said() => {
                    errors.push(format!("{}: reply {reply} is hear-only, so the learner can't say it", p.id))
                }
                _ => {}
            }
        }
    }

    // 7. The glossary: every word used has a meaning, spelled the same way.
    let mut words: Vec<Word> = Vec::new();
    match csv::Reader::from_path(&words_path) {
        Err(e) => warnings.push(format!("no glossary ({}): {e}", words_path.display())),
        Ok(mut reader) => {
            for (i, row) in reader.deserialize::<WordRow>().enumerate() {
                match row {
                    Ok(r) => words.push(Word {
                        deva: normalize_word(&r.deva),
                        roman: r.roman.trim().to_string(),
                        meaning: r.meaning.trim().to_string(),
                        note: r.note.trim().to_string(),
                        audio: String::new(),
                    }),
                    Err(e) => errors.push(format!("{} line {}: {e}", words_path.display(), i + 2)),
                }
            }
        }
    }
    let mut seen_words = HashSet::new();
    for w in &words {
        if !seen_words.insert(w.deva.as_str()) {
            errors.push(format!("{} is in the glossary twice", w.deva));
        }
    }
    let mut missing: Vec<String> = Vec::new();
    for (deva, romans) in &spellings {
        if deva.contains(NAME) {
            continue;
        }
        match words.iter().find(|w| &w.deva == deva) {
            None => missing.push(format!("{deva} ({})", romans.keys().next().unwrap())),
            Some(w) => {
                let glossary = normalize_word(&w.roman);
                if let Some((roman, ids)) = romans.iter().find(|(r, _)| **r != glossary) {
                    errors.push(format!(
                        "{deva} is \"{}\" in the glossary but \"{roman}\" in {}",
                        w.roman,
                        ids.join(", ")
                    ));
                }
            }
        }
    }
    if !missing.is_empty() {
        warnings.push(format!("{} word(s) have no meaning in the glossary: {}", missing.len(), missing.join(", ")));
    }
    let unused: Vec<&str> = words.iter().filter(|w| !spellings.contains_key(&w.deva)).map(|w| w.deva.as_str()).collect();
    if !unused.is_empty() {
        warnings.push(format!("glossary words no phrase uses: {}", unused.join(", ")));
    }

    // 8. Scenes: every line is a real phrase, and the learner only says things
    // they could say.
    let mut scenes: Vec<Scene> = Vec::new();
    if let Ok(mut reader) = csv::Reader::from_path(&scenes_path) {
        for (i, row) in reader.deserialize::<SceneRow>().enumerate() {
            let row = match row {
                Ok(r) => r,
                Err(e) => {
                    errors.push(format!("{} line {}: {e}", scenes_path.display(), i + 2));
                    continue;
                }
            };
            let id = row.scene.trim().to_string();
            if scenes.last().is_none_or(|s| s.id != id) {
                if scenes.iter().any(|s| s.id == id) {
                    errors.push(format!("scene {id}: its rows must be together"));
                }
                if row.title.trim().is_empty() {
                    errors.push(format!("scene {id}: the first row needs a title"));
                }
                scenes.push(Scene { id: id.clone(), title: row.title.trim().into(), intro: row.intro.trim().into(), lines: vec![] });
            }
            let line = Line { speaker: row.speaker.trim().into(), phrase: row.phrase.trim().into() };
            match phrases.iter().find(|p| p.id == line.phrase) {
                None => errors.push(format!("scene {id}: phrase {} doesn't exist", line.phrase)),
                Some(p) if line.is_yours() && !p.direction.is_said() => {
                    errors.push(format!("scene {id}: {} is hear-only, so it can't be the learner's line", p.id))
                }
                _ => {}
            }
            scenes.last_mut().unwrap().lines.push(line);
        }
    }

    // 6. Audio: approved phrases have a clip (a woman's form has its own,
    // `{id}-f`), and no clip is orphaned. Phrases with the learner's name
    // can't be recorded, so they always use the device voice.
    let files = audio_files(&audio_dir);
    let mut used_files: HashSet<String> = HashSet::new();
    for p in phrases.iter_mut() {
        if p.is_personal() {
            continue;
        }
        p.audio = find_clip(&files, "", &p.id);
        let mut missing = vec![];
        if p.audio.is_empty() {
            missing.push(format!("{}.mp3", p.id));
        }
        used_files.insert(p.audio.clone());
        if let Some(f) = p.female.as_mut() {
            let stem = format!("{}-f", p.id);
            f.audio = find_clip(&files, "", &stem);
            if f.audio.is_empty() {
                missing.push(format!("{stem}.mp3"));
            }
            used_files.insert(f.audio.clone());
        }
        if !missing.is_empty() && p.status != Status::Draft {
            let msg = format!("{}: approved but has no audio ({})", p.id, missing.join(" or "));
            if strict { errors.push(msg) } else { warnings.push(msg) }
        }
    }
    for w in words.iter_mut() {
        w.audio = find_clip(&files, "w/", &word_clip_name(&w.deva));
        used_files.insert(w.audio.clone());
    }
    let orphans: Vec<&String> = files.iter().filter(|f| !used_files.contains(*f)).collect();
    if !orphans.is_empty() {
        warnings.push(format!(
            "{} audio file(s) match no phrase or word (or a recording replaces them): {}",
            orphans.len(),
            orphans.iter().take(5).map(|s| s.as_str()).collect::<Vec<_>>().join(", ")
        ));
    }

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

    let content = Content { version: 1, phrases, words, scenes };
    let json = serde_json::to_string_pretty(&content).expect("content serialises");
    if let Err(e) = std::fs::write(&out_path, json + "\n") {
        eprintln!("Can't write {}: {e}", out_path.display());
        return ExitCode::FAILURE;
    }

    let say = content.phrases.iter().filter(|p| p.direction.is_said()).count();
    let hear = content.phrases.iter().filter(|p| p.direction.is_heard()).count();
    let with_audio = content.phrases.iter().filter(|p| !p.audio.is_empty()).count();
    let word_clips = content.words.iter().filter(|w| !w.audio.is_empty()).count();
    println!(
        "Wrote {} phrases ({say} to say, {hear} to hear, {with_audio} with audio), {} words ({word_clips} with audio) and {} scenes to {}",
        content.phrases.len(),
        content.words.len(),
        content.scenes.len(),
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
