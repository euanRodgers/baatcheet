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

/// Stems of the `.mp3` files in a directory.
pub fn mp3_stems(dir: &std::path::Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<String> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "mp3"))
        .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .collect();
    out.sort();
    out
}
