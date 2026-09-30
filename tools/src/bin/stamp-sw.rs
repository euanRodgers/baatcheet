//! After a release build, write the build's file list and a content hash into
//! its `sw.js`, so the service worker precaches everything and gets a new cache
//! whenever any file changes.
//!
//!     cargo run -p baatcheet-tools --bin stamp-sw [-- <bundle dir>]
//!
//! The bundle dir defaults to target/dx/baatcheet/release/web/public.

use baatcheet_tools::workspace_root;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

fn files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, out);
        } else {
            out.push(path);
        }
    }
}

fn main() -> ExitCode {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| workspace_root().join("target/dx/baatcheet/release/web/public"));
    let sw_path = dir.join("sw.js");
    let Ok(sw) = std::fs::read_to_string(&sw_path) else {
        eprintln!("No sw.js in {}. Run `dx bundle --platform web --release` first.", dir.display());
        return ExitCode::FAILURE;
    };

    let mut all = Vec::new();
    files(&dir, &mut all);
    all.sort();

    // dx leaves older builds' hashed files in assets/. Keep only the ones this
    // build uses: named in index.html, or in a script that index.html loads.
    let index = std::fs::read_to_string(dir.join("index.html")).unwrap_or_default();
    let mut referenced = index.clone();
    for path in &all {
        let name = path.file_name().unwrap().to_string_lossy();
        if name.ends_with(".js") && index.contains(name.as_ref()) {
            referenced.push_str(&std::fs::read_to_string(path).unwrap_or_default());
        }
    }
    all.retain(|path| {
        let in_assets = path.parent().is_some_and(|p| p.ends_with("assets"));
        !in_assets || referenced.contains(path.file_name().unwrap().to_string_lossy().as_ref())
    });

    // FNV-1a over every file's path and bytes.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut urls = Vec::new();
    for path in &all {
        let rel = path.strip_prefix(&dir).unwrap().to_string_lossy().replace('\\', "/");
        let name = path.file_name().unwrap().to_string_lossy();
        if rel == "sw.js" || name.starts_with('.') {
            continue;
        }
        let bytes = std::fs::read(path).unwrap_or_default();
        for b in rel.bytes().chain(bytes) {
            hash ^= b as u64;
            hash = hash.wrapping_mul(0x0100_0000_01b3);
        }
        urls.push(if rel == "index.html" { "./".to_string() } else { rel });
    }

    let version = format!("{hash:016x}");
    // Plain placeholder strings survive the minifier that `dx bundle` runs on sw.js.
    let stamped = sw.replacen("__BUILD_VERSION__", &version, 1).replacen("__BUILD_FILES__", &urls.join("|"), 1);
    if stamped == sw {
        eprintln!("sw.js has no placeholders to fill. Was it already stamped?");
        return ExitCode::FAILURE;
    }
    if let Err(e) = std::fs::write(&sw_path, stamped) {
        eprintln!("Can't write {}: {e}", sw_path.display());
        return ExitCode::FAILURE;
    }
    println!("Stamped sw.js: version {version}, {} files to precache.", urls.len());
    ExitCode::SUCCESS
}
