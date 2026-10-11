//! Shared helpers for the content tools.

use std::path::PathBuf;

/// The value after `--name`, if given.
pub fn flag_value(args: &[String], name: &str) -> Option<String> {
    args.iter().position(|a| a == name).and_then(|i| args.get(i + 1)).cloned()
}

pub fn has_flag(args: &[String], name: &str) -> bool {
    args.iter().any(|a| a == name)
}

/// A path from `--name`, or the default relative to the workspace root.
pub fn path_arg(args: &[String], name: &str, default: &str) -> PathBuf {
    flag_value(args, name).map(PathBuf::from).unwrap_or_else(|| workspace_root().join(default))
}

pub fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).parent().unwrap().to_path_buf()
}

/// Every audio file (`.mp3` or `.m4a`) under `dir`, as paths relative to it
/// with `/` separators, e.g. `tts/w/1a2b….m4a`.
pub fn audio_files(dir: &std::path::Path) -> std::collections::BTreeSet<String> {
    fn walk(base: &std::path::Path, dir: &std::path::Path, out: &mut std::collections::BTreeSet<String>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                walk(base, &path, out);
            } else if path.extension().is_some_and(|x| x == "mp3" || x == "m4a") {
                let rel = path.strip_prefix(base).unwrap().to_string_lossy().replace('\\', "/");
                out.insert(rel);
            }
        }
    }
    let mut out = std::collections::BTreeSet::new();
    walk(dir, dir, &mut out);
    out
}

/// The best clip for a file stem, relative to the audio folder. A recording
/// (a person or ElevenLabs, in `audio/`) beats the Mac's voice (`audio/tts/`).
/// `sub` is `""` for phrases or `"w/"` for words.
pub fn find_clip(files: &std::collections::BTreeSet<String>, sub: &str, stem: &str) -> String {
    [
        format!("{sub}{stem}.mp3"),
        format!("{sub}{stem}.m4a"),
        format!("tts/{sub}{stem}.m4a"),
        format!("tts/{sub}{stem}.mp3"),
    ]
    .into_iter()
    .find(|candidate| files.contains(candidate))
    .unwrap_or_default()
}
