# Baatcheet

Everyday spoken Hindi, ten minutes a day: the phrases you hear and say around the dinner table, with spaced repetition and no penalties for mistakes. A Rust web app (Dioxus), installable to the iPhone home screen and usable offline.

## Layout

- `core/` – content and progress types, Leitner scheduler, card and session logic. No web code. `cargo test -p baatcheet-core`
- `tools/` – `build-content` (CSV → `phrases.json`, with checks), `gen-audio` (ElevenLabs) and `stamp-sw` (writes the file list and version into the release build's service worker)
- `app/` – the Dioxus app. Static files live in `app/public/`
- `content/phrases.csv` – the phrases

## Everyday commands

```bash
# After editing content/phrases.csv
cargo run -p baatcheet-tools --bin build-content

# Generate missing audio (needs .env, see .env.example). Try --dry-run first
cargo run -p baatcheet-tools --bin gen-audio -- --dry-run

# Run the app locally at http://localhost:8080
cd app && dx serve --platform web

# Release build, as deployed
cd app && dx bundle --platform web --release && cd .. && cargo run -p baatcheet-tools --bin stamp-sw
```

## Deploying

Pushing to `main` builds and publishes to GitHub Pages (`.github/workflows/pages.yml`). Open the site in Safari on an iPhone, then Share → Add to Home Screen.
